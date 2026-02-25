use async_trait::async_trait;
use crate::error::ServerResult;

#[derive(Clone, Debug)]
pub struct Identity {
    pub name: String,
    pub is_admin: bool,
}

impl Identity {
    pub fn anonymous() -> Self { Self { name: "anonymous".into(), is_admin: false } }
    pub fn user(name: impl Into<String>) -> Self { Self { name: name.into(), is_admin: false } }
    pub fn admin(name: impl Into<String>) -> Self { Self { name: name.into(), is_admin: true } }
}

#[derive(Clone, Debug)]
pub enum Credentials {
    Bearer(String),
    Anonymous,
}

#[derive(Clone, Debug)]
pub enum Action {
    Read { repo: String },
    Write { repo: String },
    Admin { repo: String },
    CreateRepo,
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { repo } => write!(f, "read:{repo}"),
            Self::Write { repo } => write!(f, "write:{repo}"),
            Self::Admin { repo } => write!(f, "admin:{repo}"),
            Self::CreateRepo => write!(f, "create-repo"),
        }
    }
}

#[async_trait]
pub trait AuthProvider: Send + Sync {
    async fn authenticate(&self, credentials: &Credentials) -> ServerResult<Identity>;
    async fn authorize(&self, identity: &Identity, action: &Action) -> ServerResult<bool>;
}

pub struct AllowAllAuth;

#[async_trait]
impl AuthProvider for AllowAllAuth {
    async fn authenticate(&self, credentials: &Credentials) -> ServerResult<Identity> {
        match credentials {
            Credentials::Bearer(token) => Ok(Identity::user(format!("bearer:{}", &token[..8.min(token.len())]))),
            Credentials::Anonymous => Ok(Identity::anonymous()),
        }
    }

    async fn authorize(&self, _identity: &Identity, _action: &Action) -> ServerResult<bool> {
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_variants() {
        let a = Identity::anonymous();
        assert_eq!(a.name, "anonymous");
        assert!(!a.is_admin);

        let u = Identity::user("alice");
        assert_eq!(u.name, "alice");
        assert!(!u.is_admin);

        let adm = Identity::admin("root");
        assert!(adm.is_admin);
    }

    #[test]
    fn action_display() {
        assert_eq!(format!("{}", Action::Read { repo: "r".into() }), "read:r");
        assert_eq!(format!("{}", Action::CreateRepo), "create-repo");
    }

    #[tokio::test]
    async fn allow_all_auth() {
        let auth = AllowAllAuth;
        let id = auth.authenticate(&Credentials::Anonymous).await.unwrap();
        assert_eq!(id.name, "anonymous");
        assert!(auth.authorize(&id, &Action::CreateRepo).await.unwrap());
    }

    #[tokio::test]
    async fn allow_all_bearer() {
        let auth = AllowAllAuth;
        let id = auth.authenticate(&Credentials::Bearer("mytoken123".into())).await.unwrap();
        assert!(id.name.starts_with("bearer:"));
    }
}
