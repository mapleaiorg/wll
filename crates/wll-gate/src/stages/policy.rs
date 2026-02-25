use serde::{Deserialize, Serialize};
use wll_types::{CommitmentClass, WorldlineId};

use crate::error::GateError;
use crate::stage::{CommitmentProposal, GateContext, GateStage, StageDecision};

// ---------------------------------------------------------------------------
// Policy types
// ---------------------------------------------------------------------------

/// A named policy containing one or more rules.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Policy {
    /// Unique identifier for this policy.
    pub id: String,
    /// Human-readable policy name.
    pub name: String,
    /// Ordered list of rules that make up this policy.
    pub rules: Vec<PolicyRule>,
    /// Scope that determines when this policy applies.
    pub applies_to: PolicyScope,
}

impl Policy {
    /// A maximally permissive policy that allows everything.
    pub fn permissive() -> Self {
        Self {
            id: "permissive".into(),
            name: "Permissive (allow all)".into(),
            rules: Vec::new(),
            applies_to: PolicyScope::All,
        }
    }

    /// Check whether this policy applies to the given proposal.
    pub fn applies(&self, proposal: &CommitmentProposal) -> bool {
        match &self.applies_to {
            PolicyScope::All => true,
            PolicyScope::Worldline(wid) => proposal.proposer == *wid,
            PolicyScope::Class(class) => proposal.class == *class,
            PolicyScope::Path(pattern) => proposal
                .targets
                .iter()
                .any(|t| t.starts_with(pattern.as_str())),
        }
    }
}

/// Individual rule within a policy.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PolicyRule {
    /// The proposer must hold this capability.
    RequireCapability(String),
    /// The proposal must include evidence.
    RequireEvidence,
    /// The proposal must be signed.
    RequireSignature,
    /// Maximum number of targets in a single commitment.
    MaxTargets(usize),
    /// Only these commitment classes are allowed.
    AllowedClasses(Vec<CommitmentClass>),
    /// These commitment classes are denied.
    DenyClasses(Vec<CommitmentClass>),
    /// Commits of this class require review (treated as fail without review flag).
    RequireReviewFor(CommitmentClass),
    /// Domain-specific custom rule.
    Custom {
        name: String,
        config: serde_json::Value,
    },
}

/// Scope controlling when a policy is evaluated.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PolicyScope {
    /// Applies to all proposals.
    All,
    /// Applies only to proposals from a specific worldline.
    Worldline(WorldlineId),
    /// Applies only to proposals of a specific class.
    Class(CommitmentClass),
    /// Applies to proposals targeting paths matching this prefix.
    Path(String),
}

// ---------------------------------------------------------------------------
// PolicyStage
// ---------------------------------------------------------------------------

/// Policy enforcement stage.
///
/// Evaluates every applicable policy against the proposal. All rules in all
/// applicable policies must pass for the stage to pass.
pub struct PolicyStage;

impl PolicyStage {
    /// Evaluate a single rule against a proposal and context.
    fn evaluate_rule(
        rule: &PolicyRule,
        proposal: &CommitmentProposal,
        context: &GateContext,
    ) -> Result<StageDecision, GateError> {
        match rule {
            PolicyRule::RequireCapability(cap_name) => {
                let held = context
                    .capabilities
                    .iter()
                    .any(|c| c.id.0 == *cap_name);
                if held {
                    Ok(StageDecision::Pass)
                } else {
                    Ok(StageDecision::Fail {
                        reason: format!("policy requires capability '{cap_name}'"),
                    })
                }
            }

            PolicyRule::RequireEvidence => {
                if proposal.evidence.is_empty() {
                    Ok(StageDecision::Fail {
                        reason: "policy requires evidence but none provided".into(),
                    })
                } else {
                    Ok(StageDecision::Pass)
                }
            }

            PolicyRule::RequireSignature => {
                if proposal.signature.is_none() {
                    Ok(StageDecision::Fail {
                        reason: "policy requires a cryptographic signature".into(),
                    })
                } else {
                    Ok(StageDecision::Pass)
                }
            }

            PolicyRule::MaxTargets(max) => {
                if proposal.targets.len() > *max {
                    Ok(StageDecision::Fail {
                        reason: format!(
                            "too many targets: {} exceeds maximum of {max}",
                            proposal.targets.len()
                        ),
                    })
                } else {
                    Ok(StageDecision::Pass)
                }
            }

            PolicyRule::AllowedClasses(allowed) => {
                if allowed.contains(&proposal.class) {
                    Ok(StageDecision::Pass)
                } else {
                    Ok(StageDecision::Fail {
                        reason: format!(
                            "commitment class '{}' is not in the allowed list",
                            proposal.class
                        ),
                    })
                }
            }

            PolicyRule::DenyClasses(denied) => {
                if denied.contains(&proposal.class) {
                    Ok(StageDecision::Fail {
                        reason: format!(
                            "commitment class '{}' is denied by policy",
                            proposal.class
                        ),
                    })
                } else {
                    Ok(StageDecision::Pass)
                }
            }

            PolicyRule::RequireReviewFor(class) => {
                if proposal.class == *class {
                    Ok(StageDecision::Fail {
                        reason: format!(
                            "commitment class '{}' requires review",
                            proposal.class
                        ),
                    })
                } else {
                    Ok(StageDecision::Pass)
                }
            }

            PolicyRule::Custom { name, .. } => {
                // Custom rules pass by default; real implementations would
                // delegate to a plugin system.
                tracing::debug!(rule = %name, "custom policy rule evaluated (default pass)");
                Ok(StageDecision::Pass)
            }
        }
    }
}

impl GateStage for PolicyStage {
    fn name(&self) -> &str {
        "policy"
    }

    fn evaluate(
        &self,
        proposal: &CommitmentProposal,
        context: &GateContext,
    ) -> Result<StageDecision, GateError> {
        for policy in &context.policies {
            if !policy.applies(proposal) {
                continue;
            }

            for rule in &policy.rules {
                let decision = Self::evaluate_rule(rule, proposal, context)?;
                if decision.is_fail() {
                    return Ok(decision);
                }
            }
        }

        Ok(StageDecision::Pass)
    }
}
