mod config;
mod kafka;

use config::{AppConfig, ConfigError};
use kafka::{KafkaError, KafkaService, SendResult, TopicCreateResult, ConsumedMessage};
use serde::Serialize;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

/// Application state holding the Kafka service
pub struct AppState {
    kafka_service: Arc<Mutex<KafkaService>>,
}

/// Combined result type for Tauri commands
#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum CommandResult<T> {
    Ok(T),
    Err(String),
}

impl<T> From<Result<T, KafkaError>> for CommandResult<T> {
    fn from(result: Result<T, KafkaError>) -> Self {
        match result {
            Ok(data) => CommandResult::Ok(data),
            Err(e) => CommandResult::Err(e.to_string()),
        }
    }
}

impl<T> From<Result<T, ConfigError>> for CommandResult<T> {
    fn from(result: Result<T, ConfigError>) -> Self {
        match result {
            Ok(data) => CommandResult::Ok(data),
            Err(e) => CommandResult::Err(e.to_string()),
        }
    }
}

/// Send a message to Kafka
#[tauri::command]
async fn send_kafka_message(
    state: State<'_, AppState>,
    message: String,
) -> Result<CommandResult<SendResult>, ()> {
    let service = state.kafka_service.lock().await;
    Ok(service.send_message(message).await.into())
}

/// Get the current Kafka configuration
#[tauri::command]
async fn get_kafka_config(state: State<'_, AppState>) -> Result<AppConfig, ()> {
    let service = state.kafka_service.lock().await;
    Ok(service.get_config().await)
}

/// Save Kafka configuration
#[tauri::command]
async fn save_kafka_config(
    state: State<'_, AppState>,
    config: AppConfig,
) -> Result<CommandResult<()>, ()> {
    // Update runtime config
    {
        let service = state.kafka_service.lock().await;
        service.update_config(config.clone()).await;
    }
    
    // Persist to disk
    Ok(config.save().into())
}

/// Test connection to Kafka broker with timeout
#[tauri::command]
async fn test_kafka_connection(
    state: State<'_, AppState>,
    timeout_secs: Option<u64>,
) -> Result<CommandResult<bool>, ()> {
    let service = state.kafka_service.lock().await;
    let timeout = timeout_secs.unwrap_or(5); // Default 5 second timeout
    Ok(service.test_connection(timeout).await.into())
}

/// Create a new Kafka topic
#[tauri::command]
async fn create_kafka_topic(
    state: State<'_, AppState>,
    topic_name: String,
    num_partitions: Option<i32>,
    replication_factor: Option<i16>,
) -> Result<CommandResult<TopicCreateResult>, ()> {
    let service = state.kafka_service.lock().await;
    let partitions = num_partitions.unwrap_or(1);
    let replication = replication_factor.unwrap_or(1);
    Ok(service.create_topic(topic_name, partitions, replication).await.into())
}

/// Consume messages from a Kafka topic
#[tauri::command]
async fn consume_kafka_messages(
    state: State<'_, AppState>,
    topic: String,
    offset: Option<i64>,
    max_messages: Option<i32>,
) -> Result<CommandResult<Vec<ConsumedMessage>>, ()> {
    let service = state.kafka_service.lock().await;
    let start_offset = offset.unwrap_or(0);
    let max = max_messages.unwrap_or(50);
    Ok(service.consume_messages(topic, start_offset, max).await.into())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load config and create Kafka service
    let config = AppConfig::load();
    let kafka_service = Arc::new(Mutex::new(KafkaService::new(config)));
    
    let app_state = AppState { kafka_service };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            send_kafka_message,
            get_kafka_config,
            save_kafka_config,
            test_kafka_connection,
            create_kafka_topic,
            consume_kafka_messages,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
