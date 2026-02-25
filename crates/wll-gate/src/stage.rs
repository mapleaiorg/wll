use std::time::Duration;

use serde::{Deserialize, Serialize};
use wll_types::{Capability, WorldlineId};

use crate::error::GateError;
use crate::stages::policy::Policy;

// ---------------------------------------------------------------------------
// CommitmentProposal - defined locally since wll-ledger is not a dependency
// ---------------------------------------------------------------------------

/// A proposal to commit changes, evaluated by the gate pipeline.
///
/// This is a self-contained type that carries everything the gate needs to
/// make a decision. When wll-ledger is introduced, this may be re-exported
/// from there instead.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitmentProposal {
    /// Who is proposing this commitment.
    pub proposer: WorldlineId,
    /// Human-readable intent (commit message equivalent).
    pub intent: String,
    /// Classification of the change.
    pub class: wll_types::CommitmentClass,
    /// Targets affected by this commitment (file paths, object IDs, etc.).
    pub targets: Vec<String>,
    /// Evidence supporting the commitment.
    pub evidence: wll_types::EvidenceBundle,
    /// Capabilities the proposer claims for this operation.
    pub claimed_capabilities: Vec<String>,
    /// Optional cryptographic signature over the proposal content.
    pub signature: Option<Vec<u8>>,
}

impl CommitmentProposal {
    /// Create a minimal valid proposal for testing.
    pub fn minimal(proposer: WorldlineId, intent: impl Into<String>) -> Self {
        Self {
            proposer,
            intent: intent.into(),
            class: wll_types::CommitmentClass::ContentUpdate,
            targets: vec!["src/main.rs".into()],
            evidence: wll_types::EvidenceBundle::empty(),
            claimed_capabilities: Vec::new(),
            signature: None,
        }
    }
}

// ---------------------------------------------------------------------------
// StageDecision
// ---------------------------------------------------------------------------

/// The outcome of a single gate stage evaluation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StageDecision {
    /// The stage passed; proceed to the next stage.
    Pass,
    /// The stage failed; the proposal should be rejected.
    Fail { reason: String },
    /// The stage cannot decide now; retry later.
    Defer {
        reason: String,
        retry_after: Duration,
    },
}

impl StageDecision {
    /// Returns `true` if the decision is `Pass`.
    pub fn is_pass(&self) -> bool {
        matches!(self, Self::Pass)
    }

    /// Returns `true` if the decision is `Fail`.
    pub fn is_fail(&self) -> bool {
        matches!(self, Self::Fail { .. })
    }
}

// ---------------------------------------------------------------------------
// StageResult
// ---------------------------------------------------------------------------

/// Recorded result from a completed stage evaluation.
#[derive(Clone, Debug)]
pub struct StageResult {
    /// Name of the stage that produced this result.
    pub stage_name: String,
    /// Whether the stage passed.
    pub passed: bool,
    /// Optional reason (populated on failure or deferral).
    pub reason: Option<String>,
    /// Wall-clock time the stage took to evaluate.
    pub elapsed: Duration,
}

// ---------------------------------------------------------------------------
// GateContext
// ---------------------------------------------------------------------------

/// Contextual information available to every gate stage.
pub struct GateContext {
    /// The worldline being committed to.
    pub worldline: WorldlineId,
    /// Capabilities held by the proposer.
    pub capabilities: Vec<Capability>,
    /// Active policies that apply.
    pub policies: Vec<Policy>,
    /// Results from stages that have already run in this evaluation.
    pub previous_stages: Vec<StageResult>,
}

impl GateContext {
    /// Create a minimal context (useful for tests and permissive mode).
    pub fn minimal(worldline: WorldlineId) -> Self {
        Self {
            worldline,
            capabilities: Vec::new(),
            policies: Vec::new(),
            previous_stages: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// GateStage trait
// ---------------------------------------------------------------------------

/// A single evaluation stage in the gate pipeline.
///
/// Stages are evaluated in order. Each stage receives the proposal and a
/// shared context, and returns a pass/fail/defer decision.
///
/// The trait is object-safe and `Send + Sync` so stages can be stored in
/// a `Vec<Box<dyn GateStage>>`.
pub trait GateStage: Send + Sync {
    /// Human-readable name of this stage (e.g., "validation", "capability").
    fn name(&self) -> &str;

    /// Evaluate the proposal and return a decision.
    fn evaluate(
        &self,
        proposal: &CommitmentProposal,
        context: &GateContext,
    ) -> Result<StageDecision, GateError>;
}
