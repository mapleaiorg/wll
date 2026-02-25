use std::time::{Duration, Instant};

use wll_crypto::ContentHasher;
use wll_types::commitment::Decision;

use crate::config::GateConfig;
use crate::error::GateError;
use crate::stage::{CommitmentProposal, GateContext, GateStage, StageDecision, StageResult};
use crate::stages::{CapabilityStage, PolicyStage, ValidationStage};

// ---------------------------------------------------------------------------
// GateResult
// ---------------------------------------------------------------------------

/// The outcome of running a proposal through the full gate pipeline.
#[derive(Clone, Debug)]
pub struct GateResult {
    /// The final decision: accepted, rejected, or deferred.
    pub decision: Decision,
    /// BLAKE3 hash of the serialized policy configuration that was active.
    pub policy_hash: [u8; 32],
    /// Per-stage results in evaluation order.
    pub stage_results: Vec<StageResult>,
    /// Total wall-clock time for the pipeline evaluation.
    pub elapsed: Duration,
}

impl GateResult {
    /// Returns `true` if the proposal was accepted.
    pub fn is_accepted(&self) -> bool {
        self.decision.is_accepted()
    }
}

// ---------------------------------------------------------------------------
// CommitmentGate
// ---------------------------------------------------------------------------

/// The commitment gate: a configurable pipeline of stages that every
/// proposal must pass through before being accepted into the ledger.
///
/// The gate is the ONLY path to the ledger -- no bypass is possible.
pub struct CommitmentGate {
    stages: Vec<Box<dyn GateStage>>,
    config: GateConfig,
}

impl CommitmentGate {
    /// Create a new gate with the given configuration.
    ///
    /// Starts with an empty pipeline. Use [`Self::add_stage`] to add stages,
    /// or [`Self::with_default_stages`] for the standard pipeline.
    pub fn new(config: GateConfig) -> Self {
        Self {
            stages: Vec::new(),
            config,
        }
    }

    /// Create a gate with the default stage pipeline:
    /// Validation -> Capability -> Policy
    pub fn with_default_stages(config: GateConfig) -> Self {
        let mut gate = Self::new(config);
        gate.add_stage(Box::new(ValidationStage));
        gate.add_stage(Box::new(CapabilityStage));
        gate.add_stage(Box::new(PolicyStage));
        gate
    }

    /// Append a stage to the end of the pipeline.
    pub fn add_stage(&mut self, stage: Box<dyn GateStage>) {
        self.stages.push(stage);
    }

    /// The current configuration.
    pub fn config(&self) -> &GateConfig {
        &self.config
    }

    /// Number of stages in the pipeline.
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    /// Evaluate a proposal through the full pipeline.
    ///
    /// The pipeline is **fail-fast**: the first stage that fails stops
    /// evaluation and produces a `Rejected` decision. If all stages pass
    /// the decision is `Accepted`.
    pub fn evaluate(&self, proposal: &CommitmentProposal) -> Result<GateResult, GateError> {
        let pipeline_start = Instant::now();

        // Compute policy hash from the active configuration.
        let policy_hash = self.compute_policy_hash();

        // Build the shared context.
        let mut context = GateContext::minimal(proposal.proposer.clone());
        context.policies.push(self.config.default_policy.clone());

        // In permissive mode, skip all stage evaluations and accept.
        if self.config.permissive {
            return Ok(GateResult {
                decision: Decision::Accepted,
                policy_hash,
                stage_results: Vec::new(),
                elapsed: pipeline_start.elapsed(),
            });
        }

        let mut stage_results = Vec::with_capacity(self.stages.len());

        for stage in &self.stages {
            let stage_start = Instant::now();
            let decision = stage.evaluate(proposal, &context)?;
            let elapsed = stage_start.elapsed();

            let (passed, reason) = match &decision {
                StageDecision::Pass => (true, None),
                StageDecision::Fail { reason } => (false, Some(reason.clone())),
                StageDecision::Defer { reason, .. } => (false, Some(reason.clone())),
            };

            let result = StageResult {
                stage_name: stage.name().to_string(),
                passed,
                reason,
                elapsed,
            };

            stage_results.push(result.clone());
            context.previous_stages.push(result);

            // Fail-fast: stop on first failure.
            if let StageDecision::Fail { reason } = decision {
                return Ok(GateResult {
                    decision: Decision::Rejected { reason },
                    policy_hash,
                    stage_results,
                    elapsed: pipeline_start.elapsed(),
                });
            }

            // Defer: stop and report deferral.
            if let StageDecision::Defer { reason, .. } = decision {
                return Ok(GateResult {
                    decision: Decision::Rejected {
                        reason: format!("deferred: {reason}"),
                    },
                    policy_hash,
                    stage_results,
                    elapsed: pipeline_start.elapsed(),
                });
            }
        }

        Ok(GateResult {
            decision: Decision::Accepted,
            policy_hash,
            stage_results,
            elapsed: pipeline_start.elapsed(),
        })
    }

    /// Evaluate with an explicit context (for advanced use cases where the
    /// caller provides capabilities and policies).
    pub fn evaluate_with_context(
        &self,
        proposal: &CommitmentProposal,
        context: &mut GateContext,
    ) -> Result<GateResult, GateError> {
        let pipeline_start = Instant::now();
        let policy_hash = self.compute_policy_hash();

        if self.config.permissive {
            return Ok(GateResult {
                decision: Decision::Accepted,
                policy_hash,
                stage_results: Vec::new(),
                elapsed: pipeline_start.elapsed(),
            });
        }

        let mut stage_results = Vec::with_capacity(self.stages.len());

        for stage in &self.stages {
            let stage_start = Instant::now();
            let decision = stage.evaluate(proposal, context)?;
            let elapsed = stage_start.elapsed();

            let (passed, reason) = match &decision {
                StageDecision::Pass => (true, None),
                StageDecision::Fail { reason } => (false, Some(reason.clone())),
                StageDecision::Defer { reason, .. } => (false, Some(reason.clone())),
            };

            let result = StageResult {
                stage_name: stage.name().to_string(),
                passed,
                reason,
                elapsed,
            };

            stage_results.push(result.clone());
            context.previous_stages.push(result);

            if let StageDecision::Fail { reason } = decision {
                return Ok(GateResult {
                    decision: Decision::Rejected { reason },
                    policy_hash,
                    stage_results,
                    elapsed: pipeline_start.elapsed(),
                });
            }

            if let StageDecision::Defer { reason, .. } = decision {
                return Ok(GateResult {
                    decision: Decision::Rejected {
                        reason: format!("deferred: {reason}"),
                    },
                    policy_hash,
                    stage_results,
                    elapsed: pipeline_start.elapsed(),
                });
            }
        }

        Ok(GateResult {
            decision: Decision::Accepted,
            policy_hash,
            stage_results,
            elapsed: pipeline_start.elapsed(),
        })
    }

    /// Compute a BLAKE3 hash of the active policy configuration.
    fn compute_policy_hash(&self) -> [u8; 32] {
        let hasher = ContentHasher::new("wll-gate-policy-v1");
        match hasher.hash_json(&self.config.default_policy) {
            Ok(oid) => *oid.as_bytes(),
            Err(_) => [0u8; 32],
        }
    }
}
