use rskafka::client::partition::{Compression, OffsetAt, UnknownTopicHandling};
use rskafka::client::{ClientBuilder, Credentials, SaslConfig};
use rskafka::record::Record;
use chrono::Utc;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use serde::Serialize;
use std::io::BufReader;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::Mutex;

use crate::config::{AppConfig, SaslMechanism, SecurityProtocol};

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

/// Errors that can occur during Kafka operations
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

/// Custom certificate verifier that skips verification (insecure, for testing only)
#[derive(Debug)]
struct NoVerifier;

impl rustls::client::danger::ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// Kafka service for managing connections and sending messages
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

    /// Build a configured ClientBuilder with TLS and SASL based on security settings
    fn build_client_builder(config: &AppConfig) -> Result<ClientBuilder, KafkaError> {
        // Support comma-separated broker addresses
        let brokers: Vec<String> = config.broker
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if brokers.is_empty() {
            return Err(KafkaError::InvalidConfig("No broker addresses provided".to_string()));
        }
        let mut builder = ClientBuilder::new(brokers);

        // Configure TLS if needed
        match config.security_protocol {
            SecurityProtocol::Ssl | SecurityProtocol::SaslSsl => {
                let tls_config = Self::build_tls_config(config)?;
                builder = builder.tls_config(Arc::new(tls_config));
            }
            _ => {}
        }

        // Configure SASL if needed
        match config.security_protocol {
            SecurityProtocol::SaslPlaintext | SecurityProtocol::SaslSsl => {
                if config.sasl_username.is_empty() {
                    return Err(KafkaError::InvalidConfig(
                        "SASL username is required".to_string(),
                    ));
                }
                let credentials = Credentials::new(
                    config.sasl_username.clone(),
                    config.sasl_password.clone(),
                );
                let sasl = match config.sasl_mechanism {
                    SaslMechanism::Plain => SaslConfig::Plain(credentials),
                    SaslMechanism::ScramSha256 => SaslConfig::ScramSha256(credentials),
                    SaslMechanism::ScramSha512 => SaslConfig::ScramSha512(credentials),
                };
                builder = builder.sasl_config(sasl);
            }
            _ => {}
        }

        Ok(builder)
    }

    /// Build TLS configuration from AppConfig
    fn build_tls_config(config: &AppConfig) -> Result<rustls::ClientConfig, KafkaError> {
        // Ensure ring crypto provider is installed
        let _ = rustls::crypto::ring::default_provider().install_default();

        // Skip verification mode (insecure, for testing)
        if config.ssl_skip_verification {
            let tls_config = rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(NoVerifier))
                .with_no_client_auth();
            return Ok(tls_config);
        }

        let mut root_cert_store = rustls::RootCertStore::empty();

        if !config.ssl_ca_cert_path.is_empty() {
            // Load custom CA certificate
            let ca_data = std::fs::read(&config.ssl_ca_cert_path)
                .map_err(|e| KafkaError::InvalidConfig(format!("Failed to read CA cert: {}", e)))?;
            let mut reader = BufReader::new(ca_data.as_slice());
            let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut reader)
                .filter_map(|r| r.ok())
                .collect();
            for cert in certs {
                root_cert_store
                    .add(cert)
                    .map_err(|e| KafkaError::InvalidConfig(format!("Failed to add CA cert: {}", e)))?;
            }
        } else {
            // Use system native root certificates
            let native_certs = rustls_native_certs::load_native_certs();
            for cert in native_certs.certs {
                let _ = root_cert_store.add(cert);
            }
        }

        let builder = rustls::ClientConfig::builder()
            .with_root_certificates(root_cert_store);

        // Add client certificate (mTLS) if provided
        let tls_config = if !config.ssl_client_cert_path.is_empty()
            && !config.ssl_client_key_path.is_empty()
        {
            let cert_data = std::fs::read(&config.ssl_client_cert_path).map_err(|e| {
                KafkaError::InvalidConfig(format!("Failed to read client cert: {}", e))
            })?;
            let mut cert_reader = BufReader::new(cert_data.as_slice());
            let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_reader)
                .filter_map(|r| r.ok())
                .collect();

            let key_data = std::fs::read(&config.ssl_client_key_path).map_err(|e| {
                KafkaError::InvalidConfig(format!("Failed to read client key: {}", e))
            })?;
            let mut key_reader = BufReader::new(key_data.as_slice());
            let key: PrivateKeyDer<'static> = rustls_pemfile::private_key(&mut key_reader)
                .map_err(|e| {
                    KafkaError::InvalidConfig(format!("Failed to parse client key: {}", e))
                })?
                .ok_or_else(|| {
                    KafkaError::InvalidConfig("No private key found in key file".to_string())
                })?;

            builder
                .with_client_auth_cert(certs, key)
                .map_err(|e| {
                    KafkaError::InvalidConfig(format!("Failed to configure client auth: {}", e))
                })?
        } else {
            builder.with_no_client_auth()
        };

        Ok(tls_config)
    }

    /// Test connection to the Kafka broker with timeout
    pub async fn test_connection(&self, timeout_secs: u64) -> Result<bool, KafkaError> {
        // Clone config and release lock BEFORE async operation
        let config = {
            self.config.lock().await.clone()
        };

        let builder = Self::build_client_builder(&config)?;
        let connect_future = builder.build();

        match tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            connect_future
        ).await {
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

        // Build client builder with security config
        let builder = Self::build_client_builder(&config)?;

        // Wrap entire operation in a 10 second timeout
        let send_future = async {
            // Build client
            let client = builder
                .build()
                .await
                .map_err(|e| KafkaError::ConnectionFailed(e.to_string()))?;

            // Get partition client for topic (partition 0)
            let partition_client = client
                .partition_client(&topic, 0, UnknownTopicHandling::Error)
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

        let builder = Self::build_client_builder(&config)?;

        let create_future = async {
            let client = builder
                .build()
                .await
                .map_err(|e| KafkaError::ConnectionFailed(e.to_string()))?;

            let controller_client = client
                .controller_client()
                .map_err(|e| KafkaError::TopicCreateFailed(e.to_string()))?;

            controller_client
                .create_topic(
                    &topic_name,
                    num_partitions,
                    replication_factor,
                    5_000,
                )
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

        let builder = Self::build_client_builder(&config)?;

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
                .map_err(|e| KafkaError::ConsumeFailed(format!("Failed to get earliest offset: {}", e)))?;
            let latest = partition_client
                .get_offset(OffsetAt::Latest)
                .await
                .map_err(|e| KafkaError::ConsumeFailed(format!("Failed to get latest offset: {}", e)))?;

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
                .map(|record| {
                    ConsumedMessage {
                        offset: record.offset,
                        key: record.record.key.map(|k| String::from_utf8_lossy(&k).to_string()),
                        value: record.record.value.map(|v| String::from_utf8_lossy(&v).to_string()),
                        timestamp: record.record.timestamp.timestamp_millis(),
                    }
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
