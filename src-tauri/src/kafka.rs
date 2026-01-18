use rskafka::client::partition::{Compression, UnknownTopicHandling};
use rskafka::client::ClientBuilder;
use rskafka::record::Record;
use chrono::Utc;
use serde::Serialize;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::Mutex;

use crate::config::AppConfig;

/// Result of a message send operation
#[derive(Debug, Clone, Serialize)]
pub struct SendResult {
    pub success: bool,
    pub message: String,
    pub timestamp: u64,
}

/// Errors that can occur during Kafka operations
#[derive(Debug, thiserror::Error, Serialize)]
pub enum KafkaError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Send failed: {0}")]
    SendFailed(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

/// Kafka service for managing connections and sending messages
pub struct KafkaService {
    config: Arc<Mutex<AppConfig>>,
}

impl KafkaService {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config: Arc::new(Mutex::new(config)),
        }
    }

    pub async fn update_config(&self, config: AppConfig) {
        let mut current = self.config.lock().await;
        *current = config;
    }

    pub async fn get_config(&self) -> AppConfig {
        self.config.lock().await.clone()
    }

    /// Test connection to the Kafka broker
    pub async fn test_connection(&self) -> Result<bool, KafkaError> {
        let config = self.config.lock().await;
        
        ClientBuilder::new(vec![config.broker.clone()])
            .build()
            .await
            .map_err(|e| KafkaError::ConnectionFailed(e.to_string()))?;

        Ok(true)
    }

    /// Send a message to the configured topic
    pub async fn send_message(&self, message: String) -> Result<SendResult, KafkaError> {
        let config = self.config.lock().await;
        
        // Build client
        let client = ClientBuilder::new(vec![config.broker.clone()])
            .build()
            .await
            .map_err(|e| KafkaError::ConnectionFailed(e.to_string()))?;

        // Get partition client for topic (partition 0)
        let partition_client = client
            .partition_client(&config.topic, 0, UnknownTopicHandling::Error)
            .await
            .map_err(|e| KafkaError::SendFailed(e.to_string()))?;

        // Create record
        let record = Record {
            key: None,
            value: Some(message.into_bytes()),
            headers: Default::default(),
            timestamp: Utc::now(),
        };

        // Send the record
        partition_client
            .produce(vec![record], Compression::NoCompression)
            .await
            .map_err(|e| KafkaError::SendFailed(e.to_string()))?;

        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Ok(SendResult {
            success: true,
            message: "Message sent successfully".to_string(),
            timestamp,
        })
    }
}
