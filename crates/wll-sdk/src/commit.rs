use serde::{Deserialize, Serialize};
use wll_types::{CommitmentClass, ObjectId};
use wll_ledger::{CommitmentReceipt, OutcomeReceipt};

/// Simplified commit proposal for SDK users.
#[derive(Clone, Debug)]
pub struct CommitProposal {
    pub message: String,
    pub intent: Option<String>,
    pub class: Option<CommitmentClass>,
    pub evidence: Vec<String>,
    pub tree: Option<ObjectId>,
}

impl CommitProposal {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            intent: None,
            class: None,
            evidence: Vec::new(),
            tree: None,
        }
    }

    pub fn with_intent(mut self, intent: impl Into<String>) -> Self {
        self.intent = Some(intent.into());
        self
    }

    pub fn with_class(mut self, class: CommitmentClass) -> Self {
        self.class = Some(class);
        self
    }

    pub fn with_evidence(mut self, uri: impl Into<String>) -> Self {
        self.evidence.push(uri.into());
        self
    }

    pub fn with_tree(mut self, tree: ObjectId) -> Self {
        self.tree = Some(tree);
        self
    }

    pub fn effective_intent(&self) -> &str {
        self.intent.as_deref().unwrap_or(&self.message)
    }

    pub fn effective_class(&self) -> CommitmentClass {
        self.class.clone().unwrap_or(CommitmentClass::ContentUpdate)
    }
}

/// Result of a commit operation.
#[derive(Clone, Debug)]
pub struct CommitResult {
    pub commitment_receipt: CommitmentReceipt,
    pub outcome_receipt: OutcomeReceipt,
    pub receipt_hash: [u8; 32],
}

/// Summary of a receipt for log display.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReceiptSummary {
    pub seq: u64,
    pub receipt_hash: [u8; 32],
    pub kind: String,
    pub intent: Option<String>,
    pub accepted: Option<bool>,
    pub timestamp_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal_builder() {
        let p = CommitProposal::new("test")
            .with_intent("my intent")
            .with_class(CommitmentClass::PolicyChange)
            .with_evidence("issue://1");
        assert_eq!(p.message, "test");
        assert_eq!(p.effective_intent(), "my intent");
        assert_eq!(p.effective_class(), CommitmentClass::PolicyChange);
        assert_eq!(p.evidence, vec!["issue://1"]);
    }

    #[test]
    fn effective_intent_fallback() {
        let p = CommitProposal::new("fallback message");
        assert_eq!(p.effective_intent(), "fallback message");
    }

    #[test]
    fn effective_class_default() {
        let p = CommitProposal::new("x");
        assert_eq!(p.effective_class(), CommitmentClass::ContentUpdate);
    }
}
