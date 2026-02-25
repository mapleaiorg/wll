use crate::error::GateError;
use crate::stage::{CommitmentProposal, GateContext, GateStage, StageDecision};

/// Structural validation stage.
///
/// Checks that the proposal has all required fields populated and that basic
/// structural invariants hold (non-empty intent, at least one target, etc.).
pub struct ValidationStage;

impl GateStage for ValidationStage {
    fn name(&self) -> &str {
        "validation"
    }

    fn evaluate(
        &self,
        proposal: &CommitmentProposal,
        _context: &GateContext,
    ) -> Result<StageDecision, GateError> {
        // Intent must be non-empty.
        if proposal.intent.trim().is_empty() {
            return Ok(StageDecision::Fail {
                reason: "intent must not be empty".into(),
            });
        }

        // Must have at least one target.
        if proposal.targets.is_empty() {
            return Ok(StageDecision::Fail {
                reason: "commitment must have at least one target".into(),
            });
        }

        // Targets must be non-empty strings.
        for (i, target) in proposal.targets.iter().enumerate() {
            if target.trim().is_empty() {
                return Ok(StageDecision::Fail {
                    reason: format!("target at index {i} is empty"),
                });
            }
        }

        Ok(StageDecision::Pass)
    }
}
