use rskafka::client::{ClientBuilder, Credentials, SaslConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use std::io::BufReader;
use std::sync::Arc;

use crate::config::{AppConfig, SaslMechanism, SecurityProtocol};
use crate::connection::types::KafkaError;

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

/// Build a configured [`ClientBuilder`] with TLS and SASL based on the security settings
/// in the provided [`AppConfig`].
///
/// This is the single source of truth for Kafka client construction. Both the publisher
/// and the (upcoming) shaper engine use this function — no duplication of TLS/SASL logic.
pub fn build_client_builder(config: &AppConfig) -> Result<ClientBuilder, KafkaError> {
    // Support comma-separated broker addresses
    let brokers: Vec<String> = config
        .broker
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if brokers.is_empty() {
        return Err(KafkaError::InvalidConfig(
            "No broker addresses provided".to_string(),
        ));
    }

    let mut builder = ClientBuilder::new(brokers);

    // Configure TLS if needed
    match config.security_protocol {
        SecurityProtocol::Ssl | SecurityProtocol::SaslSsl => {
            let tls_config = build_tls_config(config)?;
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

/// Build a `rustls::ClientConfig` from the TLS-related fields in [`AppConfig`].
pub fn build_tls_config(config: &AppConfig) -> Result<rustls::ClientConfig, KafkaError> {
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

    let builder = rustls::ClientConfig::builder().with_root_certificates(root_cert_store);

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
            .map_err(|e| KafkaError::InvalidConfig(format!("Failed to configure client auth: {}", e)))?
    } else {
        builder.with_no_client_auth()
    };

    Ok(tls_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, SaslMechanism, SecurityProtocol};

    fn plaintext_config() -> AppConfig {
        AppConfig {
            broker: "localhost:9092".to_string(),
            security_protocol: SecurityProtocol::Plaintext,
            ..AppConfig::default()
        }
    }

    // --- build_client_builder ---

    #[test]
    fn plaintext_config_builds_ok() {
        // A minimal plaintext config should build a ClientBuilder without error.
        // We don't call .build() — that would require a live broker.
        let config = plaintext_config();
        let result = build_client_builder(&config);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn multi_broker_config_builds_ok() {
        let config = AppConfig {
            broker: "broker1:9092, broker2:9092".to_string(),
            security_protocol: SecurityProtocol::Plaintext,
            ..AppConfig::default()
        };
        assert!(build_client_builder(&config).is_ok());
    }

    #[test]
    fn empty_broker_returns_invalid_config() {
        let config = AppConfig {
            broker: "".to_string(),
            ..AppConfig::default()
        };
        let err = build_client_builder(&config).unwrap_err();
        assert!(
            matches!(err, KafkaError::InvalidConfig(_)),
            "Expected InvalidConfig, got: {:?}",
            err
        );
    }

    #[test]
    fn whitespace_only_broker_returns_invalid_config() {
        let config = AppConfig {
            broker: "  ,  ".to_string(),
            ..AppConfig::default()
        };
        let err = build_client_builder(&config).unwrap_err();
        assert!(matches!(err, KafkaError::InvalidConfig(_)));
    }

    #[test]
    fn sasl_plaintext_with_empty_username_returns_error() {
        let config = AppConfig {
            broker: "localhost:9092".to_string(),
            security_protocol: SecurityProtocol::SaslPlaintext,
            sasl_mechanism: SaslMechanism::Plain,
            sasl_username: "".to_string(),
            sasl_password: "secret".to_string(),
            ..AppConfig::default()
        };
        let err = build_client_builder(&config).unwrap_err();
        assert!(matches!(err, KafkaError::InvalidConfig(_)));
    }

    #[test]
    fn sasl_with_credentials_builds_ok() {
        let config = AppConfig {
            broker: "localhost:9092".to_string(),
            security_protocol: SecurityProtocol::SaslPlaintext,
            sasl_mechanism: SaslMechanism::Plain,
            sasl_username: "user".to_string(),
            sasl_password: "pass".to_string(),
            ..AppConfig::default()
        };
        assert!(build_client_builder(&config).is_ok());
    }

    // --- build_tls_config ---

    #[test]
    fn tls_skip_verify_builds_ok() {
        // Should succeed without any cert files at all
        let config = AppConfig {
            ssl_skip_verification: true,
            security_protocol: SecurityProtocol::Ssl,
            ..AppConfig::default()
        };
        let result = build_tls_config(&config);
        assert!(result.is_ok(), "Expected Ok for skip-verify mode, got: {:?}", result.err());
    }

    #[test]
    fn tls_with_nonexistent_ca_cert_returns_error() {
        let config = AppConfig {
            ssl_skip_verification: false,
            ssl_ca_cert_path: "/nonexistent/path/to/ca.pem".to_string(),
            security_protocol: SecurityProtocol::Ssl,
            ..AppConfig::default()
        };
        let err = build_tls_config(&config).unwrap_err();
        assert!(
            matches!(err, KafkaError::InvalidConfig(_)),
            "Expected InvalidConfig for missing CA cert file"
        );
    }
}
