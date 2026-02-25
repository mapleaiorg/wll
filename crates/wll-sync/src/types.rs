use serde::{Deserialize, Serialize};
use wll_types::{ObjectId, WorldlineId};

/// A refspec mapping local to remote refs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefSpec {
    pub src: String,
    pub dst: String,
    pub force: bool,
}

impl RefSpec {
    pub fn new(src: impl Into<String>, dst: impl Into<String>) -> Self {
        Self { src: src.into(), dst: dst.into(), force: false }
    }

    pub fn forced(src: impl Into<String>, dst: impl Into<String>) -> Self {
        Self { src: src.into(), dst: dst.into(), force: true }
    }

    /// Parse "+refs/heads/*:refs/remotes/origin/*"
    pub fn parse(s: &str) -> Option<Self> {
        let (force, rest) = if let Some(stripped) = s.strip_prefix('+') {
            (true, stripped)
        } else {
            (false, s)
        };
        let parts: Vec<&str> = rest.splitn(2, ':').collect();
        if parts.len() == 2 {
            Some(Self { src: parts[0].into(), dst: parts[1].into(), force })
        } else {
            Some(Self { src: rest.into(), dst: rest.into(), force })
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefUpdate {
    pub name: String,
    pub old_hash: Option<[u8; 32]>,
    pub new_hash: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefRejection {
    pub name: String,
    pub reason: String,
}

#[derive(Clone, Debug, Default)]
pub struct FetchResult {
    pub objects_received: usize,
    pub receipts_received: usize,
    pub refs_updated: Vec<RefUpdate>,
    pub bytes_transferred: u64,
}

#[derive(Clone, Debug, Default)]
pub struct PushResult {
    pub objects_sent: usize,
    pub receipts_sent: usize,
    pub refs_updated: Vec<RefUpdate>,
    pub rejected: Vec<RefRejection>,
    pub bytes_transferred: u64,
}

#[derive(Clone, Debug, Default)]
pub struct PullResult {
    pub fetch: FetchResult,
    pub merge_status: MergeStatus,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum MergeStatus {
    #[default]
    UpToDate,
    FastForward,
    Merged,
    Conflict { files: Vec<String> },
}

#[derive(Clone, Debug, Default)]
pub struct Negotiation {
    pub wants: Vec<ObjectId>,
    pub haves: Vec<ObjectId>,
    pub common: Vec<ObjectId>,
}

#[derive(Clone, Debug)]
pub struct CloneOptions {
    pub bare: bool,
    pub branch: Option<String>,
    pub depth: Option<u32>,
}

impl Default for CloneOptions {
    fn default() -> Self {
        Self { bare: false, branch: None, depth: None }
    }
}

#[derive(Clone, Debug)]
pub struct VerificationReport {
    pub worldline: WorldlineId,
    pub receipts_verified: u64,
    pub chain_valid: bool,
    pub violations: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refspec_parse_with_force() {
        let rs = RefSpec::parse("+refs/heads/*:refs/remotes/origin/*").unwrap();
        assert!(rs.force);
        assert_eq!(rs.src, "refs/heads/*");
        assert_eq!(rs.dst, "refs/remotes/origin/*");
    }

    #[test]
    fn refspec_parse_without_force() {
        let rs = RefSpec::parse("refs/heads/*:refs/heads/*").unwrap();
        assert!(!rs.force);
        assert_eq!(rs.src, "refs/heads/*");
    }

    #[test]
    fn refspec_parse_single() {
        let rs = RefSpec::parse("refs/heads/main").unwrap();
        assert_eq!(rs.src, "refs/heads/main");
        assert_eq!(rs.dst, "refs/heads/main");
    }

    #[test]
    fn refspec_new_and_forced() {
        let rs = RefSpec::new("a", "b");
        assert!(!rs.force);
        let rs = RefSpec::forced("a", "b");
        assert!(rs.force);
    }

    #[test]
    fn fetch_result_defaults() {
        let f = FetchResult::default();
        assert_eq!(f.objects_received, 0);
        assert_eq!(f.bytes_transferred, 0);
    }

    #[test]
    fn push_result_defaults() {
        let p = PushResult::default();
        assert_eq!(p.objects_sent, 0);
        assert!(p.rejected.is_empty());
    }

    #[test]
    fn pull_result_defaults() {
        let p = PullResult::default();
        assert_eq!(p.merge_status, MergeStatus::UpToDate);
    }

    #[test]
    fn merge_status_variants() {
        assert_eq!(MergeStatus::default(), MergeStatus::UpToDate);
        let c = MergeStatus::Conflict { files: vec!["a.rs".into()] };
        assert!(matches!(c, MergeStatus::Conflict { .. }));
    }

    #[test]
    fn clone_options_defaults() {
        let co = CloneOptions::default();
        assert!(!co.bare);
        assert!(co.branch.is_none());
        assert!(co.depth.is_none());
    }

    #[test]
    fn ref_update_construction() {
        let u = RefUpdate { name: "main".into(), old_hash: None, new_hash: [1; 32] };
        assert_eq!(u.name, "main");
    }

    #[test]
    fn ref_rejection_construction() {
        let r = RefRejection { name: "main".into(), reason: "non-ff".into() };
        assert_eq!(r.reason, "non-ff");
    }
}
