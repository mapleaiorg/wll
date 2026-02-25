use async_trait::async_trait;
use crate::error::ServerResult;

#[derive(Clone, Debug)]
pub struct HookRefUpdate {
    pub name: String,
    pub old_hash: Option<[u8; 32]>,
    pub new_hash: [u8; 32],
}

#[derive(Clone, Debug)]
pub enum HookResult {
    Allow,
    Reject { reason: String },
}

#[async_trait]
pub trait ServerHook: Send + Sync {
    async fn pre_receive(&self, updates: &[HookRefUpdate]) -> ServerResult<Vec<HookResult>>;
    async fn post_receive(&self, updates: &[HookRefUpdate]) -> ServerResult<()>;
}

pub struct NoOpHook;

#[async_trait]
impl ServerHook for NoOpHook {
    async fn pre_receive(&self, updates: &[HookRefUpdate]) -> ServerResult<Vec<HookResult>> {
        Ok(updates.iter().map(|_| HookResult::Allow).collect())
    }

    async fn post_receive(&self, _updates: &[HookRefUpdate]) -> ServerResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn noop_hook_allows() {
        let hook = NoOpHook;
        let updates = vec![HookRefUpdate { name: "main".into(), old_hash: None, new_hash: [1; 32] }];
        let results = hook.pre_receive(&updates).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0], HookResult::Allow));
    }

    #[tokio::test]
    async fn noop_hook_post_receive() {
        let hook = NoOpHook;
        hook.post_receive(&[]).await.unwrap();
    }
}
