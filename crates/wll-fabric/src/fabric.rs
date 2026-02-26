use std::path::Path;
use std::sync::RwLock;

use tokio::sync::broadcast;
use tracing::{debug, info};

use wll_types::{TemporalAnchor, WorldlineId};

use crate::error::Result;
use crate::event::{EventKind, EventPayload, FabricEvent};
use crate::hlc::HybridLogicalClock;
use crate::wal::{WalConfig, WalEntry, WriteAheadLog};

/// Filter for subscribing to a subset of fabric events.
#[derive(Clone, Debug, Default)]
pub struct EventFilter {
    /// If set, only events for these worldlines are delivered.
    pub worldlines: Option<Vec<WorldlineId>>,
    /// If set, only events of these kinds are delivered.
    pub kinds: Option<Vec<EventKind>>,
    /// If set, only events with timestamps after this anchor are delivered.
    pub since: Option<TemporalAnchor>,
}

impl EventFilter {
    /// Returns `true` if the given event matches this filter.
    pub fn matches(&self, event: &FabricEvent) -> bool {
        if let Some(ref wls) = self.worldlines {
            if !wls.contains(&event.worldline) {
                return false;
            }
        }
        if let Some(ref kinds) = self.kinds {
            if !kinds.contains(&event.kind) {
                return false;
            }
        }
        if let Some(ref since) = self.since {
            if !event.timestamp.is_after(since) {
                return false;
            }
        }
        true
    }
}

/// A broadcast channel receiver for fabric events.
pub type EventStream = broadcast::Receiver<FabricEvent>;

/// Internal subscriber: a filter paired with a broadcast sender.
struct Subscriber {
    filter: EventFilter,
    sender: broadcast::Sender<FabricEvent>,
}

/// Fan-out router that delivers events to matching subscribers.
struct EventRouter {
    subscribers: RwLock<Vec<Subscriber>>,
}

impl EventRouter {
    fn new() -> Self {
        Self {
            subscribers: RwLock::new(Vec::new()),
        }
    }

    /// Register a new subscriber with the given filter.
    /// Returns a broadcast receiver for the matching events.
    fn subscribe(&self, filter: EventFilter, capacity: usize) -> EventStream {
        let (tx, rx) = broadcast::channel(capacity);
        let sub = Subscriber {
            filter,
            sender: tx,
        };
        self.subscribers
            .write()
            .expect("router lock poisoned")
            .push(sub);
        rx
    }

    /// Route an event to all matching subscribers.
    /// Subscribers whose channels are closed are pruned.
    fn route(&self, event: &FabricEvent) {
        let mut subs = self.subscribers.write().expect("router lock poisoned");
        subs.retain(|sub| {
            if sub.filter.matches(event) {
                // If send fails (no receivers), the subscriber is stale.
                sub.sender.send(event.clone()).is_ok()
            } else {
                // Keep non-matching subscribers; they may match future events.
                // Only prune if the channel itself is closed.
                sub.sender.receiver_count() > 0
            }
        });
    }

    /// Number of active subscribers.
    fn subscriber_count(&self) -> usize {
        self.subscribers
            .read()
            .expect("router lock poisoned")
            .len()
    }
}

/// Configuration for the [`EventFabric`].
#[derive(Clone, Debug)]
pub struct FabricConfig {
    /// Node identifier for the HLC.
    pub node_id: u16,
    /// WAL configuration.
    pub wal: WalConfig,
    /// Capacity of per-subscriber broadcast channels.
    pub channel_capacity: usize,
}

impl Default for FabricConfig {
    fn default() -> Self {
        Self {
            node_id: 0,
            wal: WalConfig::default(),
            channel_capacity: 1024,
        }
    }
}

/// Central event fabric: crash-recoverable event bus with causal ordering.
///
/// Combines a [`HybridLogicalClock`] for causal timestamps, a
/// [`WriteAheadLog`] for crash recovery, and an [`EventRouter`] for
/// fan-out delivery to subscribers.
pub struct EventFabric {
    hlc: HybridLogicalClock,
    wal: WriteAheadLog,
    router: EventRouter,
    config: FabricConfig,
}

impl EventFabric {
    /// Create a new fabric, opening (or creating) the WAL at the given path.
    pub fn new(wal_path: &Path, config: FabricConfig) -> Result<Self> {
        let hlc = HybridLogicalClock::new(config.node_id);
        let wal = WriteAheadLog::open(wal_path, config.wal.clone())?;
        let router = EventRouter::new();

        info!(node_id = config.node_id, wal_path = %wal_path.display(), "fabric started");

        Ok(Self {
            hlc,
            wal,
            router,
            config,
        })
    }

    /// Emit a single event through the fabric.
    ///
    /// The event is stamped with the next HLC tick, persisted to the WAL,
    /// and routed to matching subscribers.
    pub fn emit(
        &self,
        worldline: WorldlineId,
        kind: EventKind,
        payload: EventPayload,
    ) -> Result<FabricEvent> {
        let timestamp = self.hlc.now();
        let event = FabricEvent::new(timestamp, worldline, kind, payload);

        // Persist to WAL first (write-ahead guarantee).
        self.wal.append(&WalEntry {
            event: event.clone(),
        })?;

        // Fan out to subscribers.
        self.router.route(&event);

        debug!(id = %event.id, kind = %event.kind, "event emitted");
        Ok(event)
    }

    /// Emit a batch of events atomically.
    ///
    /// All events are written to the WAL before any are routed.
    pub fn emit_batch(
        &self,
        events: Vec<(WorldlineId, EventKind, EventPayload)>,
    ) -> Result<Vec<FabricEvent>> {
        let mut stamped = Vec::with_capacity(events.len());

        for (worldline, kind, payload) in events {
            let timestamp = self.hlc.now();
            let event = FabricEvent::new(timestamp, worldline, kind, payload);
            stamped.push(event);
        }

        // WAL phase: persist all events.
        for event in &stamped {
            self.wal.append(&WalEntry {
                event: event.clone(),
            })?;
        }

        // Route phase: fan out to subscribers.
        for event in &stamped {
            self.router.route(event);
        }

        debug!(count = stamped.len(), "batch emitted");
        Ok(stamped)
    }

    /// Subscribe to events matching the given filter.
    pub fn subscribe(&self, filter: EventFilter) -> EventStream {
        self.router.subscribe(filter, self.config.channel_capacity)
    }

    /// Recover all events from the WAL.
    ///
    /// Used after a crash to replay events that were persisted but not yet
    /// fully processed.
    pub fn recover(&self) -> Result<Vec<FabricEvent>> {
        let entries = self.wal.recover()?;
        let events: Vec<FabricEvent> = entries.into_iter().map(|e| e.event).collect();
        info!(count = events.len(), "recovered events from WAL");
        Ok(events)
    }

    /// Checkpoint the WAL, marking all current data as committed.
    pub fn checkpoint(&self) -> Result<()> {
        let offset = self.wal.offset();
        if offset > 0 {
            self.wal.checkpoint(offset)?;
        }
        Ok(())
    }

    /// Update the HLC with a received remote timestamp.
    pub fn update_clock(&self, received: &TemporalAnchor) -> TemporalAnchor {
        self.hlc.update(received)
    }

    /// Current number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.router.subscriber_count()
    }

    /// Reference to the underlying HLC.
    pub fn hlc(&self) -> &HybridLogicalClock {
        &self.hlc
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wll_types::IdentityMaterial;

    fn test_worldline() -> WorldlineId {
        WorldlineId::derive(&IdentityMaterial::GenesisHash([42u8; 32]))
    }

    fn temp_fabric() -> (tempfile::TempDir, EventFabric) {
        let dir = tempfile::tempdir().unwrap();
        let wal_path = dir.path().join("fabric.wal");
        let config = FabricConfig::default();
        let fabric = EventFabric::new(&wal_path, config).unwrap();
        (dir, fabric)
    }

    #[test]
    fn emit_and_recover() {
        let (dir, fabric) = temp_fabric();

        let wl = test_worldline();
        fabric
            .emit(wl.clone(), EventKind::WorldlineCreated, EventPayload::Empty)
            .unwrap();
        fabric
            .emit(wl.clone(), EventKind::CommitmentProposed, EventPayload::Empty)
            .unwrap();

        // Recover from WAL (simulate restart).
        let wal_path = dir.path().join("fabric.wal");
        let fabric2 = EventFabric::new(&wal_path, FabricConfig::default()).unwrap();
        let recovered = fabric2.recover().unwrap();

        assert_eq!(recovered.len(), 2);
        assert_eq!(recovered[0].kind, EventKind::WorldlineCreated);
        assert_eq!(recovered[1].kind, EventKind::CommitmentProposed);
    }

    #[test]
    fn emit_batch_all_persisted() {
        let (dir, fabric) = temp_fabric();
        let wl = test_worldline();

        let batch = vec![
            (wl.clone(), EventKind::SyncStarted, EventPayload::Empty),
            (wl.clone(), EventKind::SyncCompleted, EventPayload::Empty),
            (wl.clone(), EventKind::RefUpdated, EventPayload::Empty),
        ];

        let events = fabric.emit_batch(batch).unwrap();
        assert_eq!(events.len(), 3);

        // Timestamps must be monotonically increasing.
        assert!(events[0].timestamp < events[1].timestamp);
        assert!(events[1].timestamp < events[2].timestamp);

        // Recover
        let wal_path = dir.path().join("fabric.wal");
        let fabric2 = EventFabric::new(&wal_path, FabricConfig::default()).unwrap();
        let recovered = fabric2.recover().unwrap();
        assert_eq!(recovered.len(), 3);
    }

    #[test]
    fn subscriber_receives_matching_events() {
        let (_dir, fabric) = temp_fabric();
        let wl = test_worldline();

        let filter = EventFilter {
            kinds: Some(vec![EventKind::CommitmentProposed]),
            ..Default::default()
        };

        let mut stream = fabric.subscribe(filter);
        assert_eq!(fabric.subscriber_count(), 1);

        // Emit matching event.
        fabric
            .emit(
                wl.clone(),
                EventKind::CommitmentProposed,
                EventPayload::Empty,
            )
            .unwrap();

        // Emit non-matching event.
        fabric
            .emit(wl.clone(), EventKind::RefUpdated, EventPayload::Empty)
            .unwrap();

        // Should receive only the matching event.
        let received = stream.try_recv().unwrap();
        assert_eq!(received.kind, EventKind::CommitmentProposed);

        // No more matching events.
        assert!(stream.try_recv().is_err());
    }

    #[test]
    fn subscriber_worldline_filter() {
        let (_dir, fabric) = temp_fabric();
        let wl1 = WorldlineId::derive(&IdentityMaterial::GenesisHash([1u8; 32]));
        let wl2 = WorldlineId::derive(&IdentityMaterial::GenesisHash([2u8; 32]));

        let filter = EventFilter {
            worldlines: Some(vec![wl1.clone()]),
            ..Default::default()
        };
        let mut stream = fabric.subscribe(filter);

        fabric
            .emit(wl1.clone(), EventKind::WorldlineCreated, EventPayload::Empty)
            .unwrap();
        fabric
            .emit(wl2.clone(), EventKind::WorldlineCreated, EventPayload::Empty)
            .unwrap();

        let received = stream.try_recv().unwrap();
        assert_eq!(received.worldline, wl1);
        assert!(stream.try_recv().is_err());
    }

    #[test]
    fn checkpoint_clears_wal() {
        let (dir, fabric) = temp_fabric();
        let wl = test_worldline();

        fabric
            .emit(wl.clone(), EventKind::SnapshotCreated, EventPayload::Empty)
            .unwrap();
        fabric.checkpoint().unwrap();

        // After checkpoint + re-open, WAL should be empty.
        let wal_path = dir.path().join("fabric.wal");
        let fabric2 = EventFabric::new(&wal_path, FabricConfig::default()).unwrap();
        let recovered = fabric2.recover().unwrap();
        assert!(recovered.is_empty());
    }

    #[test]
    fn event_integrity_verified_on_recover() {
        let (dir, fabric) = temp_fabric();
        let wl = test_worldline();

        let event = fabric
            .emit(wl, EventKind::OutcomeRecorded, EventPayload::Empty)
            .unwrap();
        assert!(event.verify_integrity());

        let wal_path = dir.path().join("fabric.wal");
        let fabric2 = EventFabric::new(&wal_path, FabricConfig::default()).unwrap();
        let recovered = fabric2.recover().unwrap();
        assert_eq!(recovered.len(), 1);
        assert!(recovered[0].verify_integrity());
    }

    #[test]
    fn concurrent_emit_is_safe() {
        use std::sync::Arc;
        use std::thread;

        let dir = tempfile::tempdir().unwrap();
        let wal_path = dir.path().join("concurrent.wal");
        let fabric = Arc::new(EventFabric::new(&wal_path, FabricConfig::default()).unwrap());

        let mut handles = Vec::new();
        for i in 0u8..4 {
            let fabric = Arc::clone(&fabric);
            handles.push(thread::spawn(move || {
                let wl = WorldlineId::derive(&IdentityMaterial::GenesisHash([i; 32]));
                for _ in 0..25 {
                    fabric
                        .emit(wl.clone(), EventKind::CommitmentProposed, EventPayload::Empty)
                        .unwrap();
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // All 100 events should be recoverable.
        let fabric2 = EventFabric::new(&wal_path, FabricConfig::default()).unwrap();
        let recovered = fabric2.recover().unwrap();
        assert_eq!(recovered.len(), 100);

        // All should have valid integrity hashes.
        for event in &recovered {
            assert!(event.verify_integrity());
        }
    }

    #[test]
    fn filter_matches_correctly() {
        let wl = test_worldline();
        let event = FabricEvent::new(
            TemporalAnchor::new(1000, 0, 1),
            wl.clone(),
            EventKind::CommitmentDecided,
            EventPayload::Empty,
        );

        // Empty filter matches everything.
        let filter = EventFilter::default();
        assert!(filter.matches(&event));

        // Kind filter.
        let filter = EventFilter {
            kinds: Some(vec![EventKind::CommitmentDecided]),
            ..Default::default()
        };
        assert!(filter.matches(&event));

        let filter = EventFilter {
            kinds: Some(vec![EventKind::RefUpdated]),
            ..Default::default()
        };
        assert!(!filter.matches(&event));

        // Since filter.
        let filter = EventFilter {
            since: Some(TemporalAnchor::new(999, 0, 0)),
            ..Default::default()
        };
        assert!(filter.matches(&event));

        let filter = EventFilter {
            since: Some(TemporalAnchor::new(2000, 0, 0)),
            ..Default::default()
        };
        assert!(!filter.matches(&event));
    }
}
