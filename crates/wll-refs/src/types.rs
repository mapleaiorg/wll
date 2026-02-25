//! Core reference types for the WorldLine Ledger.
//!
//! References are named pointers into the receipt chain. They come in three
//! flavors: branches (mutable tips), tags (immutable snapshots), and remote
//! tracking refs.

use serde::{Deserialize, Serialize};
use wll_crypto::Signature;
use wll_types::{TemporalAnchor, WorldlineId};

/// A named reference in the WorldLine Ledger.
///
/// References provide human-readable names for receipt chain tips and worldline
/// snapshots, analogous to git refs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Ref {
    /// A branch is a mutable pointer to a receipt chain tip.
    ///
    /// Branches move forward as new receipts are appended.
    Branch {
        /// Human-readable branch name (e.g. "main", "feature/auth").
        name: String,
        /// The worldline this branch belongs to.
        worldline: WorldlineId,
        /// Hash of the receipt at the tip of this branch.
        receipt_hash: [u8; 32],
    },

    /// A tag is an immutable pointer to a specific receipt.
    ///
    /// Once created, tags cannot be moved. Delete and recreate if needed.
    Tag {
        /// Tag name (e.g. "v1.0.0").
        name: String,
        /// Hash of the target receipt.
        target: [u8; 32],
        /// Identity of the tagger.
        tagger: WorldlineId,
        /// Human-readable tag message.
        message: String,
        /// Timestamp when the tag was created.
        timestamp: TemporalAnchor,
        /// Optional cryptographic signature over the tag.
        signature: Option<Signature>,
    },

    /// A remote tracking ref mirrors a branch on a remote node.
    ///
    /// Remote refs are only updated by sync operations, never directly.
    Remote {
        /// Name of the remote (e.g. "origin").
        remote: String,
        /// Branch name on the remote.
        branch: String,
        /// The worldline this remote branch tracks.
        worldline: WorldlineId,
        /// Hash of the receipt at the remote branch tip.
        receipt_hash: [u8; 32],
    },
}

impl Ref {
    /// Returns the canonical name for this ref (e.g. "refs/heads/main").
    pub fn canonical_name(&self) -> String {
        match self {
            Ref::Branch { name, .. } => format!("refs/heads/{name}"),
            Ref::Tag { name, .. } => format!("refs/tags/{name}"),
            Ref::Remote {
                remote, branch, ..
            } => format!("refs/remotes/{remote}/{branch}"),
        }
    }

    /// Returns the short name of this ref (without the refs/ prefix).
    pub fn short_name(&self) -> &str {
        match self {
            Ref::Branch { name, .. } => name,
            Ref::Tag { name, .. } => name,
            Ref::Remote { branch, .. } => branch,
        }
    }

    /// Returns `true` if this is a branch ref.
    pub fn is_branch(&self) -> bool {
        matches!(self, Ref::Branch { .. })
    }

    /// Returns `true` if this is a tag ref.
    pub fn is_tag(&self) -> bool {
        matches!(self, Ref::Tag { .. })
    }

    /// Returns `true` if this is a remote tracking ref.
    pub fn is_remote(&self) -> bool {
        matches!(self, Ref::Remote { .. })
    }

    /// Returns the receipt hash this ref points to.
    pub fn target_hash(&self) -> &[u8; 32] {
        match self {
            Ref::Branch { receipt_hash, .. } => receipt_hash,
            Ref::Tag { target, .. } => target,
            Ref::Remote { receipt_hash, .. } => receipt_hash,
        }
    }
}

/// Summary information about a branch.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BranchInfo {
    /// Branch name.
    pub name: String,
    /// The worldline this branch belongs to.
    pub worldline: WorldlineId,
    /// Hash of the receipt at the branch tip.
    pub head_receipt: [u8; 32],
    /// Sequence number of the head receipt.
    pub head_seq: u64,
    /// Whether this is the currently checked-out branch (HEAD points here).
    pub is_current: bool,
}

/// The state of HEAD: either symbolic (pointing to a branch) or detached.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Head {
    /// HEAD points to a branch by name.
    Symbolic(String),
    /// HEAD is detached, pointing directly to a receipt hash.
    Detached([u8; 32]),
}
