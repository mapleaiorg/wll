use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::stages::policy::Policy;

/// Configuration for the commitment gate pipeline.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GateConfig {
    /// Whether evidence is required on all proposals.
    pub require_evidence: bool,
    /// Whether cryptographic signatures are required.
    pub require_signatures: bool,
    /// The default policy applied when no other policies match.
    pub default_policy: Policy,
    /// Maximum wall-clock time allowed for the full pipeline.
    pub timeout: Duration,
    /// Maximum number of targets allowed per commitment.
    pub max_targets_per_commitment: usize,
    /// When `true`, the gate runs in permissive mode:
    /// all built-in stages pass without checks. This makes WLL behave like
    /// plain `git commit` for single-user local repositories.
    pub permissive: bool,
}

impl Default for GateConfig {
    fn default() -> Self {
        Self {
            require_evidence: false,
            require_signatures: false,
            default_policy: Policy::permissive(),
            timeout: Duration::from_secs(30),
            max_targets_per_commitment: 100,
            permissive: false,
        }
    }
}

impl GateConfig {
    /// A maximally permissive configuration for local single-user repos.
    ///
    /// This is the default for `wll init` — no evidence, no signatures,
    /// no policy enforcement. Governance features activate when the user
    /// explicitly configures them.
    pub fn permissive() -> Self {
        Self {
            permissive: true,
            ..Default::default()
        }
    }
}
