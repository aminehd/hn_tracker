use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Duration, TimeZone, Utc, Timelike};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration as StdDuration,
};
use tokio::{sync::RwLock, time};
use tower_http::cors::CorsLayer;
use tracing::{info, error};
use futures::future;
use std::net::SocketAddr;
use axum::serve::serve;

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
    r#type: Option<String>,  // type of item (story, job, comment, etc)
    by: Option<String>,      // author username
    text: Option<String>,    // self-post text content
    descendants: Option<i32>, // total comment count
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HourlyData {
    hour: DateTime<Utc>,
    stories: Vec<StoryInfo>,  // Changed from titles to include more info
    avg_score: f64,
    total_comments: i32,
    top_authors: Vec<(String, i32)>,  // (author, story_count)
    domains: Vec<(String, i32)>,      // (domain, story_count)
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RustStory {
    id: u32,
    title: String,
    score: i32,
    comments: usize,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct HistoricalData {
    hours: Vec<HourlyData>,
    last_update: Option<DateTime<Utc>>,
}

// Add this after the other structs
#[derive(Debug, Clone)]
struct WordCounter {
    stopwords: HashSet<String>,
}

impl Default for WordCounter {
    fn default() -> Self {
        let stopwords: HashSet<String> = vec![
            "the", "a", "an", "and", "or", "but", "in", "on", "at", "to",
            "for", "of", "with", "by", "from", "up", "about", "into", "over",
            "after", "is", "are", "was", "were", "be", "been", "being",
            "have", "has", "had", "do", "does", "did", "will", "would",
            "should", "can", "could", "may", "might", "must", "shall",
            "i", "you", "he", "she", "it", "we", "they", "my", "your",
            "his", "her", "its", "our", "their",
        ].into_iter().map(String::from).collect();
        
        Self { stopwords }
    }
}

impl WordCounter {
    fn count_words(&self, title: &str) -> HashMap<String, usize> {
        let mut word_counts = HashMap::new();
        
        for word in title.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty() && w.len() > 2 && !self.stopwords.contains(*w))
        {
            *word_counts.entry(word.to_string()).or_insert(0) += 1;
        }
        
        word_counts
    }
}

// Shared state
type AppState = Arc<RwLock<HistoricalData>>;

// Add this near the top of the file
#[derive(Debug)]
enum AppError {
    Network(reqwest::Error),
    TimestampError,
    ParseError,
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Network(e) => write!(f, "Network error: {}", e),
            AppError::TimestampError => write!(f, "Invalid timestamp"),
            AppError::ParseError => write!(f, "Parse error"),
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::Network(err)
    }
}

// API handlers
async fn get_historical_data(State(state): State<AppState>) -> impl IntoResponse {
    let data = state.read().await;
    Json(data.clone())
}

async fn get_latest_hour(State(state): State<AppState>) -> impl IntoResponse {
    let data = state.read().await;
    if let Some(latest) = data.hours.last() {
        Ok(Json(latest.clone()))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn force_update(State(state): State<AppState>) -> impl IntoResponse {
    match process_previous_hour().await {
        Ok(hourly_data) => {
            let mut data = state.write().await;
            data.hours.push(hourly_data.clone());
            data.last_update = Some(Utc::now());
            
            let new_length = data.hours.len();
            if new_length > 168 {
                let split_index = new_length - 168;
                data.hours = data.hours.split_off(split_index);
            }
            
            Ok(Json(hourly_data))
        }
        Err(e) => {
            error!("Error processing hour: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Data collection logic
async fn fetch_stories_for_hour(start_time: DateTime<Utc>) -> Result<Vec<Story>, AppError> {
    let end_time = start_time + Duration::hours(1);
    
    // Fetch both new and top stories
    let client = reqwest::Client::builder()
        .timeout(StdDuration::from_secs(10))
        .build()?;
    
    let new_stories: Vec<u32> = client.get("https://hacker-news.firebaseio.com/v0/newstories.json")
        .send()
        .await?
        .json()
        .await?;
    
    let top_stories: Vec<u32> = client.get("https://hacker-news.firebaseio.com/v0/topstories.json")
        .send()
        .await?
        .json()
        .await?;

    // Combine and deduplicate
    let mut story_ids: HashSet<u32> = HashSet::new();
    story_ids.extend(new_stories);
    story_ids.extend(top_stories);

    let mut stories = Vec::new();

    // Fetch stories in parallel batches
    for chunk in story_ids.into_iter().collect::<Vec<_>>().chunks(10) {
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

        // Be nice to the API
        time::sleep(time::Duration::from_millis(100)).await;
    }

    Ok(stories)
}

// Add this helper function to extract domain from URL
fn extract_domain(url: &str) -> Option<String> {
    url.split_once("//")
        .and_then(|(_, rest)| rest.split('/').next())
        .map(|s| s.trim_start_matches("www.").to_string())
}

// Update process_previous_hour to collect more information
async fn process_previous_hour() -> Result<HourlyData, AppError> {
    let now = Utc::now();
    let last_hour = now - Duration::hours(1);
    let hour_start = DateTime::<Utc>::from_naive_utc_and_offset(
        last_hour.naive_utc().date()
            .and_hms_opt(last_hour.naive_local().hour(), 0, 0)
            .unwrap(),
        Utc,
    );

    let stories = fetch_stories_for_hour(hour_start).await?;
    
    let mut total_score = 0;
    let mut total_comments = 0;
    let mut author_counts = HashMap::new();
    let mut domain_counts = HashMap::new();
    let mut story_infos = Vec::new();

    for story in &stories {
        // Count authors
        if let Some(author) = &story.by {
            *author_counts.entry(author.clone()).or_insert(0) += 1;
        }

        // Count domains
        if let Some(url) = &story.url {
            if let Some(domain) = extract_domain(url) {
                *domain_counts.entry(domain).or_insert(0) += 1;
            }
        }

        // Collect story info
        story_infos.push(StoryInfo {
            title: story.title.clone(),
            url: story.url.clone(),
            author: story.by.clone().unwrap_or_default(),
            score: story.score.unwrap_or(0),
            comments: story.descendants.unwrap_or(0),
            domain: story.url.as_ref().and_then(|u| extract_domain(u)),
        });

        if let Some(score) = story.score {
            total_score += score;
        }
        total_comments += story.descendants.unwrap_or(0);
    }

    // Sort authors by story count
    let mut top_authors: Vec<_> = author_counts.into_iter().collect();
    top_authors.sort_by(|a, b| b.1.cmp(&a.1));
    let top_authors = top_authors.into_iter().take(10).collect();

    // Sort domains by story count
    let mut domains: Vec<_> = domain_counts.into_iter().collect();
    domains.sort_by(|a, b| b.1.cmp(&a.1));
    let domains = domains.into_iter().take(10).collect();

    let avg_score = if !stories.is_empty() {
        total_score as f64 / stories.len() as f64
    } else {
        0.0
    };

    Ok(HourlyData {
        hour: hour_start,
        stories: story_infos,
        avg_score,
        total_comments,
        top_authors,
        domains,
    })
}

// Background task for hourly updates
async fn run_hourly_updates(state: AppState) {
    let mut interval = time::interval(time::Duration::from_secs(3600));
    
    loop {
        // Simply await the tick, no error handling needed
        interval.tick().await;
        
        info!("Running scheduled update");
        match process_previous_hour().await {
            Ok(hourly_data) => {
                let mut data = state.write().await;
                data.hours.push(hourly_data.clone());
                data.last_update = Some(Utc::now());
                
                let new_length = data.hours.len();
                if new_length > 168 {
                    let split_index = new_length - 168;
                    data.hours = data.hours.split_off(split_index);
                }
                
                info!("Successfully updated data");
            }
            Err(e) => {
                error!("Error in scheduled update: {}", e);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create shared state
    let state = Arc::new(RwLock::new(HistoricalData::default()));
    let state_clone = state.clone();

    // Spawn background task
    tokio::spawn(async move {
        run_hourly_updates(state_clone).await;
    });

    // Create router with CORS enabled
    let app = Router::new()
        .route("/api/history", get(get_historical_data))
        .route("/api/latest", get(get_latest_hour))
        .route("/api/update", post(force_update))
        .layer(CorsLayer::permissive())
        .with_state(state);

    // Start server with proper error handling
    // let addr: SocketAddr = "127.0.0.1:3000".parse().unwrap();
    let addr: SocketAddr = "[::]:3000".parse().unwrap();

    info!("Starting server on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    if let Err(e) = axum::serve(listener, app).await {
        error!("Server error: {}", e);
        std::process::exit(1);
    }
}