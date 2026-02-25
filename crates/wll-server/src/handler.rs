use axum::response::Json;
use serde_json::json;

use wll_protocol::HealthResponse;

/// Health check handler.
pub async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse::default())
}

/// Info handler.
pub async fn info_handler() -> Json<serde_json::Value> {
    Json(json!({
        "name": "wll-server",
        "version": env!("CARGO_PKG_VERSION"),
        "protocol_version": wll_protocol::PROTOCOL_VERSION,
    }))
}
