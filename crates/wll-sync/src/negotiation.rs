use std::collections::HashSet;
use wll_types::ObjectId;
use crate::types::Negotiation;

/// Object negotiation engine: computes wants/haves to minimize transfer.
pub struct NegotiationEngine;

impl NegotiationEngine {
    /// Objects the remote has that we don't.
    pub fn compute_wants(
        local_refs: &[(String, [u8; 32])],
        remote_refs: &[(String, [u8; 32])],
    ) -> Vec<ObjectId> {
        let local: HashSet<[u8; 32]> = local_refs.iter().map(|(_, h)| *h).collect();
        remote_refs
            .iter()
            .filter(|(_, h)| !local.contains(h))
            .map(|(_, h)| ObjectId::from_hash(*h))
            .collect()
    }

    /// Objects we have to advertise.
    pub fn compute_haves(local_refs: &[(String, [u8; 32])]) -> Vec<ObjectId> {
        local_refs.iter().map(|(_, h)| ObjectId::from_hash(*h)).collect()
    }

    /// Full negotiation.
    pub fn negotiate(
        local_refs: &[(String, [u8; 32])],
        remote_refs: &[(String, [u8; 32])],
    ) -> Negotiation {
        let wants = Self::compute_wants(local_refs, remote_refs);
        let haves = Self::compute_haves(local_refs);
        let local: HashSet<[u8; 32]> = local_refs.iter().map(|(_, h)| *h).collect();
        let common: Vec<ObjectId> = remote_refs
            .iter()
            .filter(|(_, h)| local.contains(h))
            .map(|(_, h)| ObjectId::from_hash(*h))
            .collect();
        Negotiation { wants, haves, common }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_wants_finds_missing() {
        let local = vec![("main".into(), [1u8; 32])];
        let remote = vec![("main".into(), [1u8; 32]), ("dev".into(), [2u8; 32])];
        let wants = NegotiationEngine::compute_wants(&local, &remote);
        assert_eq!(wants.len(), 1);
        assert_eq!(*wants[0].as_bytes(), [2u8; 32]);
    }

    #[test]
    fn compute_wants_empty_when_synced() {
        let refs = vec![("main".into(), [1u8; 32])];
        let wants = NegotiationEngine::compute_wants(&refs, &refs);
        assert!(wants.is_empty());
    }

    #[test]
    fn compute_haves_returns_all() {
        let local = vec![("a".into(), [1u8; 32]), ("b".into(), [2u8; 32])];
        let haves = NegotiationEngine::compute_haves(&local);
        assert_eq!(haves.len(), 2);
    }

    #[test]
    fn negotiate_correct_common() {
        let local = vec![("main".into(), [1u8; 32]), ("dev".into(), [2u8; 32])];
        let remote = vec![("main".into(), [1u8; 32]), ("feature".into(), [3u8; 32])];
        let neg = NegotiationEngine::negotiate(&local, &remote);
        assert_eq!(neg.wants.len(), 1);
        assert_eq!(neg.common.len(), 1);
        assert_eq!(neg.haves.len(), 2);
    }

    #[test]
    fn negotiate_empty_local() {
        let local: Vec<(String, [u8; 32])> = vec![];
        let remote = vec![("main".into(), [1u8; 32])];
        let neg = NegotiationEngine::negotiate(&local, &remote);
        assert_eq!(neg.wants.len(), 1);
        assert!(neg.haves.is_empty());
        assert!(neg.common.is_empty());
    }
}
