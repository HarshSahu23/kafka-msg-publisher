use rskafka::client::partition::{Compression, OffsetAt, UnknownTopicHandling};
use rskafka::record::Record;
use chrono::Utc;
use serde::Serialize;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::Mutex;

use crate::config::AppConfig;
use crate::connection::{build_client_builder, KafkaError};

/// Result of a message send operation
#[derive(Debug, Clone, Serialize)]
pub struct SendResult {
    pub success: bool,
    pub message: String,
    pub timestamp: u64,
}

/// Result of a topic creation operation
#[derive(Debug, Clone, Serialize)]
pub struct TopicCreateResult {
    pub success: bool,
    pub message: String,
    pub topic: String,
}

/// A consumed message from Kafka
#[derive(Debug, Clone, Serialize)]
pub struct ConsumedMessage {
    pub offset: i64,
    pub key: Option<String>,
    pub value: Option<String>,
    pub timestamp: i64,
}

/// Kafka publisher service for managing connections and sending/consuming messages.
///
/// Holds the active [`AppConfig`] behind an `Arc<Mutex>` so it can be shared across
/// Tauri command handlers and updated at runtime without restarting the service.
///
/// Connection building is delegated to [`crate::connection::factory`] — this struct
/// contains no TLS or SASL logic itself.
#[derive(Clone)]
pub struct KafkaService {
    config: Arc<Mutex<AppConfig>>,
}

impl KafkaService {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config: Arc::new(Mutex::new(config)),
        }
    }

    /// Clone the service handle (shares the same config via Arc)
    pub fn clone_service(&self) -> Self {
        self.clone()
    }

    pub async fn update_config(&self, config: AppConfig) {
        let mut current = self.config.lock().await;
        *current = config;
    }

    pub async fn get_config(&self) -> AppConfig {
        self.config.lock().await.clone()
    }

    /// Test connection to the Kafka broker with timeout
    pub async fn test_connection(&self, timeout_secs: u64) -> Result<bool, KafkaError> {
        // Clone config and release lock BEFORE async operation
        let config = {
            self.config.lock().await.clone()
        };

        let builder = build_client_builder(&config)?;
        let connect_future = builder.build();

        match tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            connect_future,
        )
        .await
        {
            Ok(Ok(_)) => Ok(true),
            Ok(Err(e)) => Err(KafkaError::ConnectionFailed(e.to_string())),
            Err(_) => Err(KafkaError::ConnectionTimeout(timeout_secs)),
        }
    }

    /// Send a message to the configured topic with timeout
    pub async fn send_message(&self, message: String) -> Result<SendResult, KafkaError> {
        // Clone config and release lock BEFORE async operations
        let config = {
            self.config.lock().await.clone()
        };
        let topic = config.topic.clone();

        let builder = build_client_builder(&config)?;

        // Wrap entire operation in a 10 second timeout
        let send_future = async {
            let client = builder
                .build()
                .await
                .map_err(|e| KafkaError::ConnectionFailed(e.to_string()))?;

            let partition_client = client
                .partition_client(&topic, 0, UnknownTopicHandling::Error)
                .await
                .map_err(|e| KafkaError::SendFailed(e.to_string()))?;

            let record = Record {
                key: None,
                value: Some(message.into_bytes()),
                headers: Default::default(),
                timestamp: Utc::now(),
            };

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
        };

        match tokio::time::timeout(std::time::Duration::from_secs(10), send_future).await {
            Ok(result) => result,
            Err(_) => Err(KafkaError::ConnectionTimeout(10)),
        }
    }

    /// Create a new topic on the Kafka broker
    pub async fn create_topic(
        &self,
        topic_name: String,
        num_partitions: i32,
        replication_factor: i16,
    ) -> Result<TopicCreateResult, KafkaError> {
        let config = {
            self.config.lock().await.clone()
        };

        let builder = build_client_builder(&config)?;

        let create_future = async {
            let client = builder
                .build()
                .await
                .map_err(|e| KafkaError::ConnectionFailed(e.to_string()))?;

            let controller_client = client
                .controller_client()
                .map_err(|e| KafkaError::TopicCreateFailed(e.to_string()))?;

            controller_client
                .create_topic(&topic_name, num_partitions, replication_factor, 5_000)
                .await
                .map_err(|e| KafkaError::TopicCreateFailed(e.to_string()))?;

            Ok(TopicCreateResult {
                success: true,
                message: format!("Topic '{}' created successfully", topic_name),
                topic: topic_name,
            })
        };

        match tokio::time::timeout(std::time::Duration::from_secs(10), create_future).await {
            Ok(result) => result,
            Err(_) => Err(KafkaError::ConnectionTimeout(10)),
        }
    }

    /// Consume messages from the configured topic
    pub async fn consume_messages(
        &self,
        topic: String,
        offset: i64,
        max_messages: i32,
    ) -> Result<Vec<ConsumedMessage>, KafkaError> {
        let config = {
            self.config.lock().await.clone()
        };

        let builder = build_client_builder(&config)?;

        let consume_future = async {
            let client = builder
                .build()
                .await
                .map_err(|e| KafkaError::ConnectionFailed(e.to_string()))?;

            let partition_client = client
                .partition_client(&topic, 0, UnknownTopicHandling::Error)
                .await
                .map_err(|e| KafkaError::ConsumeFailed(e.to_string()))?;

            // Query the actual available offset range
            let earliest = partition_client
                .get_offset(OffsetAt::Earliest)
                .await
                .map_err(|e| {
                    KafkaError::ConsumeFailed(format!("Failed to get earliest offset: {}", e))
                })?;
            let latest = partition_client
                .get_offset(OffsetAt::Latest)
                .await
                .map_err(|e| {
                    KafkaError::ConsumeFailed(format!("Failed to get latest offset: {}", e))
                })?;

            // If partition is empty (no messages), return empty
            if earliest >= latest {
                return Ok(vec![]);
            }

            // Clamp the requested offset to the valid range
            let effective_offset = if offset < earliest {
                earliest
            } else if offset >= latest {
                // No messages at or after this offset
                return Ok(vec![]);
            } else {
                offset
            };

            let (records, _high_watermark) = partition_client
                .fetch_records(
                    effective_offset,
                    1..1_048_576, // 1 byte to 1 MB
                    5_000,        // 5 second max wait
                )
                .await
                .map_err(|e| KafkaError::ConsumeFailed(e.to_string()))?;

            let messages: Vec<ConsumedMessage> = records
                .into_iter()
                .take(max_messages as usize)
                .map(|record| ConsumedMessage {
                    offset: record.offset,
                    key: record.record.key.map(|k| String::from_utf8_lossy(&k).to_string()),
                    value: record.record.value.map(|v| String::from_utf8_lossy(&v).to_string()),
                    timestamp: record.record.timestamp.timestamp_millis(),
                })
                .collect();

            Ok(messages)
        };

        match tokio::time::timeout(std::time::Duration::from_secs(15), consume_future).await {
            Ok(result) => result,
            Err(_) => Err(KafkaError::ConnectionTimeout(15)),
        }
    }
}
