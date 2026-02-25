use serde::{Deserialize, Serialize};

/// External evidence references that anchor a commitment.
///
/// Evidence bundles provide proof that a commitment has justification.
/// References are URIs pointing to external evidence stores (e.g.,
/// `issue://PROJ-42`, `obj://hash`, `doc://spec-v2`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceBundle {
    /// URIs to external evidence artifacts.
    pub references: Vec<String>,
    /// BLAKE3 digest of the serialized references (for integrity).
    pub digest: [u8; 32],
}

impl EvidenceBundle {
    /// Create a bundle from a list of reference URIs.
    ///
    /// The digest is computed automatically from the references.
    pub fn from_references(references: Vec<String>) -> Self {
        let digest = compute_digest(&references);
        Self { references, digest }
    }

    /// Create an empty evidence bundle (no evidence).
    pub fn empty() -> Self {
        Self::from_references(vec![])
    }

    /// Returns `true` if the bundle has no references.
    pub fn is_empty(&self) -> bool {
        self.references.is_empty()
    }

    /// Number of evidence references.
    pub fn len(&self) -> usize {
        self.references.len()
    }

    /// Verify that the digest matches the references.
    pub fn verify_digest(&self) -> bool {
        compute_digest(&self.references) == self.digest
    }
}

fn compute_digest(references: &[String]) -> [u8; 32] {
    let serialized = serde_json::to_vec(references).unwrap_or_default();
    *blake3::hash(&serialized).as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_references_computes_digest() {
        let bundle =
            EvidenceBundle::from_references(vec!["issue://PROJ-42".into(), "doc://spec".into()]);
        assert_ne!(bundle.digest, [0; 32]);
        assert!(bundle.verify_digest());
    }

    #[test]
    fn empty_bundle() {
        let bundle = EvidenceBundle::empty();
        assert!(bundle.is_empty());
        assert_eq!(bundle.len(), 0);
        assert!(bundle.verify_digest());
    }

    #[test]
    fn digest_changes_with_content() {
        let b1 = EvidenceBundle::from_references(vec!["a".into()]);
        let b2 = EvidenceBundle::from_references(vec!["b".into()]);
        assert_ne!(b1.digest, b2.digest);
    }

    #[test]
    fn tampered_references_fail_verify() {
        let mut bundle = EvidenceBundle::from_references(vec!["original".into()]);
        bundle.references = vec!["tampered".into()];
        assert!(!bundle.verify_digest());
    }

    #[test]
    fn serde_roundtrip() {
        let bundle = EvidenceBundle::from_references(vec!["obj://abc".into()]);
        let json = serde_json::to_string(&bundle).unwrap();
        let parsed: EvidenceBundle = serde_json::from_str(&json).unwrap();
        assert_eq!(bundle, parsed);
    }
}
