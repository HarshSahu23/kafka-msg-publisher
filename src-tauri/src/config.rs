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

/// Application configuration for Kafka connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub broker: String,
    pub topic: String,
    pub client_id: String,
    #[serde(default)]
    pub security_protocol: SecurityProtocol,
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
