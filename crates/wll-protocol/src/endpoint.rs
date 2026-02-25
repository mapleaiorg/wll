/// HTTP endpoint paths for the WLL protocol.
pub mod endpoints {
    pub const INFO_REFS: &str = "/v1/info/refs";
    pub const FETCH: &str = "/v1/fetch";
    pub const PUSH: &str = "/v1/push";
    pub const RECEIPT_QUERY: &str = "/v1/receipt/query";
    pub const OBJECT: &str = "/v1/object";
    pub const HEALTH: &str = "/v1/health";
}

/// Health check response.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub protocol_version: u32,
}

impl Default for HealthResponse {
    fn default() -> Self {
        Self {
            status: "ok".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            protocol_version: super::message::PROTOCOL_VERSION,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_response_defaults() {
        let h = HealthResponse::default();
        assert_eq!(h.status, "ok");
        assert_eq!(h.protocol_version, 1);
    }

    #[test]
    fn endpoint_paths() {
        assert_eq!(endpoints::HEALTH, "/v1/health");
        assert_eq!(endpoints::INFO_REFS, "/v1/info/refs");
        assert_eq!(endpoints::FETCH, "/v1/fetch");
        assert_eq!(endpoints::PUSH, "/v1/push");
    }
}
