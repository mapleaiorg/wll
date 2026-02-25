use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Hybrid Logical Clock timestamp for causal ordering.
///
/// Combines a physical wall-clock component with a logical counter and a
/// node identifier. This allows distributed nodes to establish causal
/// ordering without requiring precisely synchronized clocks.
///
/// Ordering: `physical_ms` → `logical` → `node_id` (total order).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TemporalAnchor {
    /// Wall-clock milliseconds since UNIX epoch.
    pub physical_ms: u64,
    /// Logical counter for events at the same physical time.
    pub logical: u32,
    /// Node identifier to break ties between nodes.
    pub node_id: u16,
}

impl TemporalAnchor {
    /// Create a new anchor with explicit values.
    pub fn new(physical_ms: u64, logical: u32, node_id: u16) -> Self {
        Self {
            physical_ms,
            logical,
            node_id,
        }
    }

    /// Create an anchor for the current wall-clock time.
    pub fn now(node_id: u16) -> Self {
        let physical_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            physical_ms,
            logical: 0,
            node_id,
        }
    }

    /// The zero anchor (genesis).
    pub const fn zero() -> Self {
        Self {
            physical_ms: 0,
            logical: 0,
            node_id: 0,
        }
    }

    /// Returns `true` if this anchor is causally after `other`.
    pub fn is_after(&self, other: &Self) -> bool {
        self > other
    }

    /// Returns `true` if this anchor is causally before `other`.
    pub fn is_before(&self, other: &Self) -> bool {
        self < other
    }

    /// Advance this anchor, ensuring it is strictly after the given anchor.
    /// Used in HLC update on message receive.
    pub fn advance(&self, received: &Self, node_id: u16) -> Self {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let max_physical = now_ms.max(self.physical_ms).max(received.physical_ms);

        let logical = if max_physical == self.physical_ms
            && max_physical == received.physical_ms
        {
            self.logical.max(received.logical) + 1
        } else if max_physical == self.physical_ms {
            self.logical + 1
        } else if max_physical == received.physical_ms {
            received.logical + 1
        } else {
            0
        };

        Self {
            physical_ms: max_physical,
            logical,
            node_id,
        }
    }
}

impl PartialOrd for TemporalAnchor {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TemporalAnchor {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.physical_ms
            .cmp(&other.physical_ms)
            .then(self.logical.cmp(&other.logical))
            .then(self.node_id.cmp(&other.node_id))
    }
}

impl fmt::Debug for TemporalAnchor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TemporalAnchor({}ms.{}.n{})",
            self.physical_ms, self.logical, self.node_id
        )
    }
}

impl fmt::Display for TemporalAnchor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}.n{}",
            self.physical_ms, self.logical, self.node_id
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering_physical_first() {
        let a = TemporalAnchor::new(100, 5, 1);
        let b = TemporalAnchor::new(200, 0, 0);
        assert!(a < b);
    }

    #[test]
    fn ordering_logical_second() {
        let a = TemporalAnchor::new(100, 1, 9);
        let b = TemporalAnchor::new(100, 2, 0);
        assert!(a < b);
    }

    #[test]
    fn ordering_node_id_third() {
        let a = TemporalAnchor::new(100, 1, 1);
        let b = TemporalAnchor::new(100, 1, 2);
        assert!(a < b);
    }

    #[test]
    fn equal_anchors() {
        let a = TemporalAnchor::new(100, 1, 1);
        let b = TemporalAnchor::new(100, 1, 1);
        assert_eq!(a, b);
        assert!(!a.is_after(&b));
        assert!(!a.is_before(&b));
    }

    #[test]
    fn now_produces_reasonable_timestamp() {
        let anchor = TemporalAnchor::now(0);
        // Should be after 2020-01-01 (1577836800000 ms)
        assert!(anchor.physical_ms > 1_577_836_800_000);
        assert_eq!(anchor.logical, 0);
        assert_eq!(anchor.node_id, 0);
    }

    #[test]
    fn zero_is_smallest() {
        let zero = TemporalAnchor::zero();
        let any = TemporalAnchor::new(1, 0, 0);
        assert!(zero < any);
    }

    #[test]
    fn advance_increases_monotonically() {
        let local = TemporalAnchor::new(100, 3, 1);
        let received = TemporalAnchor::new(100, 5, 2);
        let advanced = local.advance(&received, 1);
        assert!(advanced > local);
        assert!(advanced > received);
    }

    #[test]
    fn serde_roundtrip() {
        let anchor = TemporalAnchor::new(1234567890, 42, 7);
        let json = serde_json::to_string(&anchor).unwrap();
        let parsed: TemporalAnchor = serde_json::from_str(&json).unwrap();
        assert_eq!(anchor, parsed);
    }

    #[test]
    fn display_format() {
        let anchor = TemporalAnchor::new(1000, 5, 3);
        assert_eq!(format!("{anchor}"), "1000.5.n3");
    }
}
