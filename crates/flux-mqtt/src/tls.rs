use anyhow::{Context, Result};
use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;

/// TLS 配置
#[derive(Debug, Clone)]
pub struct TlsConfig {
    /// 证书文件路径
    pub cert_path: String,
    /// 私钥文件路径
    pub key_path: String,
    /// 客户端认证（可选）
    pub client_auth: bool,
    /// CA 证书路径（用于客户端认证）
    pub ca_cert_path: Option<String>,
}

impl TlsConfig {
    pub fn new(cert_path: String, key_path: String) -> Self {
        Self {
            cert_path,
            key_path,
            client_auth: false,
            ca_cert_path: None,
        }
    }

    pub fn with_client_auth(mut self, ca_cert_path: String) -> Self {
        self.client_auth = true;
        self.ca_cert_path = Some(ca_cert_path);
        self
    }
}

/// 加载 TLS 配置
pub fn load_tls_config(config: &TlsConfig) -> Result<Arc<ServerConfig>> {
    // 1. 加载证书链
    let cert_file = File::open(&config.cert_path)
        .context(format!("Failed to open cert file: {}", config.cert_path))?;
    let mut cert_reader = BufReader::new(cert_file);
    let cert_chain = certs(&mut cert_reader)
        .context("Failed to parse certificate")?
        .into_iter()
        .map(Certificate)
        .collect();

    // 2. 加载私钥
    let key_file = File::open(&config.key_path)
        .context(format!("Failed to open key file: {}", config.key_path))?;
    let mut key_reader = BufReader::new(key_file);
    let mut keys = pkcs8_private_keys(&mut key_reader)
        .context("Failed to parse private key")?;

    if keys.is_empty() {
        anyhow::bail!("No private key found in {}", config.key_path);
    }

    let private_key = PrivateKey(keys.remove(0));

    // 3. 构建 ServerConfig
    let tls_config = if config.client_auth {
        // 如果启用客户端认证，使用专门的配置函数
        if let Some(ca_path) = &config.ca_cert_path {
            load_client_auth_config(ca_path, cert_chain, private_key)?
        } else {
            ServerConfig::builder()
                .with_safe_defaults()
                .with_no_client_auth()
                .with_single_cert(cert_chain, private_key)
                .context("Failed to build TLS config")?
        }
    } else {
        ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(cert_chain, private_key)
            .context("Failed to build TLS config")?
    };
    
    let mut tls_config = tls_config;

    // 5. 配置 ALPN 协议（MQTT）
    tls_config.alpn_protocols = vec![b"mqtt".to_vec()];

    Ok(Arc::new(tls_config))
}

/// 加载客户端认证配置
fn load_client_auth_config(
    ca_path: &str,
    cert_chain: Vec<Certificate>,
    private_key: PrivateKey,
) -> Result<ServerConfig> {
    use rustls::server::AllowAnyAuthenticatedClient;
    use rustls::RootCertStore;

    // 加载 CA 证书
    let ca_file = File::open(ca_path)
        .context(format!("Failed to open CA cert file: {}", ca_path))?;
    let mut ca_reader = BufReader::new(ca_file);
    let ca_certs = certs(&mut ca_reader)
        .context("Failed to parse CA certificate")?;

    let mut root_store = RootCertStore::empty();
    for cert in ca_certs {
        root_store
            .add(&Certificate(cert))
            .context("Failed to add CA certificate to root store")?;
    }

    let client_auth = AllowAnyAuthenticatedClient::new(root_store);

    ServerConfig::builder()
        .with_safe_defaults()
        .with_client_cert_verifier(Arc::new(client_auth))
        .with_single_cert(cert_chain, private_key)
        .context("Failed to build TLS config with client auth")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tls_config_creation() {
        let config = TlsConfig::new(
            "cert.pem".to_string(),
            "key.pem".to_string(),
        );
        assert_eq!(config.cert_path, "cert.pem");
        assert_eq!(config.key_path, "key.pem");
        assert!(!config.client_auth);
    }

    #[test]
    fn test_tls_config_with_client_auth() {
        let config = TlsConfig::new(
            "cert.pem".to_string(),
            "key.pem".to_string(),
        )
        .with_client_auth("ca.pem".to_string());

        assert!(config.client_auth);
        assert_eq!(config.ca_cert_path, Some("ca.pem".to_string()));
    }
}
