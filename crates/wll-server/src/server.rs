use tokio::net::TcpListener;
use crate::config::ServerConfig;
use crate::error::ServerResult;
use crate::router::build_router;

/// WLL repository server.
pub struct WllServer {
    config: ServerConfig,
}

impl WllServer {
    pub fn new(config: ServerConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// Build the router (useful for testing).
    pub fn router(&self) -> axum::Router {
        build_router()
    }

    /// Start serving requests.
    pub async fn serve(self) -> ServerResult<()> {
        let app = build_router();
        let listener = TcpListener::bind(&self.config.bind_addr).await?;
        tracing::info!("WLL server listening on {}", self.config.bind_addr);
        axum::serve(listener, app)
            .await
            .map_err(|e| crate::error::ServerError::Internal(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_construction() {
        let server = WllServer::new(ServerConfig::default());
        assert_eq!(server.config().bind_addr, "127.0.0.1:9418".parse().unwrap());
    }

    #[test]
    fn router_builds() {
        let server = WllServer::new(ServerConfig::default());
        let _router = server.router();
    }
}
