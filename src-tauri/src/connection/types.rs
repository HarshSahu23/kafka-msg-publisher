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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_failed_display() {
        let err = KafkaError::ConnectionFailed("timeout".to_string());
        assert_eq!(err.to_string(), "Connection failed: timeout");
    }

    #[test]
    fn send_failed_display() {
        let err = KafkaError::SendFailed("broker unavailable".to_string());
        assert_eq!(err.to_string(), "Send failed: broker unavailable");
    }

    #[test]
    fn invalid_config_display() {
        let err = KafkaError::InvalidConfig("no broker".to_string());
        assert_eq!(err.to_string(), "Invalid configuration: no broker");
    }

    #[test]
    fn connection_timeout_display() {
        let err = KafkaError::ConnectionTimeout(10);
        assert_eq!(err.to_string(), "Connection timeout after 10 seconds");
    }

    #[test]
    fn topic_create_failed_display() {
        let err = KafkaError::TopicCreateFailed("already exists".to_string());
        assert_eq!(err.to_string(), "Topic creation failed: already exists");
    }

    #[test]
    fn consume_failed_display() {
        let err = KafkaError::ConsumeFailed("bad offset".to_string());
        assert_eq!(err.to_string(), "Consume failed: bad offset");
    }
}
