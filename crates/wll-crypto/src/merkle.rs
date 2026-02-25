use serde::{Deserialize, Serialize};
use wll_types::ObjectId;

/// Side of a sibling in a Merkle proof path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Left,
    Right,
}

/// Binary Merkle tree for efficient proof of inclusion.
///
/// Constructed from a set of leaf `ObjectId`s. Supports generating inclusion
/// proofs and verifying them.
#[derive(Clone, Debug)]
pub struct MerkleTree {
    /// The root hash of the tree.
    root: ObjectId,
    /// Original leaf hashes.
    leaves: Vec<ObjectId>,
    /// All tree nodes (leaves + internal), stored level by level.
    /// Level 0 = leaves, last element = root.
    levels: Vec<Vec<ObjectId>>,
}

impl MerkleTree {
    /// Build a Merkle tree from leaf object IDs.
    ///
    /// An empty list produces a null root. A single leaf is its own root.
    pub fn from_leaves(leaves: Vec<ObjectId>) -> Self {
        if leaves.is_empty() {
            return Self {
                root: ObjectId::null(),
                leaves: vec![],
                levels: vec![],
            };
        }

        let mut levels: Vec<Vec<ObjectId>> = vec![leaves.clone()];
        let mut current = leaves.clone();

        while current.len() > 1 {
            let mut next = Vec::with_capacity((current.len() + 1) / 2);
            for pair in current.chunks(2) {
                let hash = if pair.len() == 2 {
                    hash_pair(&pair[0], &pair[1])
                } else {
                    // Odd node: hash with itself
                    hash_pair(&pair[0], &pair[0])
                };
                next.push(hash);
            }
            levels.push(next.clone());
            current = next;
        }

        let root = current[0];
        Self {
            root,
            leaves,
            levels,
        }
    }

    /// The root hash of the tree.
    pub fn root(&self) -> ObjectId {
        self.root
    }

    /// Number of leaves.
    pub fn leaf_count(&self) -> usize {
        self.leaves.len()
    }

    /// Generate an inclusion proof for the leaf at `index`.
    pub fn proof(&self, index: usize) -> Option<MerkleProof> {
        if index >= self.leaves.len() || self.levels.is_empty() {
            return None;
        }

        let mut path = Vec::new();
        let mut idx = index;

        for level in &self.levels[..self.levels.len() - 1] {
            let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
            let sibling = if sibling_idx < level.len() {
                level[sibling_idx]
            } else {
                // Odd level: duplicate the last element
                level[idx]
            };
            let side = if idx % 2 == 0 {
                Side::Right
            } else {
                Side::Left
            };
            path.push((sibling, side));
            idx /= 2;
        }

        Some(MerkleProof {
            leaf: self.leaves[index],
            path,
            root: self.root,
        })
    }
}

/// Merkle inclusion proof.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerkleProof {
    /// The leaf being proven.
    pub leaf: ObjectId,
    /// Path of (sibling_hash, sibling_side) pairs from leaf to root.
    pub path: Vec<(ObjectId, Side)>,
    /// Expected root hash.
    pub root: ObjectId,
}

impl MerkleProof {
    /// Verify the proof: recompute the root from the leaf and path.
    pub fn verify(&self) -> bool {
        let mut current = self.leaf;
        for (sibling, side) in &self.path {
            current = match side {
                Side::Left => hash_pair(sibling, &current),
                Side::Right => hash_pair(&current, sibling),
            };
        }
        current == self.root
    }
}

fn hash_pair(left: &ObjectId, right: &ObjectId) -> ObjectId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"wll-merkle-v1:");
    hasher.update(left.as_bytes());
    hasher.update(right.as_bytes());
    ObjectId::from_hash(*hasher.finalize().as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(seed: u8) -> ObjectId {
        ObjectId::from_bytes(&[seed])
    }

    #[test]
    fn empty_tree_has_null_root() {
        let tree = MerkleTree::from_leaves(vec![]);
        assert!(tree.root().is_null());
        assert_eq!(tree.leaf_count(), 0);
    }

    #[test]
    fn single_leaf_is_root() {
        let l = leaf(1);
        let tree = MerkleTree::from_leaves(vec![l]);
        assert_eq!(tree.root(), l);
    }

    #[test]
    fn two_leaves_produce_parent() {
        let tree = MerkleTree::from_leaves(vec![leaf(1), leaf(2)]);
        assert_ne!(tree.root(), leaf(1));
        assert_ne!(tree.root(), leaf(2));
    }

    #[test]
    fn proof_verifies_for_all_leaves() {
        let leaves: Vec<ObjectId> = (0..7).map(leaf).collect();
        let tree = MerkleTree::from_leaves(leaves.clone());

        for i in 0..leaves.len() {
            let proof = tree.proof(i).expect("proof should exist");
            assert_eq!(proof.leaf, leaves[i]);
            assert!(proof.verify(), "proof for leaf {i} should verify");
        }
    }

    #[test]
    fn proof_out_of_bounds_returns_none() {
        let tree = MerkleTree::from_leaves(vec![leaf(1), leaf(2)]);
        assert!(tree.proof(5).is_none());
    }

    #[test]
    fn tampered_proof_fails_verification() {
        let tree = MerkleTree::from_leaves(vec![leaf(1), leaf(2), leaf(3), leaf(4)]);
        let mut proof = tree.proof(0).unwrap();
        proof.leaf = leaf(99); // tamper with the leaf
        assert!(!proof.verify());
    }

    #[test]
    fn different_trees_different_roots() {
        let tree1 = MerkleTree::from_leaves(vec![leaf(1), leaf(2)]);
        let tree2 = MerkleTree::from_leaves(vec![leaf(3), leaf(4)]);
        assert_ne!(tree1.root(), tree2.root());
    }

    #[test]
    fn deterministic_root() {
        let leaves: Vec<ObjectId> = (0..10).map(leaf).collect();
        let tree1 = MerkleTree::from_leaves(leaves.clone());
        let tree2 = MerkleTree::from_leaves(leaves);
        assert_eq!(tree1.root(), tree2.root());
    }

    #[test]
    fn power_of_two_leaves() {
        let leaves: Vec<ObjectId> = (0..8).map(leaf).collect();
        let tree = MerkleTree::from_leaves(leaves.clone());
        for i in 0..8 {
            let proof = tree.proof(i).unwrap();
            assert!(proof.verify());
            assert_eq!(proof.path.len(), 3); // log2(8) = 3
        }
    }

    #[test]
    fn proof_serde_roundtrip() {
        let tree = MerkleTree::from_leaves(vec![leaf(1), leaf(2), leaf(3), leaf(4)]);
        let proof = tree.proof(2).unwrap();
        let json = serde_json::to_string(&proof).unwrap();
        let parsed: MerkleProof = serde_json::from_str(&json).unwrap();
        assert_eq!(proof, parsed);
        assert!(parsed.verify());
    }
}
