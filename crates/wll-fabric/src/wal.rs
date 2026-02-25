use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::error::{FabricError, Result};
use crate::event::FabricEvent;

/// WAL entry: a single serialized event with length and CRC framing.
///
/// On-disk format:
/// ```text
/// [4 bytes: entry length (little-endian u32)]
/// [4 bytes: CRC32 of payload (little-endian u32)]
/// [N bytes: payload (bincode-serialized FabricEvent)]
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalEntry {
    /// The fabric event stored in this WAL entry.
    pub event: FabricEvent,
}

/// Flush/sync strategy for the WAL.
#[derive(Clone, Debug)]
pub enum SyncMode {
    /// `fsync` after every write (safest, highest latency).
    EveryWrite,
    /// `fsync` periodically at the given interval.
    Periodic(Duration),
    /// Rely on OS page-cache buffering (fastest, least durable).
    OsDefault,
}

impl Default for SyncMode {
    fn default() -> Self {
        Self::OsDefault
    }
}

/// Retention policy for WAL segments after checkpoint.
#[derive(Clone, Debug)]
pub enum WalRetention {
    /// Delete WAL data that has been checkpointed.
    DeleteOnCheckpoint,
    /// Keep all WAL data (useful for auditing).
    KeepAll,
}

impl Default for WalRetention {
    fn default() -> Self {
        Self::DeleteOnCheckpoint
    }
}

/// Configuration for the Write-Ahead Log.
#[derive(Clone, Debug)]
pub struct WalConfig {
    /// Maximum segment size in bytes (default: 64 MiB).
    pub max_segment_size: u64,
    /// Sync/flush strategy.
    pub sync_mode: SyncMode,
    /// Retention policy after checkpoint.
    pub retention: WalRetention,
}

impl Default for WalConfig {
    fn default() -> Self {
        Self {
            max_segment_size: 64 * 1024 * 1024, // 64 MiB
            sync_mode: SyncMode::default(),
            retention: WalRetention::default(),
        }
    }
}

/// Header size: 4 bytes length + 4 bytes CRC.
const HEADER_SIZE: usize = 8;

/// Internal mutable state for the WAL writer.
struct WalWriter {
    writer: BufWriter<File>,
    /// Current write offset in the segment file.
    offset: u64,
}

/// Crash-recoverable Write-Ahead Log.
///
/// Events are serialized with bincode, framed with a length prefix and a
/// CRC32 checksum, and written to a single segment file. On recovery the
/// file is read front-to-back; entries that fail the CRC check are skipped
/// (they represent incomplete/torn writes from a crash).
pub struct WriteAheadLog {
    /// Path to the WAL segment file.
    path: PathBuf,
    /// Writer state behind a mutex for thread safety.
    writer: Mutex<WalWriter>,
    /// Configuration.
    config: WalConfig,
}

impl WriteAheadLog {
    /// Open (or create) a WAL segment file at the given path.
    pub fn open(path: &Path, config: WalConfig) -> Result<Self> {
        // Ensure parent directory exists.
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(path)?;

        let offset = file.metadata()?.len();
        let writer = BufWriter::new(file);

        Ok(Self {
            path: path.to_path_buf(),
            writer: Mutex::new(WalWriter { writer, offset }),
            config,
        })
    }

    /// Append a single entry to the WAL. Returns the byte offset of the entry.
    pub fn append(&self, entry: &WalEntry) -> Result<u64> {
        let payload = bincode::serialize(&entry.event)
            .map_err(|e| FabricError::Serialization(e.to_string()))?;

        let length = payload.len() as u32;
        let crc = crc32fast::hash(&payload);

        let mut w = self.writer.lock().expect("WAL mutex poisoned");
        let entry_offset = w.offset;

        // Write header: [length: u32 LE] [crc: u32 LE]
        w.writer.write_all(&length.to_le_bytes())?;
        w.writer.write_all(&crc.to_le_bytes())?;
        // Write payload
        w.writer.write_all(&payload)?;

        // Sync if configured for every write.
        if matches!(self.config.sync_mode, SyncMode::EveryWrite) {
            w.writer.flush()?;
            w.writer.get_ref().sync_all()?;
        } else {
            w.writer.flush()?;
        }

        w.offset += HEADER_SIZE as u64 + payload.len() as u64;

        debug!(offset = entry_offset, len = payload.len(), "WAL append");
        Ok(entry_offset)
    }

    /// Recover all valid entries from the WAL segment.
    ///
    /// Reads the file front-to-back. Entries that fail CRC validation are
    /// logged and skipped (they represent torn writes from a crash).
    pub fn recover(&self) -> Result<Vec<WalEntry>> {
        let mut file = BufReader::new(File::open(&self.path)?);
        let file_len = file.get_ref().metadata()?.len();
        let mut entries = Vec::new();
        let mut offset: u64 = 0;

        while offset + HEADER_SIZE as u64 <= file_len {
            file.seek(SeekFrom::Start(offset))?;

            // Read header
            let mut header_buf = [0u8; HEADER_SIZE];
            match file.read_exact(&mut header_buf) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e.into()),
            }

            let length = u32::from_le_bytes([header_buf[0], header_buf[1], header_buf[2], header_buf[3]]);
            let expected_crc = u32::from_le_bytes([header_buf[4], header_buf[5], header_buf[6], header_buf[7]]);

            // Validate length
            if length == 0 || (offset + HEADER_SIZE as u64 + length as u64) > file_len {
                warn!(
                    offset,
                    length,
                    file_len,
                    "invalid WAL entry length; stopping recovery"
                );
                break;
            }

            // Read payload
            let mut payload = vec![0u8; length as usize];
            match file.read_exact(&mut payload) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    warn!(offset, "truncated WAL entry; stopping recovery");
                    break;
                }
                Err(e) => return Err(e.into()),
            }

            // CRC check
            let actual_crc = crc32fast::hash(&payload);
            if actual_crc != expected_crc {
                warn!(
                    offset,
                    expected = expected_crc,
                    actual = actual_crc,
                    "CRC mismatch; skipping entry"
                );
                offset += HEADER_SIZE as u64 + length as u64;
                continue;
            }

            // Deserialize
            match bincode::deserialize::<FabricEvent>(&payload) {
                Ok(event) => {
                    entries.push(WalEntry { event });
                }
                Err(e) => {
                    warn!(offset, error = %e, "failed to deserialize WAL entry; skipping");
                }
            }

            offset += HEADER_SIZE as u64 + length as u64;
        }

        debug!(recovered = entries.len(), "WAL recovery complete");
        Ok(entries)
    }

    /// Checkpoint: mark all entries up to `through_offset` as committed.
    ///
    /// With `DeleteOnCheckpoint` retention, the WAL is truncated to remove
    /// all data up to and including the given offset.
    pub fn checkpoint(&self, through_offset: u64) -> Result<()> {
        let w = self.writer.lock().expect("WAL mutex poisoned");

        if through_offset > w.offset {
            return Err(FabricError::InvalidCheckpoint {
                requested: through_offset,
                current: w.offset,
            });
        }
        drop(w);

        if matches!(self.config.retention, WalRetention::DeleteOnCheckpoint) {
            self.truncate_through(through_offset)?;
        }

        debug!(through_offset, "WAL checkpoint");
        Ok(())
    }

    /// Truncate the entire WAL (remove all data).
    pub fn truncate(&self) -> Result<()> {
        let mut w = self.writer.lock().expect("WAL mutex poisoned");

        // Truncate the file to zero.
        let file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.path)?;

        w.writer = BufWriter::new(file);
        w.offset = 0;

        debug!("WAL truncated");
        Ok(())
    }

    /// Current write offset.
    pub fn offset(&self) -> u64 {
        self.writer.lock().expect("WAL mutex poisoned").offset
    }

    /// Path to the WAL segment file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Truncate data up through the given offset by rewriting remaining data.
    fn truncate_through(&self, through_offset: u64) -> Result<()> {
        // Read remaining data after the checkpoint offset.
        let mut file = File::open(&self.path)?;
        let file_len = file.metadata()?.len();

        if through_offset >= file_len {
            // Everything is checkpointed; truncate to empty.
            return self.truncate();
        }

        file.seek(SeekFrom::Start(through_offset))?;
        let mut remaining = Vec::new();
        file.read_to_end(&mut remaining)?;
        drop(file);

        // Rewrite the file with only the remaining data.
        let mut w = self.writer.lock().expect("WAL mutex poisoned");
        let file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.path)?;
        let mut buf_writer = BufWriter::new(file);
        buf_writer.write_all(&remaining)?;
        buf_writer.flush()?;

        w.offset = remaining.len() as u64;
        w.writer = buf_writer;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EventKind, EventPayload, FabricEvent};
    use wll_types::{IdentityMaterial, TemporalAnchor, WorldlineId};

    fn test_worldline() -> WorldlineId {
        WorldlineId::derive(&IdentityMaterial::GenesisHash([42u8; 32]))
    }

    fn make_entry(seq: u32) -> WalEntry {
        WalEntry {
            event: FabricEvent::new(
                TemporalAnchor::new(1000 + seq as u64, seq, 1),
                test_worldline(),
                EventKind::CommitmentProposed,
                EventPayload::Empty,
            ),
        }
    }

    #[test]
    fn append_and_recover_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let wal_path = dir.path().join("test.wal");
        let wal = WriteAheadLog::open(&wal_path, WalConfig::default()).unwrap();

        let entry1 = make_entry(1);
        let entry2 = make_entry(2);
        let entry3 = make_entry(3);

        wal.append(&entry1).unwrap();
        wal.append(&entry2).unwrap();
        wal.append(&entry3).unwrap();

        let recovered = wal.recover().unwrap();
        assert_eq!(recovered.len(), 3);
        assert_eq!(recovered[0], entry1);
        assert_eq!(recovered[1], entry2);
        assert_eq!(recovered[2], entry3);
    }

    #[test]
    fn recover_empty_wal() {
        let dir = tempfile::tempdir().unwrap();
        let wal_path = dir.path().join("empty.wal");
        let wal = WriteAheadLog::open(&wal_path, WalConfig::default()).unwrap();

        let recovered = wal.recover().unwrap();
        assert!(recovered.is_empty());
    }

    #[test]
    fn crc_detects_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let wal_path = dir.path().join("corrupt.wal");
        let wal = WriteAheadLog::open(&wal_path, WalConfig::default()).unwrap();

        wal.append(&make_entry(1)).unwrap();
        wal.append(&make_entry(2)).unwrap();
        drop(wal);

        // Corrupt the payload of the first entry (byte 8 is first payload byte).
        {
            let mut file = OpenOptions::new()
                .write(true)
                .read(true)
                .open(&wal_path)
                .unwrap();
            file.seek(SeekFrom::Start(HEADER_SIZE as u64)).unwrap();
            // Flip a byte in the payload.
            let mut buf = [0u8; 1];
            file.read_exact(&mut buf).unwrap();
            buf[0] ^= 0xFF;
            file.seek(SeekFrom::Start(HEADER_SIZE as u64)).unwrap();
            file.write_all(&buf).unwrap();
            file.sync_all().unwrap();
        }

        let wal = WriteAheadLog::open(&wal_path, WalConfig::default()).unwrap();
        let recovered = wal.recover().unwrap();

        // First entry should be skipped due to CRC failure; second should survive.
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0], make_entry(2));
    }

    #[test]
    fn truncate_clears_wal() {
        let dir = tempfile::tempdir().unwrap();
        let wal_path = dir.path().join("trunc.wal");
        let wal = WriteAheadLog::open(&wal_path, WalConfig::default()).unwrap();

        wal.append(&make_entry(1)).unwrap();
        wal.append(&make_entry(2)).unwrap();
        assert!(wal.offset() > 0);

        wal.truncate().unwrap();
        assert_eq!(wal.offset(), 0);

        let recovered = wal.recover().unwrap();
        assert!(recovered.is_empty());
    }

    #[test]
    fn checkpoint_with_delete_retention() {
        let dir = tempfile::tempdir().unwrap();
        let wal_path = dir.path().join("ckpt.wal");
        let config = WalConfig {
            retention: WalRetention::DeleteOnCheckpoint,
            ..WalConfig::default()
        };
        let wal = WriteAheadLog::open(&wal_path, config).unwrap();

        let _off1 = wal.append(&make_entry(1)).unwrap();
        let off2 = wal.append(&make_entry(2)).unwrap();
        wal.append(&make_entry(3)).unwrap();

        // Checkpoint through the start of entry 2, removing entry 1.
        wal.checkpoint(off2).unwrap();

        let recovered = wal.recover().unwrap();
        // Entry 1 was checkpointed away; entries 2 and 3 remain.
        assert_eq!(recovered.len(), 2);
        assert_eq!(recovered[0], make_entry(2));
        assert_eq!(recovered[1], make_entry(3));
    }

    #[test]
    fn append_returns_increasing_offsets() {
        let dir = tempfile::tempdir().unwrap();
        let wal_path = dir.path().join("offsets.wal");
        let wal = WriteAheadLog::open(&wal_path, WalConfig::default()).unwrap();

        let off1 = wal.append(&make_entry(1)).unwrap();
        let off2 = wal.append(&make_entry(2)).unwrap();
        let off3 = wal.append(&make_entry(3)).unwrap();

        assert_eq!(off1, 0);
        assert!(off2 > off1);
        assert!(off3 > off2);
    }

    #[test]
    fn recovery_survives_truncated_tail() {
        let dir = tempfile::tempdir().unwrap();
        let wal_path = dir.path().join("tail.wal");
        let wal = WriteAheadLog::open(&wal_path, WalConfig::default()).unwrap();

        wal.append(&make_entry(1)).unwrap();
        wal.append(&make_entry(2)).unwrap();
        let total_len = wal.offset();
        drop(wal);

        // Truncate the file mid-entry (remove last 4 bytes).
        {
            let file = OpenOptions::new().write(true).open(&wal_path).unwrap();
            file.set_len(total_len - 4).unwrap();
        }

        let wal = WriteAheadLog::open(&wal_path, WalConfig::default()).unwrap();
        let recovered = wal.recover().unwrap();

        // Only the first complete entry should be recovered.
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0], make_entry(1));
    }

    #[test]
    fn sync_every_write_mode() {
        let dir = tempfile::tempdir().unwrap();
        let wal_path = dir.path().join("sync.wal");
        let config = WalConfig {
            sync_mode: SyncMode::EveryWrite,
            ..WalConfig::default()
        };
        let wal = WriteAheadLog::open(&wal_path, config).unwrap();

        // Should not panic; data should be durable.
        wal.append(&make_entry(1)).unwrap();
        let recovered = wal.recover().unwrap();
        assert_eq!(recovered.len(), 1);
    }
}
