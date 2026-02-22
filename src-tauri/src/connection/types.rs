use serde::Serialize;

/// Errors that can occur during Kafka operations.
/// Shared across the connection, publisher, and (eventually) shaper modules.
#[derive(Debug, thiserror::Error, Serialize)]
pub enum KafkaError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Send failed: {0}")]
    SendFailed(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Connection timeout after {0} seconds")]
    ConnectionTimeout(u64),

    #[error("Topic creation failed: {0}")]
    TopicCreateFailed(String),

    #[error("Consume failed: {0}")]
    ConsumeFailed(String),
}
