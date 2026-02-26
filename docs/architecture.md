# WorldLine Ledger Architecture

This document describes the internal architecture of WLL, the relationships between its 17 crates, and the design principles behind each layer.

## Design Principles

1. **Provenance-first** — Every mutation is recorded as a commitment/outcome receipt pair. The receipt chain is append-only and cryptographically linked.
2. **Zero-trust verification** — Any repository state can be independently verified by replaying the receipt chain from genesis.
3. **Layered composition** — Each crate has a single responsibility and depends only on crates in the same or lower layers.
4. **Trait-driven** — Core behaviors (ObjectStore, LedgerWriter, RefStore, etc.) are defined as traits with in-memory and on-disk implementations.
5. **Domain-separated cryptography** — All hashes use BLAKE3 with domain prefixes (BLOB, TREE, RECEIPT, COMMIT) to prevent cross-type collisions.

## Layer Architecture

```
Layer 6: Application     wll-cli, wll-sdk
Layer 5: Distribution    wll-pack, wll-sync, wll-protocol, wll-server
Layer 4: Workflow         wll-refs, wll-index, wll-diff, wll-merge
Layer 3: Policy           wll-gate
Layer 2: Core             wll-dag, wll-ledger, wll-fabric
Layer 1: Foundation       wll-types, wll-crypto, wll-store
```

### Layer 1: Foundation

**wll-types** defines the core type vocabulary shared by all crates:

- `ObjectId` — 32-byte BLAKE3 hash of content, used as the universal content address
- `WorldlineId` — Cryptographic identity derived from Ed25519 key material via `IdentityMaterial`
- `CommitmentId` — UUID v7 identifying a specific commitment proposal
- `CommitmentClass` — Enumeration: ContentUpdate, PolicyChange, SecurityPatch, StructuralReorganization, EvidenceAttachment, ConfigurationChange, AccessControl
- `TemporalAnchor` — Hybrid Logical Clock timestamp: `(physical_ms, logical, node_id)`
- `EvidenceBundle` — Set of URI references with a digest for tamper detection
- `CommitmentProposal` — The input to the commitment boundary

**wll-crypto** provides all cryptographic operations:

- `hash_with_domain(domain, data) -> [u8; 32]` — BLAKE3 with domain separation
- `Signer` / `Verifier` traits backed by Ed25519
- Domain constants: `DOMAIN_BLOB`, `DOMAIN_TREE`, `DOMAIN_RECEIPT`, `DOMAIN_COMMIT`

**wll-store** implements the content-addressable object store:

- `ObjectStore` trait: `write(&StoredObject) -> ObjectId`, `read(&ObjectId) -> Option<StoredObject>`
- `Blob` — Raw byte content
- `Tree` — Directory listing with `TreeEntry` items (name, mode, ObjectId)
- `StoredObject` — Envelope with `ObjectKind` tag + serialized data
- `InMemoryObjectStore` — Thread-safe in-memory implementation using `DashMap`

### Layer 2: Core

**wll-dag** builds the provenance directed acyclic graph:

- `ProvenanceDag` — Nodes represent receipts; edges represent causal relationships
- `add_node(hash, parents)` — Insert a receipt with its causal predecessors
- `ancestors(hash)` — BFS traversal to find all ancestors
- `descendants(hash)` — Reverse traversal for impact analysis
- `common_ancestor(a, b)` — Find the merge base for two branches

**wll-ledger** manages the append-only receipt chain:

- `LedgerWriter` trait:
  - `append_commitment(proposal, decision, policy_hash) -> CommitmentReceipt`
  - `append_outcome(commitment_hash, outcome_record) -> OutcomeReceipt`
- `LedgerReader` trait:
  - `head(worldline) -> Option<Receipt>` — Latest receipt
  - `get_by_hash(hash) -> Option<Receipt>` — Lookup by receipt hash
  - `read_all(worldline) -> Vec<Receipt>` — Full chain
  - `receipt_count(worldline) -> u64`
- `StreamValidator` — Validates hash chain continuity, sequence monotonicity, and receipt pairing
- `ReplayEngine` — Deterministic replay from genesis, applying each outcome in order
- `ProjectionBuilder` — Computes the latest materialized state from the receipt chain
- `InMemoryLedger` — Thread-safe implementation with Write-Ahead Log semantics

**wll-fabric** implements the temporal ordering layer:

- `HybridLogicalClock` — Implements the HLC algorithm for distributed causality
- `TemporalFabric` — Weaves timestamps into the receipt chain
- Guarantees: if event A causally precedes event B, then `timestamp(A) < timestamp(B)`

### Layer 3: Policy

**wll-gate** is the commitment boundary — the single point through which all mutations must pass:

- `Gate` struct with a configurable `PolicyPipeline`
- `PolicyRule` trait: `evaluate(proposal) -> PolicyDecision`
- Built-in rules: `RequireIntent`, `RequireEvidence`, `MaxSizeLimit`, `AllowedClasses`
- `CapabilityToken` — Capability-based access control with optional expiry
- Flow: `Proposal → PolicyPipeline → Decision (Accept/Reject) → CommitmentReceipt`
- Rejected proposals are still recorded for auditability

### Layer 4: Workflow

**wll-refs** manages named references:

- `RefStore` trait: `read_ref`, `write_ref`, `delete_ref`, `list_refs(prefix)`
- `Head` enum: `Symbolic(branch_name)` | `Detached(hash)` — tracks current position
- `Ref` enum: `Branch { name, worldline, receipt_hash }`, `Tag { name, target, tagger, message, timestamp, signature }`, `Remote { remote, branch, worldline, receipt_hash }`
- Branch name validation, tag immutability enforcement, detached HEAD support
- Canonical names: `refs/heads/main`, `refs/tags/v1.0`, `refs/remotes/origin/main`

**wll-index** implements the staging area:

- `StagingArea` — Tracks files staged for the next commitment
- `WorkingTreeScanner` — Detects changes between HEAD tree, index, and working directory
- `IndexEntry` — File metadata: path, mode, ObjectId, timestamps, size

**wll-diff** computes differences:

- `TreeDiffer` — Tree-to-tree comparison producing `TreeDelta` (Added, Removed, Modified, Renamed)
- `BlobDiffer` — Line-level diff using the Myers algorithm via the `similar` crate
- `DiffHunk` — Unified diff format with context lines

**wll-merge** handles branch merging:

- `ThreeWayMerge` — Given base, ours, theirs → merged result or conflicts
- `MergeConflict` — Records conflicting hunks with both sides preserved
- `MergeStrategy` — Configurable: Recursive, Ours, Theirs
- Receipt-aware: merge commits reference both parent chains in the provenance DAG

### Layer 5: Distribution

**wll-pack** implements the packfile format:

- `PackWriter` — Collects objects, compresses with zstd, writes magic + version + entries + checksum
- `PackReader` — Reads and decompresses objects from packfiles
- `PackIndex` — 256-entry fan-out table for O(log n) lookups by ObjectId
- `PackManager` — Manages multiple packfiles, garbage collection, repacking
- Format: `[WLLP magic][version u32][object_count varint][entries...][BLAKE3 checksum]`
- Each entry: `[type_byte][id: 32 bytes][size varint][zstd-compressed data]`

**wll-sync** handles remote synchronization:

- `RemoteTransport` trait (async): `list_refs`, `fetch_objects`, `fetch_receipts`, `push_pack`, `push_receipts`, `update_refs`
- `NegotiationEngine` — Computes wants/haves for efficient delta transfer
- `SyncVerifier` — Validates received receipts before integration (worldline match, sequence monotonicity, hash chain)
- `RefSpec` — Push/fetch refspec parsing with force flag support

**wll-protocol** defines the wire format:

- `WllMessage` enum — All protocol message types (ListRefs, ListRefsResponse, FetchRequest, PackData, PushRequest, PushResult, ReceiptBatch, etc.)
- `WllCodec` — Frame encoding: `[4-byte length][1-byte tag][bincode payload]`
- `AuthMethod` — Bearer, SshKey, MutualTls, Anonymous
- Endpoint constants for HTTP routing

**wll-server** provides the HTTP/2 server:

- `WllServer` — Axum-based HTTP server with configurable auth and hooks
- `AuthProvider` trait — Pluggable authentication (bearer token, SSH key, mTLS)
- `ServerHook` trait — Pre-receive and post-receive hooks for policy enforcement
- `ServerConfig` — TOML-configurable: bind address, TLS, connection limits, max pack size
- Endpoints: `/v1/health`, `/v1/info`, `/v1/fetch`, `/v1/push`, `/v1/receipt/query`

### Layer 6: Application

**wll-cli** is the command-line interface:

- Built with `clap` derive macros for type-safe argument parsing
- 25 commands matching Git's familiar UX + WLL-specific provenance commands
- Colored output with `colored` crate
- Output format options: text (default) and JSON
- Tracing integration for debug logging

**wll-sdk** is the high-level Rust SDK:

- `Wll` facade — Single entry point wrapping store + ledger + refs + DAG
- `CommitProposal` builder pattern — Fluent API for constructing commits
- `CommitResult` — Contains both commitment and outcome receipts
- Direct access to lower-level crates via accessor methods

## Data Flow

### Commit Flow

```
User intent
    │
    ▼
CommitProposal { message, intent, class, evidence, tree }
    │
    ▼
Gate::evaluate(proposal)
    ├── PolicyPipeline.run(proposal)
    │       ├── RequireIntent → pass/fail
    │       ├── RequireEvidence → pass/fail
    │       └── CustomRules → pass/fail
    │
    ├── Decision::Accepted ──────────────────────────┐
    │                                                │
    ▼                                                ▼
LedgerWriter::append_commitment(proposal, decision, policy_hash)
    │                                                │
    │   Creates CommitmentReceipt {                  │
    │     seq, receipt_hash, prev_hash,              │
    │     worldline, intent, decision,               │
    │     timestamp (HLC), evidence_digest            │
    │   }                                            │
    │                                                │
    ▼                                                │
LedgerWriter::append_outcome(commitment_hash, outcome_record)
    │                                                │
    │   Creates OutcomeReceipt {                     │
    │     seq, receipt_hash, prev_hash,              │
    │     commitment_receipt_hash,                    │
    │     effects, state_updates                     │
    │   }                                            │
    │                                                │
    ▼                                                │
RefStore::write_ref(branch, updated_tip)  ◄──────────┘
    │
    ▼
ProvenanceDag::add_node(outcome_hash, [commitment_hash, prev_tip])
```

### Verification Flow

```
StreamValidator::validate_stream(ledger, worldline)
    │
    ├── Check 1: prev_hash chain → each receipt.prev_hash == previous.receipt_hash
    ├── Check 2: Sequence monotonicity → seq values strictly increasing
    ├── Check 3: Commitment/Outcome pairing → outcomes reference valid commitments
    ├── Check 4: Snapshot anchoring → snapshot hashes match computed state
    └── Result: ValidationReport { is_valid, violations: Vec<Violation> }
```

### Replay Flow

```
ReplayEngine::replay_from_genesis(ledger, worldline)
    │
    ├── Read all receipts in sequence order
    ├── For each OutcomeReceipt:
    │     ├── Apply state_updates to accumulated state
    │     ├── Verify effects are consistent
    │     └── Track applied_outcomes count
    │
    └── Result: ReplayResult { applied_outcomes, final_state, trajectory_length }
```

## Cryptographic Design

### Hash Domains

Every hash in WLL is computed with a domain prefix to prevent cross-type collisions:

```
blob_hash    = BLAKE3("BLOB:"    || data)
tree_hash    = BLAKE3("TREE:"    || serialized_entries)
receipt_hash = BLAKE3("RECEIPT:" || serialized_receipt)
commit_hash  = BLAKE3("COMMIT:" || serialized_proposal)
```

### Receipt Hash Chain

```
Receipt #1:  hash=H1, prev_hash=0x00...00, seq=1
Receipt #2:  hash=H2, prev_hash=H1,        seq=2
Receipt #3:  hash=H3, prev_hash=H2,        seq=3
    ...
Receipt #N:  hash=HN, prev_hash=H(N-1),    seq=N
```

If any receipt is modified, its hash changes, breaking the chain for all subsequent receipts. This makes the receipt chain tamper-evident.

### WorldLine Identity

```
IdentityMaterial::GenesisHash(seed: [u8; 32])
    │
    ▼
Ed25519 key derivation
    │
    ▼
WorldlineId = BLAKE3(public_key_bytes)
```

The WorldlineId is the root of trust. All receipts in a repository reference their WorldlineId, binding them to a specific identity.

## Thread Safety

All in-memory implementations use thread-safe primitives:

- `InMemoryObjectStore` — `DashMap<ObjectId, StoredObject>`
- `InMemoryLedger` — `RwLock<Vec<Receipt>>` with WAL semantics
- `InMemoryRefStore` — `RwLock<HashMap<String, Ref>>`

This allows concurrent reads with serialized writes, suitable for multi-threaded server deployment.

## Packfile Format

```
┌──────────────────────────────────────────────────────┐
│ Magic: "WLLP" (4 bytes)                              │
│ Version: u32 (4 bytes)                               │
│ Object Count: varint                                 │
├──────────────────────────────────────────────────────┤
│ Entry 1:                                             │
│   Type Byte (1 byte): 0x01=Full, 0x02=Delta          │
│   Object ID (32 bytes): BLAKE3 hash                  │
│   Compressed Size (varint)                           │
│   Data (zstd-compressed bytes)                       │
├──────────────────────────────────────────────────────┤
│ Entry 2: ...                                         │
├──────────────────────────────────────────────────────┤
│ ...                                                  │
├──────────────────────────────────────────────────────┤
│ Checksum: BLAKE3(all preceding bytes) (32 bytes)     │
└──────────────────────────────────────────────────────┘
```

### Pack Index Format

```
┌──────────────────────────────────────────────────────┐
│ Fan-out Table: [u32; 256]                            │
│   fanout[i] = count of objects with first byte <= i  │
├──────────────────────────────────────────────────────┤
│ Sorted Object IDs: [[u8; 32]; N]                     │
│   Sorted by ObjectId for binary search               │
├──────────────────────────────────────────────────────┤
│ Offsets: [u64; N]                                    │
│   Byte offset of each object in the packfile         │
└──────────────────────────────────────────────────────┘
```

## Wire Protocol

Messages are framed as:

```
┌────────────┬──────────┬──────────────────────┐
│ Length (4B) │ Tag (1B) │ Payload (bincode)    │
└────────────┴──────────┴──────────────────────┘
```

Protocol flow for a push operation:

```
Client                          Server
  │                               │
  ├── ListRefs ──────────────────►│
  │◄──────────── ListRefsResponse─┤
  │                               │
  ├── PushRequest ───────────────►│  (pack data + receipts)
  │   + PackData                  │
  │   + ReceiptBatch              │
  │                               │── Verify receipts
  │                               │── Run server hooks
  │                               │── Store objects
  │◄──────────── PushResult ──────┤
  │              (accepted/errors)│
```

## Future Directions

- **Filesystem backend** — On-disk ObjectStore with loose objects + packfiles
- **Network transport** — HTTP/2 and gRPC RemoteTransport implementations
- **Snapshot compaction** — Periodic state snapshots to bound replay time
- **Partial clone** — Fetch only the receipt chain metadata without full object transfer
- **Multi-WorldLine** — Cross-repository provenance linking
- **WASM bindings** — Browser and edge deployment via wasm-bindgen
