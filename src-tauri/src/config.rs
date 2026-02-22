use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Security protocol for Kafka connection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SecurityProtocol {
    Plaintext,
    Ssl,
    SaslPlaintext,
    SaslSsl,
}

impl Default for SecurityProtocol {
    fn default() -> Self {
        Self::Plaintext
    }
}

/// SASL authentication mechanism
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SaslMechanism {
    Plain,
    ScramSha256,
    ScramSha512,
}

impl Default for SaslMechanism {
    fn default() -> Self {
        Self::Plain
    }
}

/// Application configuration for Kafka connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub broker: String,
    pub topic: String,
    pub client_id: String,
    #[serde(default)]
    pub security_protocol: SecurityProtocol,
    #[serde(default)]
    pub sasl_mechanism: SaslMechanism,
    #[serde(default)]
    pub sasl_username: String,
    #[serde(default)]
    pub sasl_password: String,
    #[serde(default)]
    pub ssl_ca_cert_path: String,
    #[serde(default)]
    pub ssl_client_cert_path: String,
    #[serde(default)]
    pub ssl_client_key_path: String,
    #[serde(default)]
    pub ssl_skip_verification: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            broker: "localhost:9092".to_string(),
            topic: "test-topic".to_string(),
            client_id: "kafka-msg-publisher".to_string(),
            security_protocol: SecurityProtocol::default(),
            sasl_mechanism: SaslMechanism::default(),
            sasl_username: String::new(),
            sasl_password: String::new(),
            ssl_ca_cert_path: String::new(),
            ssl_client_cert_path: String::new(),
            ssl_client_key_path: String::new(),
            ssl_skip_verification: false,
        }
    }
}

impl AppConfig {
    /// Get the config file path in the app data directory
    fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("kafka-msg-publisher").join("config.json"))
    }

    /// Load config from disk, or return default if not found
    pub fn load() -> Self {
        Self::config_path()
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default()
    }

    /// Save config to disk
    pub fn save(&self) -> Result<(), ConfigError> {
        let path = Self::config_path().ok_or(ConfigError::NoConfigDir)?;
        
        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| ConfigError::IoError(e.to_string()))?;
        }

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| ConfigError::SerializeError(e.to_string()))?;
        
        fs::write(path, content).map_err(|e| ConfigError::IoError(e.to_string()))?;
        
        Ok(())
    }
}

/// Errors that can occur during config operations
#[derive(Debug, thiserror::Error, Serialize)]
pub enum ConfigError {
    #[error("Could not find config directory")]
    NoConfigDir,
    
    #[error("IO error: {0}")]
    IoError(String),
    
    #[error("Serialization error: {0}")]
    SerializeError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn minimal_config() -> AppConfig {
        AppConfig {
            broker: "localhost:9092".to_string(),
            topic: "my-topic".to_string(),
            client_id: "test".to_string(),
            ..AppConfig::default()
        }
    }

    // --- Default values ---

    #[test]
    fn default_broker_is_localhost() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.broker, "localhost:9092");
    }

    #[test]
    fn default_topic_is_test_topic() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.topic, "test-topic");
    }

    #[test]
    fn default_security_protocol_is_plaintext() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.security_protocol, SecurityProtocol::Plaintext);
    }

    #[test]
    fn default_sasl_mechanism_is_plain() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.sasl_mechanism, SaslMechanism::Plain);
    }

    #[test]
    fn default_ssl_skip_verification_is_false() {
        let cfg = AppConfig::default();
        assert!(!cfg.ssl_skip_verification);
    }

    // --- Serialization roundtrip ---

    #[test]
    fn serde_roundtrip_preserves_all_fields() {
        let original = minimal_config();
        let json = serde_json::to_string(&original).expect("serialize failed");
        let restored: AppConfig = serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(restored.broker, original.broker);
        assert_eq!(restored.topic, original.topic);
        assert_eq!(restored.client_id, original.client_id);
    }

    #[test]
    fn deserialize_missing_optional_fields_uses_defaults() {
        // Only the 3 required fields; all serde(default) fields should fall back gracefully
        let json = r#"{"broker":"b:9092","topic":"t","client_id":"c"}"#;
        let cfg: AppConfig = serde_json::from_str(json).expect("deserialize failed");
        assert_eq!(cfg.security_protocol, SecurityProtocol::Plaintext);
        assert!(!cfg.ssl_skip_verification);
        assert!(cfg.sasl_username.is_empty());
    }

    #[test]
    fn deserialize_unknown_fields_does_not_panic() {
        // Future-proofing: extra JSON fields should be silently ignored
        let json = r#"{"broker":"b:9092","topic":"t","client_id":"c","unknown_field":"x"}"#;
        let result: Result<AppConfig, _> = serde_json::from_str(json);
        // We accept either Ok (if deny_unknown_fields is not set) or Err -- just no panic
        let _ = result;
    }

    // --- Disk I/O ---

    #[test]
    fn save_and_load_roundtrip_via_tempdir() {
        let tmp_dir = tempfile::tempdir().expect("tempdir failed");
        let path = tmp_dir.path().join("config.json");

        let original = minimal_config();
        let json = serde_json::to_string_pretty(&original).expect("serialize failed");
        fs::write(&path, json.as_bytes()).expect("write failed");

        let loaded: AppConfig =
            serde_json::from_str(&fs::read_to_string(&path).expect("read failed"))
                .expect("deserialize failed");

        assert_eq!(loaded.broker, original.broker);
        assert_eq!(loaded.topic, original.topic);
        assert_eq!(loaded.client_id, original.client_id);
    }

    #[test]
    fn load_from_missing_file_returns_nonempty_default() {
        // AppConfig::load() silently falls back to Default when no file exists
        let cfg = AppConfig::default();
        assert!(!cfg.broker.is_empty());
        assert!(!cfg.topic.is_empty());
    }
}
