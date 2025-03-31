use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{collections::{HashSet, HashMap}, time::Duration, sync::Arc};
use tokio::time;
use tokio::sync::RwLock;
use chrono::{TimeZone, Utc};
use tracing::{info, error};
use rdkafka::{
    config::ClientConfig,
    producer::{FutureProducer, FutureRecord},
    consumer::{Consumer, StreamConsumer},
    message::{Message, BorrowedMessage},
};
use rdkafka::util::Timeout;
use std::env;
use axum::{
    Router, 
    routing::get,
    http::StatusCode,
    Json,
    extract::State,
};

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

// Domain count structure for API response
#[derive(Debug, Clone, Serialize)]
struct DomainCount {
    domain: String,
    count: usize,
}

// API response for top domains
#[derive(Debug, Clone, Serialize)]
struct TopDomainsResponse {
    domains: Vec<DomainCount>,
    total_stories: usize,
    updated_at: String,
}

// Application state to be shared between components
#[derive(Clone)]
struct AppState {
    domain_counts: Arc<RwLock<HashMap<String, usize>>>,
    top_domains: Arc<RwLock<Vec<DomainCount>>>,
    total_stories: Arc<RwLock<usize>>,
    last_updated: Arc<RwLock<String>>,
}

fn extract_domain(url: &str) -> Option<String> {
    url.split_once("//")
        .and_then(|(_, rest)| rest.split('/').next())
        .map(|s| s.trim_start_matches("www.").to_string())
}

async fn fetch_latest_stories(client: &Client, count: usize) -> Result<Vec<Story>, reqwest::Error> {
    // Fetch the IDs of the newest stories
    let new_stories: Vec<u32> = client
        .get("https://hacker-news.firebaseio.com/v0/newstories.json")
        .send()
        .await?
        .json()
        .await?;
    
    // Take only the most recent stories (they're already sorted, newest first)
    let latest_ids: Vec<u32> = new_stories.into_iter().take(count).collect();
    
    // Fetch each story in parallel
    let mut stories = Vec::new();
    for id in latest_ids {
        let url = format!("https://hacker-news.firebaseio.com/v0/item/{}.json", id);
        match client.get(&url).send().await {
            Ok(resp) => {
                if let Ok(story) = resp.json::<Story>().await {
                    stories.push(story);
                }
            }
            Err(e) => {
                error!("Failed to fetch story {}: {}", id, e);
            }
        }
        
        // Small delay to avoid rate limiting
        time::sleep(Duration::from_millis(50)).await;
    }
    
    Ok(stories)
}

fn format_story(story: &Story) -> String {
    let timestamp = match Utc.timestamp_opt(story.time, 0).single() {
        Some(dt) => dt.to_rfc3339(),
        None => "Invalid timestamp".to_string(),
    };
    
    let domain = story.url.as_ref()
        .and_then(|u| extract_domain(u))
        .unwrap_or_else(|| "no domain".to_string());
    
    format!(
        "ID: {}\nTitle: {}\nBy: {}\nTime: {}\nURL: {}\nDomain: {}\nScore: {}\nComments: {}\n",
        story.id,
        story.title,
        story.by.as_deref().unwrap_or("anonymous"),
        timestamp,
        story.url.as_deref().unwrap_or("none"),
        domain,
        story.score.unwrap_or(0),
        story.descendants.unwrap_or(0)
    )
}

async fn send_to_kafka(producer: &FutureProducer, topic: &str, story: &Story) -> Result<(), String> {
    // Serialize the story to JSON
    let story_json = match serde_json::to_string(story) {
        Ok(json) => json,
        Err(e) => return Err(format!("Failed to serialize story: {}", e)),
    };
    
    // Convert id to string once and keep it
    let id_str = story.id.to_string();
    
    // Create a record with the story ID as the key
    let record = FutureRecord::to(topic)
        .key(&id_str)
        .payload(&story_json);
    
    // Send the record to Kafka
    match producer.send(record, Duration::from_secs(5)).await {
        Ok(delivery) => {
            let (partition, offset) = delivery;
            info!("Sent story ID {} to Kafka topic {} (partition: {}, offset: {})",
                story.id, topic, partition, offset);
            Ok(())
        }
        Err((e, _)) => {
            error!("Failed to send story ID {} to Kafka: {}", story.id, e);
            Err(format!("Failed to send to Kafka: {}", e))
        }
    }
}

// Process a story for domain counting
async fn process_story_domain(story: &Story, app_state: &AppState) {
    // Extract domain from URL
    if let Some(url) = &story.url {
        if let Some(domain) = extract_domain(url) {
            // Update domain counts
            let mut domain_counts = app_state.domain_counts.write().await;
            let count = domain_counts.entry(domain.clone()).or_insert(0);
            *count += 1;
            
            // Update total stories counter
            let mut total = app_state.total_stories.write().await;
            *total += 1;
            
            // Update last updated timestamp
            let current_time = Utc::now().to_rfc3339();
            let mut last_updated = app_state.last_updated.write().await;
            *last_updated = current_time;
            
            // Log domain count update
            info!("Updated domain count for {}: {}", domain, *count);
        }
    }
}

// Update the top domains list based on current counts
async fn update_top_domains(app_state: &AppState) {
    let domain_counts = app_state.domain_counts.read().await;
    
    // Convert HashMap to Vec for sorting
    let mut domains: Vec<DomainCount> = domain_counts
        .iter()
        .map(|(domain, count)| DomainCount {
            domain: domain.clone(),
            count: *count,
        })
        .collect();
    
    // Sort by count, descending
    domains.sort_by(|a, b| b.count.cmp(&a.count));
    
    // Take top 10
    let top_domains = domains.into_iter().take(100).collect();
    
    // Update state
    let mut top = app_state.top_domains.write().await;
    *top = top_domains;
    
    info!("Updated top domains list");
}

// API handler for retrieving top domains
async fn get_top_domains(
    State(app_state): State<AppState>,
) -> Result<Json<TopDomainsResponse>, StatusCode> {
    // Read current state
    let top_domains = app_state.top_domains.read().await.clone();
    let total_stories = *app_state.total_stories.read().await;
    let last_updated = app_state.last_updated.read().await.clone();
    
    // Prepare response
    let response = TopDomainsResponse {
        domains: top_domains,
        total_stories,
        updated_at: last_updated,
    };
    
    Ok(Json(response))
}

// Create Kafka consumer that reads messages and updates domain counts
async fn run_kafka_consumer(kafka_broker: String, topic: String, app_state: AppState) -> Result<(), String> {
    // Create consumer
    let consumer: StreamConsumer = ClientConfig::new()
        .set("group.id", "hn-domain-counter")
        .set("bootstrap.servers", &kafka_broker)
        .set("enable.auto.commit", "true")
        .set("auto.offset.reset", "earliest")
        .create()
        .expect("Failed to create Kafka consumer");
    
    // Subscribe to topic
    consumer.subscribe(&[&topic])
        .expect("Failed to subscribe to topic");
    
    info!("Kafka consumer started, reading from topic: {}", topic);
    
    // Process interval for updating top domains
    let mut update_interval = time::interval(Duration::from_secs(30));
    
    // Main consumer loop
    loop {
        tokio::select! {
            _ = update_interval.tick() => {
                update_top_domains(&app_state).await;
            }
            message_result = consumer.recv() => {
                match message_result {
                    Ok(message) => {
                        if let Some(payload) = message.payload() {
                            // Try to parse the message as a story
                            match serde_json::from_slice::<Story>(payload) {
                                Ok(story) => {
                                    info!("Received story: {} (ID: {})", story.title, story.id);
                                    process_story_domain(&story, &app_state).await;
                                }
                                Err(e) => {
                                    error!("Failed to parse message as Story: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error receiving message: {}", e);
                        // Return error if recv fails
                        return Err(format!("Kafka consumer error: {}", e));
                    }
                }
            }
        }
    }
    
    // This point should never be reached due to infinite loop
    #[allow(unreachable_code)]
    Ok(())
}

#[tokio::main]
async fn main() {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();
    
    info!("HackerNews story tracker starting...");
    
    // Get Kafka broker from environment or use default
    let kafka_broker = env::var("KAFKA_BROKER").unwrap_or_else(|_| "localhost:9092".to_string());
    info!("Using Kafka broker: {}", kafka_broker);
    
    // Create Kafka producer
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &kafka_broker)
        .set("message.timeout.ms", "5000")
        .create()
        .expect("Failed to create Kafka producer");
    
    // Topic to send stories to
    let topic = "hackernews-stories";
    
    // Create application state
    let app_state = AppState {
        domain_counts: Arc::new(RwLock::new(HashMap::new())),
        top_domains: Arc::new(RwLock::new(Vec::new())),
        total_stories: Arc::new(RwLock::new(0)),
        last_updated: Arc::new(RwLock::new(Utc::now().to_rfc3339())),
    };
    
    // Clone state and topic for consumer task
    let consumer_app_state = app_state.clone();
    let consumer_topic = topic.to_string();
    let consumer_kafka_broker = kafka_broker.clone();
    
    // Start Kafka consumer in a separate task
    tokio::spawn(async move {
        match run_kafka_consumer(consumer_kafka_broker, consumer_topic, consumer_app_state).await {
            Ok(_) => info!("Kafka consumer task completed successfully"),
            Err(e) => error!("Kafka consumer task failed: {}", e),
        }
    });
    
    // CORS layer to allow requests from any origin
    let cors = tower_http::cors::CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);
        
    // Create API router
    let app = Router::new()
        .route("/api/top-domains", get(get_top_domains))
        .with_state(app_state.clone())
        .layer(cors);
    
    // Start API server in a separate task
    let api_port = env::var("API_PORT").unwrap_or_else(|_| "3000".to_string());
    let api_addr = format!("0.0.0.0:{}", api_port);
    info!("Starting API server on {}", api_addr);
    
    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(&api_addr).await.unwrap();
        info!("API server listening on {}", api_addr);
        match axum::serve(listener, app).await {
            Ok(_) => {},
            Err(e) => error!("API server error: {}", e),
        }
    });
    
    // Create a reqwest client with appropriate timeouts
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to create HTTP client");
    
    // Track the IDs of stories we've already seen
    let mut seen_story_ids = HashSet::new();
    
    // Main loop - fetch stories every minute
    let mut interval = time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
        info!("Fetching latest stories...");
        
        match fetch_latest_stories(&client, 100).await {
            Ok(stories) => {
                // Filter to only new stories
                let new_stories: Vec<&Story> = stories.iter()
                    .filter(|story| !seen_story_ids.contains(&story.id))
                    .collect();
                
                if new_stories.is_empty() {
                    info!("No new stories found");
                } else {
                    info!("Found {} new stories", new_stories.len());
                    
                    // Log each new story
                    for story in &new_stories {
                        // Add to seen IDs
                        seen_story_ids.insert(story.id);
                        
                        // Format and log the story
                        let story_text = format_story(story);
                        info!("New story:\n{}", story_text);
                        
                        // Send to Kafka
                        match send_to_kafka(&producer, topic, story).await {
                            Ok(_) => info!("Successfully sent story ID {} to Kafka", story.id),
                            Err(e) => error!("Failed to send story ID {} to Kafka: {}", story.id, e),
                        }
                        
                        // Process the story domain locally as well (in case consumer is behind)
                        process_story_domain(story, &app_state).await;
                    }
                    
                    // Update top domains after processing batch
                    update_top_domains(&app_state).await;
                }
                
                // Prevent the seen stories set from growing indefinitely
                if seen_story_ids.len() > 1000 {
                    // Keep only the most recent 500 story IDs
                    let newest_ids: Vec<u32> = stories.iter()
                        .map(|s| s.id)
                        .collect();
                    
                    let old_count = seen_story_ids.len();
                    seen_story_ids = newest_ids.into_iter().collect();
                    info!("Cleaned story ID cache: {} -> {}", old_count, seen_story_ids.len());
                }
            }
            Err(e) => {
                error!("Failed to fetch stories to kafka: {}", e);
            }
        }
    }
}