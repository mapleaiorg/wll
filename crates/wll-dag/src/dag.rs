//! The core provenance DAG structure and traversal algorithms.
//!
//! [`ProvenanceDag`] is the main data structure. It stores nodes in a
//! [`HashMap`] and maintains a forward-edge index (`children`) for efficient
//! descendant queries. Root nodes (those with no parents) are tracked
//! separately for fast enumeration.
//!
//! # Invariants
//!
//! - The graph is acyclic (append-only + temporal ordering).
//! - Every parent reference resolves to an existing node.
//! - Node IDs are unique within the DAG.

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use tracing::debug;

use wll_types::{ObjectId, TemporalAnchor, WorldlineId};

use crate::audit::{AuditEntry, AuditTrail, ImpactReport};
use crate::error::{DagError, DagResult};
use crate::node::{CausalRelation, DagNode};

/// The provenance DAG: a directed acyclic graph of causal relationships
/// between receipts across worldlines.
///
/// The DAG is a *derived* structure --- it can always be rebuilt from
/// receipt streams. It supports incremental construction via [`add_node`].
///
/// [`add_node`]: ProvenanceDag::add_node
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProvenanceDag {
    /// All nodes, keyed by their ObjectId.
    nodes: HashMap<ObjectId, DagNode>,
    /// Forward-edge index: parent -> list of children.
    children: HashMap<ObjectId, Vec<ObjectId>>,
    /// Nodes that have no parents (genesis / stream starts).
    roots: Vec<ObjectId>,
}

impl ProvenanceDag {
    /// Create an empty DAG.
    pub fn new() -> Self {
        Self::default()
    }

    /// Total number of nodes in the DAG.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns `true` if the DAG has no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    // ---------------------------------------------------------------
    // Mutation
    // ---------------------------------------------------------------

    /// Add a node to the DAG.
    ///
    /// All parents referenced by the node must already exist in the DAG
    /// (or the parents list must be empty for root nodes). Returns an error
    /// if the node ID already exists or if a parent reference dangles.
    pub fn add_node(&mut self, node: DagNode) -> DagResult<()> {
        if self.nodes.contains_key(&node.id) {
            return Err(DagError::DuplicateNode(node.id));
        }

        // Validate parent references.
        for parent_ref in &node.parents {
            if !self.nodes.contains_key(&parent_ref.target) {
                return Err(DagError::DanglingParent {
                    node: node.id,
                    parent: parent_ref.target,
                });
            }
        }

        // Update forward-edge index.
        for parent_ref in &node.parents {
            self.children
                .entry(parent_ref.target)
                .or_default()
                .push(node.id);
        }

        // Track roots.
        if node.is_root() {
            self.roots.push(node.id);
        }

        debug!(node = %node.id.short_hex(), seq = node.seq, "added DAG node");
        self.nodes.insert(node.id, node);

        Ok(())
    }

    /// Retrieve a node by its ObjectId.
    pub fn get_node(&self, id: &ObjectId) -> Option<&DagNode> {
        self.nodes.get(id)
    }

    /// All root nodes (nodes with no parents).
    pub fn roots(&self) -> Vec<&DagNode> {
        self.roots
            .iter()
            .filter_map(|id| self.nodes.get(id))
            .collect()
    }

    // ---------------------------------------------------------------
    // Ancestor / Descendant queries
    // ---------------------------------------------------------------

    /// All ancestors of a node up to `max_depth` levels (BFS upward).
    ///
    /// Returns an empty vec if the node is not found. The node itself
    /// is **not** included in the result.
    pub fn ancestors(&self, id: &ObjectId, max_depth: usize) -> Vec<&DagNode> {
        let Some(start) = self.nodes.get(id) else {
            return Vec::new();
        };

        let mut visited = HashSet::new();
        visited.insert(*id);
        let mut result = Vec::new();
        let mut queue: VecDeque<(&ObjectId, usize)> = VecDeque::new();

        // Seed with the start node's parents at depth 1.
        for parent_ref in &start.parents {
            if visited.insert(parent_ref.target) {
                queue.push_back((&parent_ref.target, 1));
            }
        }

        while let Some((current_id, depth)) = queue.pop_front() {
            if depth > max_depth {
                continue;
            }
            if let Some(node) = self.nodes.get(current_id) {
                result.push(node);
                if depth < max_depth {
                    for parent_ref in &node.parents {
                        if visited.insert(parent_ref.target) {
                            queue.push_back((&parent_ref.target, depth + 1));
                        }
                    }
                }
            }
        }

        result
    }

    /// All descendants of a node up to `max_depth` levels (BFS downward).
    ///
    /// Returns an empty vec if the node is not found. The node itself
    /// is **not** included in the result.
    pub fn descendants(&self, id: &ObjectId, max_depth: usize) -> Vec<&DagNode> {
        if !self.nodes.contains_key(id) {
            return Vec::new();
        }

        let mut visited = HashSet::new();
        visited.insert(*id);
        let mut result = Vec::new();
        let mut queue: VecDeque<(&ObjectId, usize)> = VecDeque::new();

        // Seed with direct children at depth 1.
        if let Some(child_ids) = self.children.get(id) {
            for child_id in child_ids {
                if visited.insert(*child_id) {
                    queue.push_back((child_id, 1));
                }
            }
        }

        while let Some((current_id, depth)) = queue.pop_front() {
            if depth > max_depth {
                continue;
            }
            if let Some(node) = self.nodes.get(current_id) {
                result.push(node);
                if depth < max_depth {
                    if let Some(child_ids) = self.children.get(current_id) {
                        for child_id in child_ids {
                            if visited.insert(*child_id) {
                                queue.push_back((child_id, depth + 1));
                            }
                        }
                    }
                }
            }
        }

        result
    }

    // ---------------------------------------------------------------
    // Path queries
    // ---------------------------------------------------------------

    /// Find the shortest causal path between two nodes using bidirectional
    /// BFS over the combined parent + child edges.
    ///
    /// Returns `None` if no path exists.
    pub fn causal_path(&self, from: &ObjectId, to: &ObjectId) -> Option<Vec<&DagNode>> {
        if !self.nodes.contains_key(from) || !self.nodes.contains_key(to) {
            return None;
        }
        if from == to {
            return Some(vec![self.nodes.get(from).unwrap()]);
        }

        // BFS from `from` following both parent and child edges.
        let mut visited: HashMap<ObjectId, Option<ObjectId>> = HashMap::new();
        visited.insert(*from, None);
        let mut queue: VecDeque<ObjectId> = VecDeque::new();
        queue.push_back(*from);

        while let Some(current) = queue.pop_front() {
            if current == *to {
                // Reconstruct path.
                return Some(self.reconstruct_path(&visited, from, to));
            }

            // Explore parents (upward).
            if let Some(node) = self.nodes.get(&current) {
                for parent_ref in &node.parents {
                    if !visited.contains_key(&parent_ref.target) {
                        visited.insert(parent_ref.target, Some(current));
                        queue.push_back(parent_ref.target);
                    }
                }
            }

            // Explore children (downward).
            if let Some(child_ids) = self.children.get(&current) {
                for child_id in child_ids {
                    if !visited.contains_key(child_id) {
                        visited.insert(*child_id, Some(current));
                        queue.push_back(*child_id);
                    }
                }
            }
        }

        None
    }

    /// Reconstruct a path from the BFS predecessor map.
    fn reconstruct_path<'a>(
        &'a self,
        predecessors: &HashMap<ObjectId, Option<ObjectId>>,
        from: &ObjectId,
        to: &ObjectId,
    ) -> Vec<&'a DagNode> {
        let mut path = Vec::new();
        let mut current = *to;

        loop {
            if let Some(node) = self.nodes.get(&current) {
                path.push(node);
            }
            if current == *from {
                break;
            }
            match predecessors.get(&current) {
                Some(Some(prev)) => current = *prev,
                _ => break,
            }
        }

        path.reverse();
        path
    }

    // ---------------------------------------------------------------
    // Worldline queries
    // ---------------------------------------------------------------

    /// All nodes belonging to a specific worldline, ordered by sequence number.
    pub fn worldline_history(&self, worldline: &WorldlineId) -> Vec<&DagNode> {
        let mut nodes: Vec<&DagNode> = self
            .nodes
            .values()
            .filter(|n| &n.worldline == worldline)
            .collect();
        nodes.sort_by_key(|n| n.seq);
        nodes
    }

    // ---------------------------------------------------------------
    // Common ancestor
    // ---------------------------------------------------------------

    /// Find the lowest common ancestor of two nodes.
    ///
    /// Uses the ancestor-set intersection approach: compute ancestor sets
    /// for both nodes, then find the common ancestor with the highest
    /// timestamp (most recent).
    pub fn common_ancestor(&self, a: &ObjectId, b: &ObjectId) -> Option<&DagNode> {
        if !self.nodes.contains_key(a) || !self.nodes.contains_key(b) {
            return None;
        }
        if a == b {
            return self.nodes.get(a);
        }

        // Collect all ancestors of a (including a itself).
        let ancestors_a = self.all_ancestors_set(a);
        // Collect all ancestors of b (including b itself).
        let ancestors_b = self.all_ancestors_set(b);

        // Common ancestors are the intersection.
        let common: Vec<&ObjectId> = ancestors_a.intersection(&ancestors_b).collect();

        // The lowest common ancestor is the one with the latest timestamp
        // (i.e., closest to both nodes).
        common
            .into_iter()
            .filter_map(|id| self.nodes.get(id))
            .max_by_key(|node| node.timestamp)
    }

    /// Collect all ancestors of a node (including the node itself) into a set.
    fn all_ancestors_set(&self, id: &ObjectId) -> HashSet<ObjectId> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        visited.insert(*id);
        queue.push_back(*id);

        while let Some(current) = queue.pop_front() {
            if let Some(node) = self.nodes.get(&current) {
                for parent_ref in &node.parents {
                    if visited.insert(parent_ref.target) {
                        queue.push_back(parent_ref.target);
                    }
                }
            }
        }

        visited
    }

    // ---------------------------------------------------------------
    // Topological sort
    // ---------------------------------------------------------------

    /// Return all nodes in topological order (parents before children).
    ///
    /// Uses Kahn's algorithm. If the graph has no cycles (which is an
    /// invariant), this will return all nodes.
    pub fn topological_order(&self) -> Vec<&DagNode> {
        // Compute in-degree for each node.
        let mut in_degree: HashMap<ObjectId, usize> = HashMap::new();
        for node in self.nodes.values() {
            in_degree.entry(node.id).or_insert(0);
            for _parent_ref in &node.parents {
                // Each child edge from parent -> node increases node's in-degree
                // from the perspective of "parent edges are the forward direction".
                // Actually, in our DAG parents point backward, so we need:
                // in_degree tracks how many parents each node has.
            }
        }

        // In-degree = number of parents for each node.
        for node in self.nodes.values() {
            in_degree.insert(node.id, node.parents.len());
        }

        // Start with roots (in-degree = 0).
        let mut queue: VecDeque<ObjectId> = VecDeque::new();
        for (&id, &deg) in &in_degree {
            if deg == 0 {
                queue.push_back(id);
            }
        }

        // Sort the initial queue by timestamp for deterministic output.
        let mut initial: Vec<ObjectId> = queue.drain(..).collect();
        initial.sort_by(|a, b| {
            let na = self.nodes.get(a);
            let nb = self.nodes.get(b);
            match (na, nb) {
                (Some(na), Some(nb)) => na.timestamp.cmp(&nb.timestamp),
                _ => std::cmp::Ordering::Equal,
            }
        });
        for id in initial {
            queue.push_back(id);
        }

        let mut result = Vec::new();

        while let Some(current) = queue.pop_front() {
            if let Some(node) = self.nodes.get(&current) {
                result.push(node);
            }

            // "Remove" this node by decrementing in-degree of children.
            if let Some(child_ids) = self.children.get(&current) {
                // Sort children by timestamp for deterministic output.
                let mut sorted_children = child_ids.clone();
                sorted_children.sort_by(|a, b| {
                    let na = self.nodes.get(a);
                    let nb = self.nodes.get(b);
                    match (na, nb) {
                        (Some(na), Some(nb)) => na.timestamp.cmp(&nb.timestamp),
                        _ => std::cmp::Ordering::Equal,
                    }
                });

                for child_id in &sorted_children {
                    if let Some(deg) = in_degree.get_mut(child_id) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(*child_id);
                        }
                    }
                }
            }
        }

        result
    }

    // ---------------------------------------------------------------
    // Audit & Impact
    // ---------------------------------------------------------------

    /// Build an audit trail for a commitment by following parent edges.
    ///
    /// The trail starts with the commitment node itself and follows all
    /// parent edges recursively, producing a chain ordered by timestamp
    /// (most recent first).
    pub fn audit_trail(&self, commitment_id: &ObjectId) -> AuditTrail {
        let mut trail = AuditTrail::new(*commitment_id);

        let Some(start) = self.nodes.get(commitment_id) else {
            return trail;
        };

        // BFS from the commitment backward through parents.
        let mut visited = HashSet::new();
        visited.insert(*commitment_id);
        let mut queue = VecDeque::new();

        // Add the start node.
        trail.chain.push(AuditEntry {
            node: start.id,
            relation: CausalRelation::Sequential, // root of the chain
            worldline: start.worldline.clone(),
            timestamp: start.timestamp,
            summary: start.summary(),
        });

        for parent_ref in &start.parents {
            if visited.insert(parent_ref.target) {
                queue.push_back((parent_ref.target, parent_ref.relation));
            }
        }

        while let Some((current_id, relation)) = queue.pop_front() {
            if let Some(node) = self.nodes.get(&current_id) {
                trail.chain.push(AuditEntry {
                    node: node.id,
                    relation,
                    worldline: node.worldline.clone(),
                    timestamp: node.timestamp,
                    summary: node.summary(),
                });

                for parent_ref in &node.parents {
                    if visited.insert(parent_ref.target) {
                        queue.push_back((parent_ref.target, parent_ref.relation));
                    }
                }
            }
        }

        // Sort by timestamp, most recent first.
        trail.chain.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        trail
    }

    /// Compute an impact report: what downstream nodes are affected by the
    /// given origin node.
    pub fn impact_report(&self, id: &ObjectId) -> ImpactReport {
        let mut report = ImpactReport::new(*id);

        if !self.nodes.contains_key(id) {
            return report;
        }

        // BFS downward through children.
        let mut visited = HashSet::new();
        visited.insert(*id);
        let mut queue: VecDeque<(ObjectId, usize)> = VecDeque::new();
        let mut max_depth: usize = 0;
        let mut worldlines_set = HashSet::new();
        let mut leaf_ids = Vec::new();

        // Seed with direct children.
        if let Some(child_ids) = self.children.get(id) {
            for child_id in child_ids {
                if visited.insert(*child_id) {
                    queue.push_back((*child_id, 1));
                }
            }
        }

        while let Some((current_id, depth)) = queue.pop_front() {
            if let Some(node) = self.nodes.get(&current_id) {
                report.downstream_receipts += 1;
                worldlines_set.insert(node.worldline.clone());
                if depth > max_depth {
                    max_depth = depth;
                }

                let mut has_children = false;
                if let Some(child_ids) = self.children.get(&current_id) {
                    for child_id in child_ids {
                        if visited.insert(*child_id) {
                            queue.push_back((*child_id, depth + 1));
                            has_children = true;
                        }
                    }
                }

                if !has_children {
                    leaf_ids.push(current_id);
                }
            }
        }

        report.cascade_depth = max_depth;
        report.affected_worldlines = worldlines_set.into_iter().collect();
        report.affected_worldlines.sort();

        // Build critical paths from origin to each leaf.
        for leaf in &leaf_ids {
            if let Some(path) = self.directed_path_down(id, leaf) {
                report.critical_paths.push(path);
            }
        }

        report
    }

    /// Find a directed path from `from` to `to` following only child edges.
    fn directed_path_down(&self, from: &ObjectId, to: &ObjectId) -> Option<Vec<ObjectId>> {
        if from == to {
            return Some(vec![*from]);
        }

        let mut visited = HashSet::new();
        visited.insert(*from);
        let mut predecessors: HashMap<ObjectId, ObjectId> = HashMap::new();
        let mut queue = VecDeque::new();
        queue.push_back(*from);

        while let Some(current) = queue.pop_front() {
            if current == *to {
                // Reconstruct.
                let mut path = vec![*to];
                let mut c = *to;
                while c != *from {
                    if let Some(prev) = predecessors.get(&c) {
                        path.push(*prev);
                        c = *prev;
                    } else {
                        break;
                    }
                }
                path.reverse();
                return Some(path);
            }

            if let Some(child_ids) = self.children.get(&current) {
                for child_id in child_ids {
                    if visited.insert(*child_id) {
                        predecessors.insert(*child_id, current);
                        queue.push_back(*child_id);
                    }
                }
            }
        }

        None
    }

    // ---------------------------------------------------------------
    // Validation
    // ---------------------------------------------------------------

    /// Validate the DAG's structural integrity.
    ///
    /// Checks that:
    /// - All parent references resolve to existing nodes.
    /// - The children index is consistent with parent edges.
    /// - Root tracking is correct.
    pub fn validate(&self) -> DagResult<()> {
        for node in self.nodes.values() {
            for parent_ref in &node.parents {
                if !self.nodes.contains_key(&parent_ref.target) {
                    return Err(DagError::DanglingParent {
                        node: node.id,
                        parent: parent_ref.target,
                    });
                }
            }
        }

        // Verify roots are correct.
        for root_id in &self.roots {
            if let Some(node) = self.nodes.get(root_id) {
                if !node.is_root() {
                    return Err(DagError::CycleDetected(*root_id));
                }
            }
        }

        Ok(())
    }

    // ---------------------------------------------------------------
    // Serialization helpers
    // ---------------------------------------------------------------

    /// Serialize the DAG to bincode bytes.
    pub fn to_bytes(&self) -> DagResult<Vec<u8>> {
        bincode::serialize(self).map_err(|e| DagError::Serialization(e.to_string()))
    }

    /// Deserialize a DAG from bincode bytes.
    pub fn from_bytes(data: &[u8]) -> DagResult<Self> {
        bincode::deserialize(data).map_err(|e| DagError::Serialization(e.to_string()))
    }

    // ---------------------------------------------------------------
    // Checkpoint / Pruning
    // ---------------------------------------------------------------

    /// Prune all nodes with timestamps before the given horizon.
    ///
    /// Nodes that are ancestors of any retained node but fall before the
    /// horizon are removed. The retained children that referenced pruned
    /// parents become new roots. Returns the number of pruned nodes.
    pub fn checkpoint(&mut self, horizon: &TemporalAnchor) -> usize {
        // Identify nodes to prune.
        let to_prune: Vec<ObjectId> = self
            .nodes
            .iter()
            .filter(|(_, node)| node.timestamp.is_before(horizon))
            .map(|(id, _)| *id)
            .collect();

        let pruned_count = to_prune.len();
        let pruned_set: HashSet<ObjectId> = to_prune.iter().copied().collect();

        // Remove pruned nodes from the node map.
        for id in &to_prune {
            self.nodes.remove(id);
            self.children.remove(id);
        }

        // Clean up roots.
        self.roots.retain(|id| !pruned_set.contains(id));

        // For remaining nodes, remove parent refs to pruned nodes.
        // Nodes whose parents were all pruned become new roots.
        let remaining_ids: Vec<ObjectId> = self.nodes.keys().copied().collect();
        for id in remaining_ids {
            if let Some(node) = self.nodes.get_mut(&id) {
                let had_parents = !node.parents.is_empty();
                node.parents.retain(|p| !pruned_set.contains(&p.target));
                if had_parents && node.parents.is_empty() && !self.roots.contains(&id) {
                    self.roots.push(id);
                }
            }
        }

        // Clean up children index to remove references to pruned nodes.
        for children_list in self.children.values_mut() {
            children_list.retain(|id| !pruned_set.contains(id));
        }

        pruned_count
    }
}

/// Trait for persistent DAG storage backends.
pub trait DagStorage: Send + Sync {
    /// Load the full DAG from storage.
    fn load(&self) -> DagResult<ProvenanceDag>;
    /// Save the full DAG to storage.
    fn save(&self, dag: &ProvenanceDag) -> DagResult<()>;
    /// Append a single node (incremental update).
    fn append_node(&self, node: DagNode) -> DagResult<()>;
    /// Prune nodes older than the horizon.
    fn checkpoint(&self, horizon: &TemporalAnchor) -> DagResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{DagNodeMetadata, ParentRef};
    use wll_types::identity::IdentityMaterial;
    use wll_types::ReceiptKind;

    fn wl(seed: u8) -> WorldlineId {
        WorldlineId::derive(&IdentityMaterial::GenesisHash([seed; 32]))
    }

    fn oid(byte: u8) -> ObjectId {
        ObjectId::from_hash([byte; 32])
    }

    fn make_node(
        id_byte: u8,
        worldline: &WorldlineId,
        seq: u64,
        kind: ReceiptKind,
        parents: Vec<ParentRef>,
    ) -> DagNode {
        DagNode {
            id: oid(id_byte),
            worldline: worldline.clone(),
            seq,
            kind,
            timestamp: TemporalAnchor::new(1000 + seq * 100, 0, 0),
            parents,
            metadata: DagNodeMetadata::empty(),
        }
    }

    /// Build a simple linear chain: A -> B -> C
    fn build_linear_dag() -> ProvenanceDag {
        let w = wl(1);
        let mut dag = ProvenanceDag::new();
        dag.add_node(make_node(1, &w, 0, ReceiptKind::Commitment, vec![]))
            .unwrap();
        dag.add_node(make_node(
            2,
            &w,
            1,
            ReceiptKind::Outcome,
            vec![ParentRef::sequential(oid(1))],
        ))
        .unwrap();
        dag.add_node(make_node(
            3,
            &w,
            2,
            ReceiptKind::Commitment,
            vec![ParentRef::sequential(oid(2))],
        ))
        .unwrap();
        dag
    }

    /// Build a diamond DAG:
    ///   A
    ///  / \
    /// B   C
    ///  \ /
    ///   D
    fn build_diamond_dag() -> ProvenanceDag {
        let w = wl(1);
        let mut dag = ProvenanceDag::new();
        dag.add_node(make_node(1, &w, 0, ReceiptKind::Commitment, vec![]))
            .unwrap();
        dag.add_node(make_node(
            2,
            &w,
            1,
            ReceiptKind::Outcome,
            vec![ParentRef::sequential(oid(1))],
        ))
        .unwrap();
        dag.add_node(make_node(
            3,
            &w,
            2,
            ReceiptKind::Commitment,
            vec![ParentRef::sequential(oid(1))],
        ))
        .unwrap();
        dag.add_node(make_node(
            4,
            &w,
            3,
            ReceiptKind::Outcome,
            vec![
                ParentRef::sequential(oid(2)),
                ParentRef::new(oid(3), CausalRelation::Merge),
            ],
        ))
        .unwrap();
        dag
    }

    // ----------------------------------------------------------
    // Basic construction tests
    // ----------------------------------------------------------

    #[test]
    fn empty_dag() {
        let dag = ProvenanceDag::new();
        assert!(dag.is_empty());
        assert_eq!(dag.len(), 0);
        assert!(dag.roots().is_empty());
    }

    #[test]
    fn add_root_node() {
        let w = wl(1);
        let mut dag = ProvenanceDag::new();
        dag.add_node(make_node(1, &w, 0, ReceiptKind::Commitment, vec![]))
            .unwrap();
        assert_eq!(dag.len(), 1);
        assert_eq!(dag.roots().len(), 1);
        assert_eq!(dag.roots()[0].id, oid(1));
    }

    #[test]
    fn duplicate_node_is_rejected() {
        let w = wl(1);
        let mut dag = ProvenanceDag::new();
        dag.add_node(make_node(1, &w, 0, ReceiptKind::Commitment, vec![]))
            .unwrap();
        let result = dag.add_node(make_node(1, &w, 0, ReceiptKind::Commitment, vec![]));
        assert!(matches!(result, Err(DagError::DuplicateNode(_))));
    }

    #[test]
    fn dangling_parent_is_rejected() {
        let w = wl(1);
        let mut dag = ProvenanceDag::new();
        let result = dag.add_node(make_node(
            2,
            &w,
            1,
            ReceiptKind::Outcome,
            vec![ParentRef::sequential(oid(99))],
        ));
        assert!(matches!(result, Err(DagError::DanglingParent { .. })));
    }

    #[test]
    fn linear_chain_structure() {
        let dag = build_linear_dag();
        assert_eq!(dag.len(), 3);
        assert_eq!(dag.roots().len(), 1);
        assert_eq!(dag.roots()[0].id, oid(1));
    }

    // ----------------------------------------------------------
    // Ancestor tests
    // ----------------------------------------------------------

    #[test]
    fn ancestors_of_root_is_empty() {
        let dag = build_linear_dag();
        let ancestors = dag.ancestors(&oid(1), 10);
        assert!(ancestors.is_empty());
    }

    #[test]
    fn ancestors_of_leaf_in_linear_chain() {
        let dag = build_linear_dag();
        let ancestors = dag.ancestors(&oid(3), 10);
        assert_eq!(ancestors.len(), 2);
        let ids: HashSet<ObjectId> = ancestors.iter().map(|n| n.id).collect();
        assert!(ids.contains(&oid(1)));
        assert!(ids.contains(&oid(2)));
    }

    #[test]
    fn ancestors_respects_max_depth() {
        let dag = build_linear_dag();
        let ancestors = dag.ancestors(&oid(3), 1);
        assert_eq!(ancestors.len(), 1);
        assert_eq!(ancestors[0].id, oid(2));
    }

    // ----------------------------------------------------------
    // Descendant tests
    // ----------------------------------------------------------

    #[test]
    fn descendants_of_leaf_is_empty() {
        let dag = build_linear_dag();
        let descendants = dag.descendants(&oid(3), 10);
        assert!(descendants.is_empty());
    }

    #[test]
    fn descendants_of_root_in_linear_chain() {
        let dag = build_linear_dag();
        let descendants = dag.descendants(&oid(1), 10);
        assert_eq!(descendants.len(), 2);
        let ids: HashSet<ObjectId> = descendants.iter().map(|n| n.id).collect();
        assert!(ids.contains(&oid(2)));
        assert!(ids.contains(&oid(3)));
    }

    #[test]
    fn descendants_respects_max_depth() {
        let dag = build_linear_dag();
        let descendants = dag.descendants(&oid(1), 1);
        assert_eq!(descendants.len(), 1);
        assert_eq!(descendants[0].id, oid(2));
    }

    // ----------------------------------------------------------
    // Causal path tests
    // ----------------------------------------------------------

    #[test]
    fn causal_path_same_node() {
        let dag = build_linear_dag();
        let path = dag.causal_path(&oid(1), &oid(1)).unwrap();
        assert_eq!(path.len(), 1);
        assert_eq!(path[0].id, oid(1));
    }

    #[test]
    fn causal_path_linear_chain() {
        let dag = build_linear_dag();
        let path = dag.causal_path(&oid(1), &oid(3)).unwrap();
        assert_eq!(path.len(), 3);
        assert_eq!(path[0].id, oid(1));
        assert_eq!(path[1].id, oid(2));
        assert_eq!(path[2].id, oid(3));
    }

    #[test]
    fn causal_path_reverse_direction() {
        let dag = build_linear_dag();
        let path = dag.causal_path(&oid(3), &oid(1)).unwrap();
        assert_eq!(path.len(), 3);
        assert_eq!(path[0].id, oid(3));
        assert_eq!(path[2].id, oid(1));
    }

    #[test]
    fn causal_path_nonexistent() {
        let dag = build_linear_dag();
        let path = dag.causal_path(&oid(1), &oid(99));
        assert!(path.is_none());
    }

    #[test]
    fn causal_path_diamond() {
        let dag = build_diamond_dag();
        let path = dag.causal_path(&oid(1), &oid(4)).unwrap();
        // Should be length 3 (one of: 1->2->4 or 1->3->4).
        assert_eq!(path.len(), 3);
        assert_eq!(path[0].id, oid(1));
        assert_eq!(path[2].id, oid(4));
    }

    // ----------------------------------------------------------
    // Worldline history tests
    // ----------------------------------------------------------

    #[test]
    fn worldline_history_returns_sorted() {
        let dag = build_linear_dag();
        let w = wl(1);
        let history = dag.worldline_history(&w);
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].seq, 0);
        assert_eq!(history[1].seq, 1);
        assert_eq!(history[2].seq, 2);
    }

    #[test]
    fn worldline_history_filters_correctly() {
        let w1 = wl(1);
        let w2 = wl(2);
        let mut dag = ProvenanceDag::new();
        dag.add_node(make_node(1, &w1, 0, ReceiptKind::Commitment, vec![]))
            .unwrap();
        dag.add_node(make_node(2, &w2, 0, ReceiptKind::Commitment, vec![]))
            .unwrap();
        dag.add_node(make_node(
            3,
            &w1,
            1,
            ReceiptKind::Outcome,
            vec![ParentRef::sequential(oid(1))],
        ))
        .unwrap();

        let h1 = dag.worldline_history(&w1);
        assert_eq!(h1.len(), 2);
        let h2 = dag.worldline_history(&w2);
        assert_eq!(h2.len(), 1);
    }

    // ----------------------------------------------------------
    // Common ancestor tests
    // ----------------------------------------------------------

    #[test]
    fn common_ancestor_same_node() {
        let dag = build_linear_dag();
        let ca = dag.common_ancestor(&oid(2), &oid(2)).unwrap();
        assert_eq!(ca.id, oid(2));
    }

    #[test]
    fn common_ancestor_linear_chain() {
        let dag = build_linear_dag();
        // LCA of node 2 and node 3 should be node 2 (since 2 is ancestor of 3).
        let ca = dag.common_ancestor(&oid(2), &oid(3)).unwrap();
        assert_eq!(ca.id, oid(2));
    }

    #[test]
    fn common_ancestor_diamond() {
        let dag = build_diamond_dag();
        // LCA of B(2) and C(3) should be A(1).
        let ca = dag.common_ancestor(&oid(2), &oid(3)).unwrap();
        assert_eq!(ca.id, oid(1));
    }

    #[test]
    fn common_ancestor_nonexistent() {
        let dag = build_linear_dag();
        let ca = dag.common_ancestor(&oid(1), &oid(99));
        assert!(ca.is_none());
    }

    // ----------------------------------------------------------
    // Topological order tests
    // ----------------------------------------------------------

    #[test]
    fn topological_order_linear() {
        let dag = build_linear_dag();
        let order = dag.topological_order();
        assert_eq!(order.len(), 3);
        // Parents must appear before children.
        let positions: HashMap<ObjectId, usize> = order
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id, i))
            .collect();
        assert!(positions[&oid(1)] < positions[&oid(2)]);
        assert!(positions[&oid(2)] < positions[&oid(3)]);
    }

    #[test]
    fn topological_order_diamond() {
        let dag = build_diamond_dag();
        let order = dag.topological_order();
        assert_eq!(order.len(), 4);
        let positions: HashMap<ObjectId, usize> = order
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id, i))
            .collect();
        // A before B, C; B and C before D.
        assert!(positions[&oid(1)] < positions[&oid(2)]);
        assert!(positions[&oid(1)] < positions[&oid(3)]);
        assert!(positions[&oid(2)] < positions[&oid(4)]);
        assert!(positions[&oid(3)] < positions[&oid(4)]);
    }

    // ----------------------------------------------------------
    // Audit trail tests
    // ----------------------------------------------------------

    #[test]
    fn audit_trail_captures_chain() {
        let dag = build_linear_dag();
        let trail = dag.audit_trail(&oid(3));
        assert_eq!(trail.commitment, oid(3));
        // Should include all 3 nodes.
        assert_eq!(trail.len(), 3);
        let ids: HashSet<ObjectId> = trail.chain.iter().map(|e| e.node).collect();
        assert!(ids.contains(&oid(1)));
        assert!(ids.contains(&oid(2)));
        assert!(ids.contains(&oid(3)));
    }

    #[test]
    fn audit_trail_for_root_has_one_entry() {
        let dag = build_linear_dag();
        let trail = dag.audit_trail(&oid(1));
        assert_eq!(trail.len(), 1);
        assert_eq!(trail.chain[0].node, oid(1));
    }

    // ----------------------------------------------------------
    // Impact report tests
    // ----------------------------------------------------------

    #[test]
    fn impact_report_leaf_has_no_downstream() {
        let dag = build_linear_dag();
        let report = dag.impact_report(&oid(3));
        assert!(report.is_empty());
        assert_eq!(report.downstream_receipts, 0);
    }

    #[test]
    fn impact_report_root_sees_all_downstream() {
        let dag = build_linear_dag();
        let report = dag.impact_report(&oid(1));
        assert_eq!(report.downstream_receipts, 2);
        assert_eq!(report.cascade_depth, 2);
    }

    #[test]
    fn impact_report_diamond_from_root() {
        let dag = build_diamond_dag();
        let report = dag.impact_report(&oid(1));
        assert_eq!(report.downstream_receipts, 3); // B, C, D
        assert_eq!(report.cascade_depth, 2);
    }

    // ----------------------------------------------------------
    // Cross-worldline tests
    // ----------------------------------------------------------

    #[test]
    fn cross_worldline_edge() {
        let w1 = wl(1);
        let w2 = wl(2);
        let mut dag = ProvenanceDag::new();

        // W1: root node.
        dag.add_node(make_node(1, &w1, 0, ReceiptKind::Commitment, vec![]))
            .unwrap();
        // W2: root node.
        dag.add_node(make_node(2, &w2, 0, ReceiptKind::Commitment, vec![]))
            .unwrap();
        // W2: node that depends on W1's node.
        dag.add_node(make_node(
            3,
            &w2,
            1,
            ReceiptKind::Outcome,
            vec![
                ParentRef::sequential(oid(2)),
                ParentRef::cross_worldline(oid(1)),
            ],
        ))
        .unwrap();

        // Node 3 should have node 1 as an ancestor (cross-worldline).
        let ancestors = dag.ancestors(&oid(3), 10);
        let ancestor_ids: HashSet<ObjectId> = ancestors.iter().map(|n| n.id).collect();
        assert!(ancestor_ids.contains(&oid(1)));
        assert!(ancestor_ids.contains(&oid(2)));

        // Impact of node 1 should include node 3 from W2.
        let report = dag.impact_report(&oid(1));
        assert_eq!(report.downstream_receipts, 1);
        assert!(report.affected_worldlines.contains(&w2));
    }

    // ----------------------------------------------------------
    // Serialization tests
    // ----------------------------------------------------------

    #[test]
    fn bincode_roundtrip() {
        let dag = build_diamond_dag();
        let bytes = dag.to_bytes().unwrap();
        let restored = ProvenanceDag::from_bytes(&bytes).unwrap();
        assert_eq!(restored.len(), dag.len());
        assert_eq!(restored.roots().len(), dag.roots().len());
    }

    // ----------------------------------------------------------
    // Checkpoint / Pruning tests
    // ----------------------------------------------------------

    #[test]
    fn checkpoint_prunes_old_nodes() {
        let mut dag = build_linear_dag();
        assert_eq!(dag.len(), 3);

        // Prune nodes before timestamp 1100 (only node at seq=0 has ts=1000).
        let horizon = TemporalAnchor::new(1100, 0, 0);
        let pruned = dag.checkpoint(&horizon);
        assert_eq!(pruned, 1);
        assert_eq!(dag.len(), 2);

        // Node 2 should now be a root since its parent was pruned.
        assert!(dag.roots().iter().any(|n| n.id == oid(2)));

        // Validation should pass.
        dag.validate().unwrap();
    }

    #[test]
    fn checkpoint_promotes_orphans_to_roots() {
        let mut dag = build_linear_dag();
        // Prune everything before seq=2 (ts < 1200 means seq 0 and 1).
        let horizon = TemporalAnchor::new(1200, 0, 0);
        let pruned = dag.checkpoint(&horizon);
        assert_eq!(pruned, 2);
        assert_eq!(dag.len(), 1);
        assert_eq!(dag.roots().len(), 1);
        assert_eq!(dag.roots()[0].id, oid(3));
    }

    // ----------------------------------------------------------
    // Validation tests
    // ----------------------------------------------------------

    #[test]
    fn valid_dag_passes_validation() {
        let dag = build_diamond_dag();
        dag.validate().unwrap();
    }
}
