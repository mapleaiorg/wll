use wll_types::TemporalAnchor;

use crate::error::GateError;
use crate::stage::{CommitmentProposal, GateContext, GateStage, StageDecision};

/// Capability verification stage.
///
/// Checks that the proposer holds every capability they have claimed in the
/// proposal. Capabilities are matched by ID and must not be expired.
pub struct CapabilityStage;

impl GateStage for CapabilityStage {
    fn name(&self) -> &str {
        "capability"
    }

    fn evaluate(
        &self,
        proposal: &CommitmentProposal,
        context: &GateContext,
    ) -> Result<StageDecision, GateError> {
        if proposal.claimed_capabilities.is_empty() {
            // Nothing claimed, nothing to verify.
            return Ok(StageDecision::Pass);
        }

        let now = TemporalAnchor::now(0);

        for claimed in &proposal.claimed_capabilities {
            let held = context.capabilities.iter().any(|cap| {
                cap.id.0 == *claimed && !cap.is_expired_at(&now)
            });
            if !held {
                return Ok(StageDecision::Fail {
                    reason: format!("proposer lacks required capability: {claimed}"),
                });
            }
        }

        Ok(StageDecision::Pass)
    }
}
