//! Policy gate pipeline for the WorldLine Ledger.
//!
//! Every commitment proposal must pass through the gate before it can be
//! recorded in a worldline. The gate runs a configurable pipeline of stages
//! (validation, capability, policy, etc.) and produces a final accept/reject
//! decision with a full audit trail.
//!
//! # Quick Start
//!
//! ```rust
//! use wll_gate::{CommitmentGate, CommitmentProposal, GateConfig};
//! use wll_types::{IdentityMaterial, WorldlineId};
//!
//! let gate = CommitmentGate::with_default_stages(GateConfig::default());
//! let proposer = WorldlineId::derive(&IdentityMaterial::GenesisHash([1u8; 32]));
//! let proposal = CommitmentProposal::minimal(proposer, "fix: correct off-by-one");
//! let result = gate.evaluate(&proposal).unwrap();
//! assert!(result.is_accepted());
//! ```

pub mod config;
pub mod error;
pub mod gate;
pub mod stage;
pub mod stages;

// Re-exports for convenience.
pub use config::GateConfig;
pub use error::GateError;
pub use gate::{CommitmentGate, GateResult};
pub use stage::{CommitmentProposal, GateContext, GateStage, StageDecision, StageResult};
pub use stages::capability::CapabilityStage;
pub use stages::policy::{Policy, PolicyRule, PolicyScope, PolicyStage};
pub use stages::validation::ValidationStage;

#[cfg(test)]
mod tests {
    use super::*;
    use wll_types::{
        Capability, CapabilityId, CapabilityScope, CommitmentClass, EvidenceBundle,
        IdentityMaterial, TemporalAnchor, WorldlineId,
    };

    /// Helper: create a test proposer.
    fn test_proposer() -> WorldlineId {
        WorldlineId::derive(&IdentityMaterial::GenesisHash([42u8; 32]))
    }

    /// Helper: create a valid minimal proposal.
    fn valid_proposal() -> CommitmentProposal {
        CommitmentProposal::minimal(test_proposer(), "feat: add user authentication")
    }

    // -----------------------------------------------------------------------
    // 1. Default gate passes valid proposals
    // -----------------------------------------------------------------------
    #[test]
    fn default_gate_passes_valid_proposal() {
        let gate = CommitmentGate::with_default_stages(GateConfig::default());
        let result = gate.evaluate(&valid_proposal()).unwrap();
        assert!(result.is_accepted());
        assert_eq!(result.stage_results.len(), 3); // validation, capability, policy
        assert!(result.stage_results.iter().all(|r| r.passed));
    }

    // -----------------------------------------------------------------------
    // 2. Validation catches empty intent
    // -----------------------------------------------------------------------
    #[test]
    fn validation_rejects_empty_intent() {
        let gate = CommitmentGate::with_default_stages(GateConfig::default());
        let mut proposal = valid_proposal();
        proposal.intent = String::new();
        let result = gate.evaluate(&proposal).unwrap();
        assert!(!result.is_accepted());
        assert_eq!(result.stage_results.len(), 1); // fail-fast at validation
        assert!(!result.stage_results[0].passed);
    }

    // -----------------------------------------------------------------------
    // 3. Validation catches whitespace-only intent
    // -----------------------------------------------------------------------
    #[test]
    fn validation_rejects_whitespace_intent() {
        let gate = CommitmentGate::with_default_stages(GateConfig::default());
        let mut proposal = valid_proposal();
        proposal.intent = "   \t\n  ".into();
        let result = gate.evaluate(&proposal).unwrap();
        assert!(!result.is_accepted());
    }

    // -----------------------------------------------------------------------
    // 4. Validation catches empty targets
    // -----------------------------------------------------------------------
    #[test]
    fn validation_rejects_empty_targets() {
        let gate = CommitmentGate::with_default_stages(GateConfig::default());
        let mut proposal = valid_proposal();
        proposal.targets.clear();
        let result = gate.evaluate(&proposal).unwrap();
        assert!(!result.is_accepted());
        let reason = result.stage_results[0].reason.as_deref().unwrap();
        assert!(reason.contains("at least one target"));
    }

    // -----------------------------------------------------------------------
    // 5. Validation catches blank target entries
    // -----------------------------------------------------------------------
    #[test]
    fn validation_rejects_blank_target_entry() {
        let gate = CommitmentGate::with_default_stages(GateConfig::default());
        let mut proposal = valid_proposal();
        proposal.targets = vec!["src/main.rs".into(), "  ".into()];
        let result = gate.evaluate(&proposal).unwrap();
        assert!(!result.is_accepted());
    }

    // -----------------------------------------------------------------------
    // 6. Capability stage rejects missing capabilities
    // -----------------------------------------------------------------------
    #[test]
    fn capability_rejects_missing_capability() {
        let gate = CommitmentGate::with_default_stages(GateConfig::default());
        let mut proposal = valid_proposal();
        proposal.claimed_capabilities = vec!["deploy".into()];
        // Context has no capabilities, so "deploy" is not held.
        let result = gate.evaluate(&proposal).unwrap();
        assert!(!result.is_accepted());
        // Should fail at the capability stage (index 1).
        let cap_result = &result.stage_results[1];
        assert!(!cap_result.passed);
        assert!(cap_result.reason.as_deref().unwrap().contains("deploy"));
    }

    // -----------------------------------------------------------------------
    // 7. Capability stage passes when cap is held
    // -----------------------------------------------------------------------
    #[test]
    fn capability_passes_when_cap_held() {
        let gate = CommitmentGate::with_default_stages(GateConfig::default());
        let mut proposal = valid_proposal();
        proposal.claimed_capabilities = vec!["write".into()];

        let mut context = GateContext::minimal(proposal.proposer.clone());
        context.capabilities.push(Capability {
            id: CapabilityId("write".into()),
            scope: CapabilityScope::Global,
            granted_at: TemporalAnchor::zero(),
            expires_at: None,
        });
        context.policies.push(Policy::permissive());

        let result = gate.evaluate_with_context(&proposal, &mut context).unwrap();
        assert!(result.is_accepted());
    }

    // -----------------------------------------------------------------------
    // 8. Policy enforcement: RequireEvidence
    // -----------------------------------------------------------------------
    #[test]
    fn policy_requires_evidence() {
        let mut gate = CommitmentGate::new(GateConfig::default());
        gate.add_stage(Box::new(ValidationStage));
        gate.add_stage(Box::new(PolicyStage));

        let proposal = valid_proposal();
        let policy = Policy {
            id: "strict".into(),
            name: "Strict policy".into(),
            rules: vec![PolicyRule::RequireEvidence],
            applies_to: PolicyScope::All,
        };

        let mut context = GateContext::minimal(proposal.proposer.clone());
        context.policies.push(policy);

        let result = gate.evaluate_with_context(&proposal, &mut context).unwrap();
        assert!(!result.is_accepted());
        // Policy stage should report the failure.
        let policy_result = result.stage_results.iter().find(|r| r.stage_name == "policy").unwrap();
        assert!(!policy_result.passed);
        assert!(policy_result.reason.as_deref().unwrap().contains("evidence"));
    }

    // -----------------------------------------------------------------------
    // 9. Policy enforcement: MaxTargets
    // -----------------------------------------------------------------------
    #[test]
    fn policy_max_targets() {
        let mut gate = CommitmentGate::new(GateConfig::default());
        gate.add_stage(Box::new(ValidationStage));
        gate.add_stage(Box::new(PolicyStage));

        let mut proposal = valid_proposal();
        proposal.targets = (0..10).map(|i| format!("file_{i}.rs")).collect();

        let policy = Policy {
            id: "limited".into(),
            name: "Limited targets".into(),
            rules: vec![PolicyRule::MaxTargets(5)],
            applies_to: PolicyScope::All,
        };

        let mut context = GateContext::minimal(proposal.proposer.clone());
        context.policies.push(policy);

        let result = gate.evaluate_with_context(&proposal, &mut context).unwrap();
        assert!(!result.is_accepted());
    }

    // -----------------------------------------------------------------------
    // 10. Policy enforcement: DenyClasses
    // -----------------------------------------------------------------------
    #[test]
    fn policy_deny_classes() {
        let mut gate = CommitmentGate::new(GateConfig::default());
        gate.add_stage(Box::new(PolicyStage));

        let mut proposal = valid_proposal();
        proposal.class = CommitmentClass::PolicyChange;

        let policy = Policy {
            id: "no-policy-changes".into(),
            name: "Deny policy changes".into(),
            rules: vec![PolicyRule::DenyClasses(vec![CommitmentClass::PolicyChange])],
            applies_to: PolicyScope::All,
        };

        let mut context = GateContext::minimal(proposal.proposer.clone());
        context.policies.push(policy);

        let result = gate.evaluate_with_context(&proposal, &mut context).unwrap();
        assert!(!result.is_accepted());
    }

    // -----------------------------------------------------------------------
    // 11. Pipeline fail-fast: stops at first failure
    // -----------------------------------------------------------------------
    #[test]
    fn pipeline_is_fail_fast() {
        let gate = CommitmentGate::with_default_stages(GateConfig::default());
        let mut proposal = valid_proposal();
        proposal.intent = String::new(); // will fail validation
        proposal.claimed_capabilities = vec!["admin".into()]; // would also fail capability

        let result = gate.evaluate(&proposal).unwrap();
        assert!(!result.is_accepted());
        // Only validation ran (fail-fast); capability + policy did not.
        assert_eq!(result.stage_results.len(), 1);
        assert_eq!(result.stage_results[0].stage_name, "validation");
    }

    // -----------------------------------------------------------------------
    // 12. Permissive mode accepts everything
    // -----------------------------------------------------------------------
    #[test]
    fn permissive_mode_accepts_all() {
        let config = GateConfig::permissive();
        let gate = CommitmentGate::with_default_stages(config);

        // Even a proposal that would normally fail passes in permissive mode.
        let mut proposal = valid_proposal();
        proposal.intent = String::new(); // would fail validation
        let result = gate.evaluate(&proposal).unwrap();
        assert!(result.is_accepted());
        // No stages were executed.
        assert!(result.stage_results.is_empty());
    }

    // -----------------------------------------------------------------------
    // 13. Policy with AllowedClasses
    // -----------------------------------------------------------------------
    #[test]
    fn policy_allowed_classes_rejects_unlisted() {
        let mut gate = CommitmentGate::new(GateConfig::default());
        gate.add_stage(Box::new(PolicyStage));

        let mut proposal = valid_proposal();
        proposal.class = CommitmentClass::StructuralChange;

        let policy = Policy {
            id: "content-only".into(),
            name: "Content only".into(),
            rules: vec![PolicyRule::AllowedClasses(vec![
                CommitmentClass::ReadOnly,
                CommitmentClass::ContentUpdate,
            ])],
            applies_to: PolicyScope::All,
        };

        let mut context = GateContext::minimal(proposal.proposer.clone());
        context.policies.push(policy);

        let result = gate.evaluate_with_context(&proposal, &mut context).unwrap();
        assert!(!result.is_accepted());
    }

    // -----------------------------------------------------------------------
    // 14. Policy scoped to a specific class only applies to that class
    // -----------------------------------------------------------------------
    #[test]
    fn policy_scope_class_filtering() {
        let mut gate = CommitmentGate::new(GateConfig::default());
        gate.add_stage(Box::new(ValidationStage));
        gate.add_stage(Box::new(PolicyStage));

        let proposal = valid_proposal(); // class = ContentUpdate

        // This policy only applies to PolicyChange, so it should NOT trigger.
        let policy = Policy {
            id: "policy-guard".into(),
            name: "Guard policy changes".into(),
            rules: vec![PolicyRule::RequireSignature],
            applies_to: PolicyScope::Class(CommitmentClass::PolicyChange),
        };

        let mut context = GateContext::minimal(proposal.proposer.clone());
        context.policies.push(policy);

        let result = gate.evaluate_with_context(&proposal, &mut context).unwrap();
        assert!(result.is_accepted());
    }

    // -----------------------------------------------------------------------
    // 15. RequireSignature policy rejects unsigned proposals
    // -----------------------------------------------------------------------
    #[test]
    fn policy_require_signature() {
        let mut gate = CommitmentGate::new(GateConfig::default());
        gate.add_stage(Box::new(PolicyStage));

        let proposal = valid_proposal(); // signature = None

        let policy = Policy {
            id: "signed".into(),
            name: "Require signature".into(),
            rules: vec![PolicyRule::RequireSignature],
            applies_to: PolicyScope::All,
        };

        let mut context = GateContext::minimal(proposal.proposer.clone());
        context.policies.push(policy);

        let result = gate.evaluate_with_context(&proposal, &mut context).unwrap();
        assert!(!result.is_accepted());
    }

    // -----------------------------------------------------------------------
    // 16. Custom stage integration
    // -----------------------------------------------------------------------
    #[test]
    fn custom_stage_integration() {
        struct AlwaysFailStage;
        impl GateStage for AlwaysFailStage {
            fn name(&self) -> &str {
                "always-fail"
            }
            fn evaluate(
                &self,
                _proposal: &CommitmentProposal,
                _context: &GateContext,
            ) -> Result<StageDecision, GateError> {
                Ok(StageDecision::Fail {
                    reason: "custom stage says no".into(),
                })
            }
        }

        let mut gate = CommitmentGate::new(GateConfig::default());
        gate.add_stage(Box::new(ValidationStage));
        gate.add_stage(Box::new(AlwaysFailStage));
        gate.add_stage(Box::new(PolicyStage)); // should never run

        let result = gate.evaluate(&valid_proposal()).unwrap();
        assert!(!result.is_accepted());
        assert_eq!(result.stage_results.len(), 2); // validation + always-fail
        assert_eq!(result.stage_results[1].stage_name, "always-fail");
    }

    // -----------------------------------------------------------------------
    // 17. Evidence provided passes RequireEvidence policy
    // -----------------------------------------------------------------------
    #[test]
    fn policy_passes_when_evidence_provided() {
        let mut gate = CommitmentGate::new(GateConfig::default());
        gate.add_stage(Box::new(ValidationStage));
        gate.add_stage(Box::new(PolicyStage));

        let mut proposal = valid_proposal();
        proposal.evidence = EvidenceBundle::from_references(vec!["issue://PROJ-42".into()]);

        let policy = Policy {
            id: "strict".into(),
            name: "Strict".into(),
            rules: vec![PolicyRule::RequireEvidence],
            applies_to: PolicyScope::All,
        };

        let mut context = GateContext::minimal(proposal.proposer.clone());
        context.policies.push(policy);

        let result = gate.evaluate_with_context(&proposal, &mut context).unwrap();
        assert!(result.is_accepted());
    }

    // -----------------------------------------------------------------------
    // 18. Empty pipeline accepts (no stages = no objections)
    // -----------------------------------------------------------------------
    #[test]
    fn empty_pipeline_accepts() {
        let gate = CommitmentGate::new(GateConfig::default());
        let result = gate.evaluate(&valid_proposal()).unwrap();
        assert!(result.is_accepted());
        assert!(result.stage_results.is_empty());
    }

    // -----------------------------------------------------------------------
    // 19. RequireReviewFor triggers on matching class
    // -----------------------------------------------------------------------
    #[test]
    fn policy_require_review_for_class() {
        let mut gate = CommitmentGate::new(GateConfig::default());
        gate.add_stage(Box::new(PolicyStage));

        let mut proposal = valid_proposal();
        proposal.class = CommitmentClass::IdentityOperation;

        let policy = Policy {
            id: "review".into(),
            name: "Review identity ops".into(),
            rules: vec![PolicyRule::RequireReviewFor(
                CommitmentClass::IdentityOperation,
            )],
            applies_to: PolicyScope::All,
        };

        let mut context = GateContext::minimal(proposal.proposer.clone());
        context.policies.push(policy);

        let result = gate.evaluate_with_context(&proposal, &mut context).unwrap();
        assert!(!result.is_accepted());
    }

    // -----------------------------------------------------------------------
    // 20. GateResult contains a non-zero policy hash
    // -----------------------------------------------------------------------
    #[test]
    fn gate_result_has_policy_hash() {
        let gate = CommitmentGate::with_default_stages(GateConfig::default());
        let result = gate.evaluate(&valid_proposal()).unwrap();
        assert_ne!(result.policy_hash, [0u8; 32]);
    }

    // -----------------------------------------------------------------------
    // 21. Stage count reflects added stages
    // -----------------------------------------------------------------------
    #[test]
    fn stage_count() {
        let mut gate = CommitmentGate::new(GateConfig::default());
        assert_eq!(gate.stage_count(), 0);
        gate.add_stage(Box::new(ValidationStage));
        assert_eq!(gate.stage_count(), 1);
        gate.add_stage(Box::new(CapabilityStage));
        gate.add_stage(Box::new(PolicyStage));
        assert_eq!(gate.stage_count(), 3);
    }

    // -----------------------------------------------------------------------
    // 22. Policy RequireCapability through PolicyStage
    // -----------------------------------------------------------------------
    #[test]
    fn policy_require_capability_rule() {
        let mut gate = CommitmentGate::new(GateConfig::default());
        gate.add_stage(Box::new(PolicyStage));

        let proposal = valid_proposal();

        let policy = Policy {
            id: "admin-only".into(),
            name: "Admin only".into(),
            rules: vec![PolicyRule::RequireCapability("admin".into())],
            applies_to: PolicyScope::All,
        };

        let mut context = GateContext::minimal(proposal.proposer.clone());
        context.policies.push(policy);

        // No capabilities held -> should fail.
        let result = gate.evaluate_with_context(&proposal, &mut context).unwrap();
        assert!(!result.is_accepted());

        // Now grant the capability.
        let mut context2 = GateContext::minimal(proposal.proposer.clone());
        context2.policies.push(Policy {
            id: "admin-only".into(),
            name: "Admin only".into(),
            rules: vec![PolicyRule::RequireCapability("admin".into())],
            applies_to: PolicyScope::All,
        });
        context2.capabilities.push(Capability {
            id: CapabilityId("admin".into()),
            scope: CapabilityScope::Global,
            granted_at: TemporalAnchor::zero(),
            expires_at: None,
        });

        let result2 = gate.evaluate_with_context(&proposal, &mut context2).unwrap();
        assert!(result2.is_accepted());
    }
}
