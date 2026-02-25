//! HTTP server for the WorldLine Ledger.
//!
//! Hosts remote WLL repositories over HTTP/2 with authentication,
//! server-side hooks, and policy enforcement.

pub mod auth;
pub mod config;
pub mod error;
pub mod handler;
pub mod hooks;
pub mod router;
pub mod server;

pub use auth::{Action, AllowAllAuth, AuthProvider, Credentials, Identity};
pub use config::{ServerConfig, TlsConfig};
pub use error::{ServerError, ServerResult};
pub use hooks::{HookRefUpdate, HookResult, NoOpHook, ServerHook};
pub use server::WllServer;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn health_endpoint() {
        let app = router::build_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
    }

    #[tokio::test]
    async fn info_endpoint() {
        let app = router::build_router();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/info")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
    }
}
