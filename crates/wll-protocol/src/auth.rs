use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Authentication method for connecting to a remote.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AuthMethod {
    Bearer(String),
    SshKey { key_path: PathBuf },
    MutualTls { cert_path: PathBuf, key_path: PathBuf },
    Anonymous,
}

impl Default for AuthMethod {
    fn default() -> Self { Self::Anonymous }
}

impl AuthMethod {
    pub fn is_authenticated(&self) -> bool {
        !matches!(self, Self::Anonymous)
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Bearer(_) => "bearer-token",
            Self::SshKey { .. } => "ssh-key",
            Self::MutualTls { .. } => "mutual-tls",
            Self::Anonymous => "anonymous",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anonymous_not_authenticated() {
        assert!(!AuthMethod::Anonymous.is_authenticated());
    }

    #[test]
    fn bearer_is_authenticated() {
        assert!(AuthMethod::Bearer("token".into()).is_authenticated());
    }

    #[test]
    fn display_names() {
        assert_eq!(AuthMethod::Anonymous.display_name(), "anonymous");
        assert_eq!(AuthMethod::Bearer("x".into()).display_name(), "bearer-token");
        assert_eq!(AuthMethod::SshKey { key_path: "k".into() }.display_name(), "ssh-key");
        assert_eq!(AuthMethod::MutualTls { cert_path: "c".into(), key_path: "k".into() }.display_name(), "mutual-tls");
    }

    #[test]
    fn default_is_anonymous() {
        assert!(matches!(AuthMethod::default(), AuthMethod::Anonymous));
    }
}
