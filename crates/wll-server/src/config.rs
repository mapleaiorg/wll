use std::net::SocketAddr;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerConfig {
    pub bind_addr: SocketAddr,
    pub repos_root: PathBuf,
    pub tls: Option<TlsConfig>,
    pub max_pack_size: u64,
    pub max_connections: usize,
    pub allow_anonymous_read: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:9418".parse().unwrap(),
            repos_root: PathBuf::from("."),
            tls: None,
            max_pack_size: 100 * 1024 * 1024,
            max_connections: 256,
            allow_anonymous_read: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TlsConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let c = ServerConfig::default();
        assert_eq!(c.bind_addr, "127.0.0.1:9418".parse::<SocketAddr>().unwrap());
        assert_eq!(c.max_pack_size, 100 * 1024 * 1024);
        assert_eq!(c.max_connections, 256);
        assert!(c.allow_anonymous_read);
        assert!(c.tls.is_none());
    }

    #[test]
    fn tls_config() {
        let tls = TlsConfig { cert_path: "cert.pem".into(), key_path: "key.pem".into() };
        assert_eq!(tls.cert_path, PathBuf::from("cert.pem"));
    }
}
