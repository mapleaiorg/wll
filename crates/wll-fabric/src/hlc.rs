use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use wll_types::TemporalAnchor;

/// Internal mutable state of the Hybrid Logical Clock.
struct HlcState {
    /// Last-known physical millisecond timestamp.
    physical_ms: u64,
    /// Logical counter for events within the same physical millisecond.
    logical: u32,
}

/// Hybrid Logical Clock for causal event ordering.
///
/// Combines wall-clock time with a logical counter to produce monotonically
/// increasing [`TemporalAnchor`] values. Safe for concurrent use across
/// threads via an internal [`Mutex`].
///
/// # HLC Rules
///
/// - **Local event**: `physical = max(wall_clock, state.physical)`.
///   If physical advanced, `logical = 0`; otherwise `logical += 1`.
/// - **Receive**: `physical = max(wall_clock, state.physical, received.physical)`,
///   with logical adjusted to be strictly greater than both local and received
///   counters when the physical component ties.
/// - **Guarantee**: timestamps are monotonic and preserve causal ordering.
pub struct HybridLogicalClock {
    /// Unique identifier for this node.
    node_id: u16,
    /// Mutable clock state protected by a mutex.
    state: Mutex<HlcState>,
}

impl HybridLogicalClock {
    /// Create a new HLC for the given node.
    pub fn new(node_id: u16) -> Self {
        Self {
            node_id,
            state: Mutex::new(HlcState {
                physical_ms: 0,
                logical: 0,
            }),
        }
    }

    /// Generate a new monotonic timestamp for a local event.
    ///
    /// The returned [`TemporalAnchor`] is guaranteed to be strictly greater
    /// than any previously returned value from this clock.
    pub fn now(&self) -> TemporalAnchor {
        let wall = Self::wall_clock_ms();
        let mut state = self.state.lock().expect("HLC mutex poisoned");

        let new_physical = wall.max(state.physical_ms);

        let new_logical = if new_physical > state.physical_ms {
            // Physical clock advanced; reset logical counter.
            0
        } else {
            // Same physical tick; increment logical counter.
            state.logical + 1
        };

        state.physical_ms = new_physical;
        state.logical = new_logical;

        TemporalAnchor::new(new_physical, new_logical, self.node_id)
    }

    /// Update the clock on receipt of a remote timestamp, returning a new
    /// timestamp that is strictly greater than both the local state and the
    /// received anchor.
    pub fn update(&self, received: &TemporalAnchor) -> TemporalAnchor {
        let wall = Self::wall_clock_ms();
        let mut state = self.state.lock().expect("HLC mutex poisoned");

        let new_physical = wall.max(state.physical_ms).max(received.physical_ms);

        let new_logical = if new_physical > state.physical_ms
            && new_physical > received.physical_ms
        {
            // Wall clock is ahead of both; reset logical counter.
            0
        } else if new_physical == state.physical_ms
            && new_physical == received.physical_ms
        {
            // All three are equal; take max of both logical + 1.
            state.logical.max(received.logical) + 1
        } else if new_physical == state.physical_ms {
            // Tied with local only; increment local logical.
            state.logical + 1
        } else {
            // Tied with received only; increment received logical.
            received.logical + 1
        };

        state.physical_ms = new_physical;
        state.logical = new_logical;

        TemporalAnchor::new(new_physical, new_logical, self.node_id)
    }

    /// The node identifier this clock was created with.
    pub fn node_id(&self) -> u16 {
        self.node_id
    }

    /// Current wall-clock time in milliseconds since the UNIX epoch.
    fn wall_clock_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotonic_across_rapid_calls() {
        let hlc = HybridLogicalClock::new(1);
        let mut prev = hlc.now();
        for _ in 0..1000 {
            let next = hlc.now();
            assert!(next > prev, "HLC must be strictly monotonic: {prev:?} >= {next:?}");
            prev = next;
        }
    }

    #[test]
    fn logical_increments_within_same_physical() {
        let hlc = HybridLogicalClock::new(1);
        // Force the physical clock to a fixed value by setting state directly.
        {
            let mut state = hlc.state.lock().unwrap();
            state.physical_ms = u64::MAX; // Far future; wall clock can never exceed.
            state.logical = 0;
        }
        let t1 = hlc.now();
        let t2 = hlc.now();
        let t3 = hlc.now();

        assert_eq!(t1.physical_ms, u64::MAX);
        assert_eq!(t1.logical, 1); // incremented from 0
        assert_eq!(t2.logical, 2);
        assert_eq!(t3.logical, 3);
        assert!(t1 < t2);
        assert!(t2 < t3);
    }

    #[test]
    fn node_id_is_preserved() {
        let hlc = HybridLogicalClock::new(42);
        let ts = hlc.now();
        assert_eq!(ts.node_id, 42);
        assert_eq!(hlc.node_id(), 42);
    }

    #[test]
    fn update_advances_past_received() {
        let hlc = HybridLogicalClock::new(1);

        // Simulate receiving a timestamp from the far future.
        let remote = TemporalAnchor::new(u64::MAX - 1, 10, 2);
        let updated = hlc.update(&remote);

        assert!(updated > remote, "updated must be > received: {updated:?} vs {remote:?}");
    }

    #[test]
    fn update_when_local_is_ahead() {
        let hlc = HybridLogicalClock::new(1);
        // Push local clock far into the future.
        {
            let mut state = hlc.state.lock().unwrap();
            state.physical_ms = u64::MAX;
            state.logical = 100;
        }
        let remote = TemporalAnchor::new(1000, 5, 2);
        let updated = hlc.update(&remote);

        assert_eq!(updated.physical_ms, u64::MAX);
        assert_eq!(updated.logical, 101);
        assert_eq!(updated.node_id, 1);
    }

    #[test]
    fn update_when_all_three_tie() {
        let hlc = HybridLogicalClock::new(1);
        let far_future = u64::MAX;
        {
            let mut state = hlc.state.lock().unwrap();
            state.physical_ms = far_future;
            state.logical = 5;
        }
        // Remote has the same physical but higher logical.
        let remote = TemporalAnchor::new(far_future, 10, 2);
        let updated = hlc.update(&remote);

        assert_eq!(updated.physical_ms, far_future);
        // max(5, 10) + 1 = 11
        assert_eq!(updated.logical, 11);
    }

    #[test]
    fn concurrent_now_calls_are_monotonic() {
        use std::sync::Arc;
        use std::thread;

        let hlc = Arc::new(HybridLogicalClock::new(1));
        let mut handles = Vec::new();

        for _ in 0..8 {
            let hlc = Arc::clone(&hlc);
            handles.push(thread::spawn(move || {
                let mut timestamps = Vec::with_capacity(100);
                for _ in 0..100 {
                    timestamps.push(hlc.now());
                }
                timestamps
            }));
        }

        let mut all_timestamps: Vec<TemporalAnchor> = Vec::new();
        for handle in handles {
            all_timestamps.extend(handle.join().unwrap());
        }

        // All timestamps must be unique (monotonic per thread, unique globally).
        let len = all_timestamps.len();
        all_timestamps.sort();
        all_timestamps.dedup();
        assert_eq!(
            all_timestamps.len(),
            len,
            "all timestamps must be unique across threads"
        );
    }
}
