use axum::{Router, routing::get};
use crate::handler;

/// Build the axum router with all WLL endpoints.
pub fn build_router() -> Router {
    Router::new()
        .route("/v1/health", get(handler::health_handler))
        .route("/v1/info", get(handler::info_handler))
}
