use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Duration as ChronoDuration, TimeZone, Timelike, Utc};
use futures::future;
use rdkafka::{
    config::ClientConfig,
    consumer::{Consumer, StreamConsumer},
    producer::{FutureProducer, FutureRecord},
    util::Timeout,
    Message,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};
use tokio::{sync::RwLock, time};
use tower_http::cors::CorsLayer;
use tracing::{error, info, warn};

// Data structures
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Story {
    id: u32,
    title: String,
    score: Option<i32>,
    time: i64,
    #[serde(default)]
    kids: Vec<u32>,
    url: Option<String>,
    r#type: Option<String>, // type of item (story, job, comment, etc)
    by: Option<String>,     // author username
    text: Option<String>,   // self-post text content
    descendants: Option<i32>, // total comment count
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HourlyData {
    hour: DateTime<Utc>,
    stories: Vec<StoryInfo>,
    avg_score: f64,
    total_comments: i32,
    top_authors: Vec<(String, i32)>, // (author, story_count)
    domains: Vec<(String, i32)>,     // (domain, story_count)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoryInfo {
    title: String,
    url: Option<String>,
    author: String,
    score: i32,
    comments: i32,
    domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct HistoricalData {
    hours: Vec<HourlyData>,
    last_update: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct HourlyResponse {
    pub hourly_data: Vec<HourlyData>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct WordCounter {
    stopwords: HashSet<String>,
}

impl Default for WordCounter {
    fn default() -> Self {
        let stopwords: HashSet<String> = vec![
            "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with",
            "by", "from", "up", "about", "into", "over", "after", "is", "are", "was", "were",
            "be", "been", "being", "have", "has", "had", "do", "does", "did", "will", "would",
            "should", "can", "could", "may", "might", "must", "shall", "i", "you", "he", "she",
            "it", "we", "they", "my", "your", "his", "her", "its", "our", "their",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        Self { stopwords }
    }
}

impl WordCounter {
    fn count_words(&self, title: &str) -> HashMap<String, usize> {
        let mut word_counts = HashMap::new();

        for word in title
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty() && w.len() > 2 && !self.stopwords.contains(*w))
        {
            *word_counts.entry(word.to_string()).or_insert(0) += 1;
        }

        word_counts
    }
}

// App state and error handling
struct AppStateData {
    historical_data: HistoricalData,
    kafka_producer: FutureProducer,
}

type AppState = Arc<RwLock<AppStateData>>;

fn setup_kafka() -> FutureProducer {
    let broker = std::env::var("KAFKA_BROKER").unwrap_or_else(|_| "kafka:9092".to_string());
    ClientConfig::new()
        .set("bootstrap.servers", &broker)
        .set("message.timeout.ms", "9000")
        .set("security.protocol", "PLAINTEXT")
        .set("allow.auto.create.topics", "true")
        .set("socket.timeout.ms", "10000")
        .set("retry.backoff.ms", "500")
        .set("metadata.request.timeout.ms", "10000")
        .create()
        .expect("Failed to create Kafka producer")
}

#[derive(Debug)]
enum AppError {
    Network(reqwest::Error),
    TimestampError,
    ParseError,
    KafkaError(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Network(e) => write!(f, "Network error: {}", e),
            AppError::TimestampError => write!(f, "Invalid timestamp"),
            AppError::ParseError => write!(f, "Parse error"),
            AppError::KafkaError(e) => write!(f, "Kafka error: {}", e),
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::Network(err)
    }
}

impl From<StatusCode> for AppError {
    fn from(status: StatusCode) -> Self {
        AppError::KafkaError(format!("HTTP error: {}", status))
    }
}

// API handlers
async fn get_historical_data(State(state): State<AppState>) -> impl IntoResponse {
    // Try to get historical data from Kafka first
    match get_all_kafka_messages().await {
        Ok(hourly_data) => {
            if !hourly_data.is_empty() {
                let historical = HistoricalData {
                    hours: hourly_data,
                    last_update: Some(Utc::now()),
                };
                return Json(historical);
            }
        }
        Err(e) => {
            error!("Failed to fetch Kafka history: {}", e);
            // Continue to use in-memory data as fallback
        }
    }

    // Fallback to in-memory data
    let data = state.read().await;
    Json(data.historical_data.clone())
}

async fn get_latest_hour(State(state): State<AppState>) -> impl IntoResponse {
    let data = state.read().await;
    if let Some(latest) = data.historical_data.hours.last() {
        Ok(Json(latest.clone()))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn latest_kafka_handler() -> impl IntoResponse {
    match get_latest_kafka_message().await {
        Ok(Some(data)) => {
            info!("Received Kafka message: {:?}", data);
            Ok(Json(data))
        }
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!("Kafka error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Kafka functions
async fn get_all_kafka_messages() -> Result<Vec<HourlyData>, AppError> {
    info!("Creating Kafka consumer");
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", "kafka:9092")
        .set("group.id", "history-reader-group") // Use a different consumer group
        .set("auto.offset.reset", "earliest") // Read from the beginning
        .set("enable.auto.commit", "false") // Disable auto commit to keep messages
        .set("session.timeout.ms", "6000")
        .set("heartbeat.interval.ms", "2000")
        .create()
        .map_err(|e| {
            error!("Failed to create Kafka consumer: {}", e);
            AppError::KafkaError(e.to_string())
        })?;

    info!("Subscribing to hn-updates topic");
    consumer.subscribe(&["hn-updates"]).map_err(|e| {
        error!("Failed to subscribe to topic: {}", e);
        AppError::KafkaError(e.to_string())
    })?;

    let mut all_hourly_data = Vec::new();
    let mut timeout_count = 0;

    // Try to read up to 168 messages (1 week worth of hourly data)
    // or stop after 3 consecutive timeouts
    while all_hourly_data.len() < 168 && timeout_count < 3 {
        match tokio::time::timeout(Duration::from_secs(2), consumer.recv()).await {
            Ok(msg_result) => match msg_result {
                Ok(msg) => {
                    // Reset timeout counter on successful message
                    timeout_count = 0;

                    if let Some(payload) = msg.payload() {
                        if let Ok(payload_str) = std::str::from_utf8(payload) {
                            match serde_json::from_str::<HourlyData>(payload_str) {
                                Ok(data) => {
                                    info!(
                                        "Parsed message: hour={}, stories={}",
                                        data.hour,
                                        data.stories.len()
                                    );
                                    all_hourly_data.push(data);
                                }
                                Err(e) => {
                                    error!("Failed to parse message: {}", e);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Error receiving Kafka message: {}", e);
                    break;
                }
            },
            Err(_) => {
                // Increment timeout counter
                timeout_count += 1;
                info!("Timeout waiting for messages (count: {})", timeout_count);
            }
        }
    }

    info!(
        "Retrieved {} hourly data records from Kafka",
        all_hourly_data.len()
    );

    // Sort data by hour in descending order (newest first)
    all_hourly_data.sort_by(|a, b| b.hour.cmp(&a.hour));

    Ok(all_hourly_data)
}

async fn get_latest_kafka_message() -> Result<Option<HourlyData>, AppError> {
    match get_all_kafka_messages().await {
        Ok(messages) => {
            // Return only the latest (first) message
            Ok(messages.into_iter().next())
        }
        Err(e) => Err(e),
    }
}

// HackerNews API functions
async fn fetch_stories_for_timespan(
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
) -> Result<Vec<Story>, AppError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let new_stories: Vec<u32> = client
        .get("https://hacker-news.firebaseio.com/v0/newstories.json")
        .send()
        .await?
        .json()
        .await?;

    let top_stories: Vec<u32> = client
        .get("https://hacker-news.firebaseio.com/v0/topstories.json")
        .send()
        .await?
        .json()
        .await?;

    let mut story_ids: HashSet<u32> = HashSet::new();
    story_ids.extend(new_stories);
    story_ids.extend(top_stories);

    let mut stories = Vec::new();

    for chunk in story_ids.into_iter().collect::<Vec<_>>().chunks(20) {
        let mut futures = Vec::new();
        for &id in chunk {
            let url = format!("https://hacker-news.firebaseio.com/v0/item/{}.json", id);
            futures.push(client.get(&url).send());
        }

        let responses = future::join_all(futures).await;
        for response in responses {
            if let Ok(resp) = response {
                if let Ok(story) = resp.json::<Story>().await {
                    if let Some(story_time) = Utc.timestamp_opt(story.time, 0).single() {
                        if story_time >= start_time && story_time < end_time {
                            stories.push(story);
                        }
                    }
                }
            }
        }

        time::sleep(time::Duration::from_millis(100)).await;
    }

    Ok(stories)
}

fn extract_domain(url: &str) -> Option<String> {
    url.split_once("//")
        .and_then(|(_, rest)| rest.split('/').next())
        .map(|s| s.trim_start_matches("www.").to_string())
}

async fn process_time_range(end_time: DateTime<Utc>) -> Result<HourlyResponse, AppError> {
    let one_hour_ago = end_time - ChronoDuration::hours(1);
    let stories = fetch_stories_for_timespan(one_hour_ago, end_time).await?;

    // Group stories by hour
    let mut hour_groups: HashMap<DateTime<Utc>, Vec<Story>> = HashMap::new();

    for story in stories {
        if let Some(story_time) = Utc.timestamp_opt(story.time, 0).single() {
            let hour_start = DateTime::<Utc>::from_naive_utc_and_offset(
                story_time
                    .naive_utc()
                    .date()
                    .and_hms_opt(story_time.hour(), 0, 0)
                    .unwrap(),
                Utc,
            );
            hour_groups.entry(hour_start).or_default().push(story);
        }
    }

    let mut hourly_data = Vec::new();

    for (hour, stories) in hour_groups {
        let mut total_score = 0;
        let mut total_comments = 0;
        let mut author_counts = HashMap::new();
        let mut domain_counts = HashMap::new();
        let mut story_infos = Vec::new();

        for story in &stories {
            if let Some(author) = &story.by {
                *author_counts.entry(author.clone()).or_insert(0) += 1;
            }

            if let Some(url) = &story.url {
                if let Some(domain) = extract_domain(url) {
                    *domain_counts.entry(domain).or_insert(0) += 1;
                }
            }

            story_infos.push(StoryInfo {
                title: story.title.clone(),
                url: story.url.clone(),
                author: story.by.clone().unwrap_or_default(),
                score: story.score.unwrap_or(0),
                comments: story.descendants.unwrap_or(0),
                domain: story.url.as_ref().and_then(|u| extract_domain(u)),
            });

            total_score += story.score.unwrap_or(0);
            total_comments += story.descendants.unwrap_or(0);
        }

        let mut top_authors: Vec<_> = author_counts.into_iter().collect();
        top_authors.sort_by(|a, b| b.1.cmp(&a.1));

        let mut domains: Vec<_> = domain_counts.into_iter().collect();
        domains.sort_by(|a, b| b.1.cmp(&a.1));

        let hourly = HourlyData {
            hour,
            stories: story_infos,
            avg_score: if !stories.is_empty() {
                total_score as f64 / stories.len() as f64
            } else {
                0.0
            },
            total_comments,
            top_authors: top_authors.into_iter().take(10).collect(),
            domains: domains.into_iter().take(10).collect(),
        };

        hourly_data.push(hourly);
    }

    hourly_data.sort_by(|a, b| b.hour.cmp(&a.hour));

    Ok(HourlyResponse {
        hourly_data,
        timestamp: end_time,
    })
}

// Background update task
async fn run_updates(state: AppState) {
    let mut interval = time::interval(time::Duration::from_secs(3600)); // 1 hour

    loop {
        interval.tick().await;

        let now = Utc::now();
        let current_hour = DateTime::<Utc>::from_naive_utc_and_offset(
            now.naive_utc()
                .date()
                .and_hms_opt(now.hour(), 0, 0)
                .unwrap(),
            Utc,
        );
        info!(
            "Processing data for hour starting at: {}",
            current_hour.to_rfc3339()
        );
        match process_time_range(current_hour).await {
            Ok(response) => {
                let state_data = state.read().await;

                // Write to Kafka
                for hour_data in &response.hourly_data {
                    if let Ok(json) = serde_json::to_string(&hour_data) {
                        let key = hour_data.hour.to_rfc3339();
                        let record = FutureRecord::to("hn-updates")
                            .payload(&json)
                            .key(&key);
                        match state_data
                            .kafka_producer
                            .send(record, Timeout::After(Duration::from_secs(25)))
                            .await
                        {
                            Ok(_) => info!("Successfully sent to Kafka"),
                            Err(e) => error!("Failed to send to Kafka: {:?}", e),
                        }
                    }
                }

                // Update state
                let mut state_data = state.write().await;
                state_data.historical_data.hours = response.hourly_data;
                state_data.historical_data.last_update = Some(response.timestamp);

                info!("Successfully updated hourly data");
            }
            Err(e) => {
                error!("Error in update: {}", e);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Setup Kafka producer
    let producer = setup_kafka();

    // Create shared state
    let state = Arc::new(RwLock::new(AppStateData {
        historical_data: HistoricalData::default(),
        kafka_producer: producer,
    }));
    let state_clone = state.clone();

    // Spawn background task for updates
    tokio::spawn(async move {
        run_updates(state_clone).await;
    });

    // API handler for source domain counts
async fn get_latest_source_counts() -> impl IntoResponse {
    match get_latest_kafka_message().await {
        Ok(Some(hourly_data)) => {
            // Count domains
            let mut domain_counts: HashMap<String, usize> = HashMap::new();
            
            for story in &hourly_data.stories {
                if let Some(domain) = &story.domain {
                    *domain_counts.entry(domain.clone()).or_insert(0) += 1;
                }
            }
            
            // Convert to vector and sort by count (descending)
            let mut sources: Vec<(String, usize)> = domain_counts.into_iter().collect();
            sources.sort_by(|a, b| b.1.cmp(&a.1));
            
            // Take top 20 sources
            let top_sources = sources.into_iter().take(20).collect::<Vec<_>>();
            
            Ok(Json(top_sources))
        },
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!("Error fetching latest data for source counts: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Create router with CORS enabled
    let app = Router::new()
        .route("/api/history", get(get_historical_data))
        .route("/api/latest", get(get_latest_hour))
        .route("/api/kafka/latest", get(latest_kafka_handler))
        .route("/api/sources", get(get_latest_source_counts))
        .layer(CorsLayer::permissive())
        .with_state(state);

    // Start server with proper error handling
    let addr: SocketAddr = "[::]:3000".parse().unwrap();
    info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    if let Err(e) = axum::serve(listener, app).await {
        error!("Server error: {}", e);
        std::process::exit(1);
    }
}