# WorldLine Ledger SDK Programming Guide

The `wll-sdk` crate provides a high-level facade for the WorldLine Ledger (WLL), a
content-addressable, append-only ledger system with built-in provenance tracking. This
guide covers everything needed to integrate WLL into Rust applications, from initial
setup through advanced usage patterns.

---

## Table of Contents

1. [Installation & Setup](#1-installation--setup)
2. [Quick Start](#2-quick-start)
3. [Repository Management](#3-repository-management)
4. [Content Operations](#4-content-operations)
5. [Commitment Operations](#5-commitment-operations)
6. [Branch Management](#6-branch-management)
7. [Provenance Queries](#7-provenance-queries)
8. [Error Handling](#8-error-handling)
9. [Advanced Usage](#9-advanced-usage)
10. [Best Practices](#10-best-practices)

---

## 1. Installation & Setup

### Cargo Dependency

Add `wll-sdk` to your project's `Cargo.toml`:

```toml
[dependencies]
wll-sdk = { git = "https://github.com/mapleaiorg/wll", version = "0.1" }
```

For workspace-based projects that vendor WLL as a submodule or path dependency:

```toml
[dependencies]
wll-sdk = { path = "../wll/crates/wll-sdk" }
```

### Minimum Supported Rust Version

WLL requires **Rust 1.80** or later. Verify your toolchain:

```bash
rustc --version   # must be >= 1.80.0
```

### Crate Prelude

The SDK re-exports the types you need most often. A typical import block looks like
this:

```rust
use wll_sdk::{
    // Main entry point
    Wll,
    // Commit workflow
    CommitProposal, CommitResult, ReceiptSummary,
    // Content-addressable objects
    ObjectId, Tree, TreeEntry, EntryMode, Blob,
    // Commitment classification
    CommitmentClass,
    // Provenance
    Receipt, ValidationReport,
    // Identity
    WorldlineId, CommitmentId,
    // Error handling
    SdkError, SdkResult,
};
```

---

## 2. Quick Start

The following self-contained example initializes a repository, stores content, creates
a commit, and verifies the resulting chain:

```rust
use wll_sdk::{Wll, CommitProposal, EntryMode, TreeEntry};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize a new repository (random worldline identity).
    let wll = Wll::init()?;
    println!("Worldline: {}", wll.worldline().short_id());

    // 2. Store some content.
    let blob_id = wll.write_blob(b"Hello, WorldLine Ledger!")?;
    let tree_id = wll.write_tree(vec![
        TreeEntry::new(EntryMode::Regular, "greeting.txt", blob_id),
    ])?;

    // 3. Create a commit with the tree attached.
    let result = wll.commit(
        CommitProposal::new("Initial content")
            .with_tree(tree_id),
    )?;
    println!(
        "Committed: seq={}, hash={}",
        result.commitment_receipt.seq,
        hex::encode(result.receipt_hash),
    );

    // 4. Verify chain integrity.
    let report = wll.verify()?;
    assert!(report.is_valid(), "Chain integrity check failed");
    println!(
        "Verified {} receipts -- chain is valid",
        report.receipt_count,
    );

    Ok(())
}
```

---

## 3. Repository Management

### Initialization with a Random Worldline

`Wll::init()` derives a fresh worldline identity from a time-based seed. Every call
produces a distinct worldline. This is the simplest way to get started:

```rust
use wll_sdk::Wll;

fn main() -> wll_sdk::SdkResult<()> {
    let wll = Wll::init()?;

    // The worldline is a 32-byte BLAKE3-derived identifier.
    println!("Worldline (hex): {}", wll.worldline().to_hex());
    println!("Worldline (short): {}", wll.worldline().short_id());

    // A fresh repository starts on the "main" branch with zero receipts.
    assert_eq!(wll.current_branch()?, "main");
    assert_eq!(wll.receipt_count()?, 0);

    Ok(())
}
```

### Initialization with a Deterministic Worldline

When you need reproducible identities -- for example in tests or when reconnecting to a
known ledger stream -- supply a specific `WorldlineId`:

```rust
use wll_sdk::{Wll, WorldlineId};
use wll_types::IdentityMaterial;

fn main() -> wll_sdk::SdkResult<()> {
    // Derive a deterministic worldline from a known seed.
    let seed: [u8; 32] = *b"my-application-seed-value-32byte";
    let worldline = WorldlineId::derive(&IdentityMaterial::GenesisHash(seed));

    let wll = Wll::init_with_worldline(worldline.clone())?;
    assert_eq!(wll.worldline(), &worldline);

    Ok(())
}
```

### Worldline Identity

The `WorldlineId` is the root identity anchor for a ledger stream. It is a 32-byte
BLAKE3 hash derived from `IdentityMaterial`. Key operations:

| Method | Description |
|---|---|
| `WorldlineId::derive(&material)` | Derive from identity material |
| `.to_hex()` | Full 64-character hex string |
| `.short_id()` | Abbreviated identifier for display |

A worldline is immutable once created. All commits, branches, and receipts are scoped
to the worldline that owns the repository.

---

## 4. Content Operations

WLL uses a content-addressable object store. Every piece of data is identified by its
BLAKE3 hash (`ObjectId`). There are two fundamental object types: **blobs** (raw byte
sequences) and **trees** (ordered collections of named entries).

### Writing and Reading Blobs

A blob is an opaque byte array. The store deduplicates automatically; writing the same
bytes twice returns the same `ObjectId`:

```rust
use wll_sdk::Wll;

fn main() -> wll_sdk::SdkResult<()> {
    let wll = Wll::init()?;

    // Write a blob and get its content-addressed ID.
    let id = wll.write_blob(b"sensor reading: 42.7")?;
    println!("Blob ID: {}", id.to_hex());

    // Read the blob back.
    let data = wll.read_blob(&id)?;
    assert_eq!(data, b"sensor reading: 42.7");

    // Writing the same content yields the same ID (deduplication).
    let id2 = wll.write_blob(b"sensor reading: 42.7")?;
    assert_eq!(id, id2);

    Ok(())
}
```

### ObjectId Utilities

`ObjectId` provides several convenience methods:

```rust
use wll_sdk::ObjectId;

fn demonstrate_object_id() {
    // Create from raw data (BLAKE3 hash of the input).
    let id = ObjectId::from_bytes(b"example payload");

    // Hex representations.
    let full_hex: String = id.to_hex();       // 64 hex characters
    let short_hex: String = id.short_hex();   // abbreviated for display

    // Access the underlying 32-byte array.
    let raw: &[u8; 32] = id.as_bytes();

    // Construct from an existing 32-byte hash.
    let restored = ObjectId::from_hash(*raw);
    assert_eq!(id, restored);

    // The null ObjectId (all zeros) represents "no object".
    let null_id = ObjectId::null();
    assert_eq!(null_id.to_hex(), "0".repeat(64));
}
```

### Writing and Reading Trees

A tree is an ordered set of `TreeEntry` values. Each entry pairs a name with an
`ObjectId` and a file mode. Trees can reference blobs (files) or other trees
(subdirectories):

```rust
use wll_sdk::{Wll, TreeEntry, EntryMode};

fn main() -> wll_sdk::SdkResult<()> {
    let wll = Wll::init()?;

    // Create some blobs for file content.
    let readme_id = wll.write_blob(b"# My Project\n")?;
    let main_id = wll.write_blob(b"fn main() {}\n")?;
    let script_id = wll.write_blob(b"#!/bin/sh\necho 'build'\n")?;

    // Build a nested tree structure:
    //   /
    //   +-- README.md      (Regular)
    //   +-- src/
    //   |   +-- main.rs    (Regular)
    //   +-- scripts/
    //       +-- build.sh   (Executable)

    // First, create the subtrees.
    let src_tree_id = wll.write_tree(vec![
        TreeEntry::new(EntryMode::Regular, "main.rs", main_id),
    ])?;

    let scripts_tree_id = wll.write_tree(vec![
        TreeEntry::new(EntryMode::Executable, "build.sh", script_id),
    ])?;

    // Then, create the root tree referencing the subtrees.
    let root_tree_id = wll.write_tree(vec![
        TreeEntry::new(EntryMode::Regular, "README.md", readme_id),
        TreeEntry::new(EntryMode::Directory, "scripts", scripts_tree_id),
        TreeEntry::new(EntryMode::Directory, "src", src_tree_id),
    ])?;

    // Read the root tree back and inspect it.
    let root_tree = wll.read_tree(&root_tree_id)?;
    assert_eq!(root_tree.len(), 3);
    for entry in &root_tree.entries {
        println!("  {:?} {} -> {}", entry.mode, entry.name, entry.object_id.short_hex());
    }

    Ok(())
}
```

### Entry Modes

| Variant | Octal | Meaning |
|---|---|---|
| `EntryMode::Regular` | `0o100644` | Normal file |
| `EntryMode::Executable` | `0o100755` | Executable file |
| `EntryMode::Symlink` | `0o120000` | Symbolic link |
| `EntryMode::Directory` | `0o040000` | Subtree / directory |

---

## 5. Commitment Operations

Commitments are the ledger's unit of change. Each commit produces a **commitment
receipt** (recording the intent and decision) followed by an **outcome receipt**
(recording the effects). Together, these form an immutable, ordered audit trail.

### Building a CommitProposal

`CommitProposal` uses a builder pattern. Only the message is required; all other fields
have sensible defaults:

```rust
use wll_sdk::{CommitProposal, CommitmentClass};

// Minimal proposal -- defaults to ContentUpdate class, message as intent.
let simple = CommitProposal::new("Fix typo in README");

// Fully specified proposal.
let detailed = CommitProposal::new("Rotate API credentials")
    .with_intent("security: credential rotation")
    .with_class(CommitmentClass::IdentityOperation)
    .with_evidence("https://issues.example.com/SEC-1042")
    .with_evidence("https://runbook.example.com/credential-rotation");
```

#### CommitProposal Fields

| Builder method | Default | Description |
|---|---|---|
| `new(message)` | *(required)* | Human-readable commit message |
| `.with_intent(intent)` | Falls back to `message` | Machine-readable intent label |
| `.with_class(class)` | `CommitmentClass::ContentUpdate` | Commitment classification |
| `.with_evidence(uri)` | Empty list | URI references to supporting evidence (additive) |
| `.with_tree(object_id)` | `None` | Root tree `ObjectId` for this commit |

The `effective_intent()` method returns the explicit intent if set, otherwise the
message. Similarly, `effective_class()` returns the explicit class or
`ContentUpdate`.

### Commitment Classes

Every commitment carries a classification that signals its risk level and nature:

| Class | Use case |
|---|---|
| `CommitmentClass::ReadOnly` | Observation / query (lowest risk) |
| `CommitmentClass::ContentUpdate` | Normal file or data changes |
| `CommitmentClass::StructuralChange` | Directory reorganization, schema migration |
| `CommitmentClass::PolicyChange` | Governance rule modifications |
| `CommitmentClass::IdentityOperation` | Key rotation, delegation changes |
| `CommitmentClass::Custom(String)` | Domain-specific classification |

### Executing a Commit

Pass a `CommitProposal` to `Wll::commit()`. The SDK handles proposal submission,
decision recording, outcome generation, and branch-tip advancement:

```rust
use wll_sdk::{Wll, CommitProposal, CommitmentClass, TreeEntry, EntryMode};

fn main() -> wll_sdk::SdkResult<()> {
    let wll = Wll::init()?;

    // Store content and build a tree.
    let data_id = wll.write_blob(b"{\"version\": 2}")?;
    let tree_id = wll.write_tree(vec![
        TreeEntry::new(EntryMode::Regular, "config.json", data_id),
    ])?;

    // Commit with full metadata.
    let result = wll.commit(
        CommitProposal::new("Upgrade config schema to v2")
            .with_intent("schema-migration-v2")
            .with_class(CommitmentClass::StructuralChange)
            .with_evidence("https://jira.example.com/PLAT-500")
            .with_tree(tree_id),
    )?;

    // Inspect the result.
    println!("Commitment seq:  {}", result.commitment_receipt.seq);
    println!("Outcome seq:     {}", result.outcome_receipt.seq);
    println!("Receipt hash:    {}", hex::encode(result.receipt_hash));

    // Each commit produces exactly 2 receipts (commitment + outcome).
    assert_eq!(wll.receipt_count()?, 2);

    Ok(())
}
```

### CommitResult Structure

The `CommitResult` returned by `commit()` contains:

| Field | Type | Description |
|---|---|---|
| `commitment_receipt` | `CommitmentReceipt` | The ledger record of the commitment decision |
| `outcome_receipt` | `OutcomeReceipt` | The ledger record of the outcome/effects |
| `receipt_hash` | `[u8; 32]` | BLAKE3 hash of the outcome receipt (the new branch tip) |

### Querying the Log

The `log()` method returns receipt summaries in reverse chronological order:

```rust
use wll_sdk::{Wll, CommitProposal};

fn main() -> wll_sdk::SdkResult<()> {
    let wll = Wll::init()?;

    wll.commit(CommitProposal::new("First change"))?;
    wll.commit(CommitProposal::new("Second change"))?;

    // Retrieve the 10 most recent receipt summaries.
    let entries = wll.log(10)?;

    for entry in &entries {
        println!(
            "seq={:<4} kind={:<12} intent={:<20} accepted={:?}  ts={}",
            entry.seq,
            entry.kind,
            entry.intent.as_deref().unwrap_or("-"),
            entry.accepted,
            entry.timestamp_ms,
        );
    }

    // Two commits produce 4 receipts (2 each).
    assert_eq!(entries.len(), 4);
    // Reverse chronological: highest sequence number first.
    assert!(entries[0].seq > entries[1].seq);

    Ok(())
}
```

### Inspecting Individual Receipts

Use `show()` to retrieve the full `Receipt` for a given hash:

```rust
use wll_sdk::{Wll, CommitProposal};

fn main() -> wll_sdk::SdkResult<()> {
    let wll = Wll::init()?;
    let result = wll.commit(CommitProposal::new("Auditable change"))?;

    // Look up the commitment receipt by its hash.
    let receipt = wll.show(&result.commitment_receipt.receipt_hash)?;

    // Receipts are typed. Commitment receipts expose the intent and decision.
    if let Some(commitment) = receipt.as_commitment() {
        println!("Intent:   {}", commitment.intent);
        println!("Accepted: {}", commitment.decision.is_accepted());
    }

    Ok(())
}
```

---

## 6. Branch Management

WLL supports lightweight branches similar to Git. Every repository starts with a single
`main` branch. Branches track the latest receipt hash (the "tip") and advance
automatically on each commit.

### Creating and Switching Branches

```rust
use wll_sdk::{Wll, CommitProposal};

fn main() -> wll_sdk::SdkResult<()> {
    let wll = Wll::init()?;

    // Commit on main.
    wll.commit(CommitProposal::new("Initial setup"))?;

    // Create a feature branch (forked from the current tip).
    wll.create_branch("feature/audit-export")?;
    wll.switch_branch("feature/audit-export")?;
    assert_eq!(wll.current_branch()?, "feature/audit-export");

    // Commits now advance the feature branch tip.
    wll.commit(CommitProposal::new("Add CSV export for audit logs"))?;

    // Switch back to main.
    wll.switch_branch("main")?;
    assert_eq!(wll.current_branch()?, "main");

    Ok(())
}
```

### Listing Branches

```rust
use wll_sdk::Wll;

fn main() -> wll_sdk::SdkResult<()> {
    let wll = Wll::init()?;
    wll.create_branch("develop")?;
    wll.create_branch("release/1.0")?;

    let branches = wll.list_branches()?;
    for name in &branches {
        let marker = if *name == wll.current_branch()? { " *" } else { "" };
        println!("  {}{}", name, marker);
    }

    // Output:
    //   develop
    //   main *
    //   release/1.0

    Ok(())
}
```

### Branch Errors

Attempting to switch to a nonexistent branch returns `SdkError::BranchNotFound`:

```rust
use wll_sdk::{Wll, SdkError};

fn main() -> wll_sdk::SdkResult<()> {
    let wll = Wll::init()?;

    match wll.switch_branch("nonexistent") {
        Err(SdkError::BranchNotFound(name)) => {
            println!("No such branch: {}", name);
        }
        other => panic!("Unexpected result: {:?}", other),
    }

    Ok(())
}
```

---

## 7. Provenance Queries

WLL's core value proposition is verifiable provenance. Three query methods let you
validate, replay, and summarize a ledger stream.

### Chain Verification

`verify()` runs the stream validator, which checks hash-chain continuity, sequence
monotonicity, and outcome attribution across all receipts:

```rust
use wll_sdk::{Wll, CommitProposal};

fn main() -> wll_sdk::SdkResult<()> {
    let wll = Wll::init()?;
    wll.commit(CommitProposal::new("genesis"))?;
    wll.commit(CommitProposal::new("second entry"))?;

    let report = wll.verify()?;

    println!("Receipt count:       {}", report.receipt_count);
    println!("Hash chain valid:    {}", report.hash_chain_valid);
    println!("Sequence monotonic:  {}", report.sequence_monotonic);
    println!("Outcomes attributed: {}", report.outcomes_attributed);
    println!("Overall valid:       {}", report.is_valid());

    if !report.is_valid() {
        for v in &report.violations {
            eprintln!(
                "  VIOLATION at seq {}: {:?} -- {}",
                v.seq, v.kind, v.description,
            );
        }
    }

    Ok(())
}
```

#### ValidationReport Fields

| Field | Type | Description |
|---|---|---|
| `worldline` | `WorldlineId` | The worldline that was validated |
| `receipt_count` | `u64` | Total receipts examined |
| `hash_chain_valid` | `bool` | Whether every receipt's prev-hash matches |
| `sequence_monotonic` | `bool` | Whether sequence numbers strictly increase |
| `outcomes_attributed` | `bool` | Whether every outcome links to a commitment |
| `violations` | `Vec<Violation>` | Detailed list of integrity failures |

#### Violation Kinds

| Kind | Meaning |
|---|---|
| `SequenceGap` | Non-contiguous sequence numbers |
| `HashChainBreak` | A receipt's prev-hash does not match its predecessor |
| `HashMismatch` | Stored hash does not match recomputed hash |
| `UnattributedOutcome` | Outcome receipt without a matching commitment |
| `UnanchoredSnapshot` | Snapshot receipt without a valid anchor point |

### Deterministic Replay

`replay()` re-executes the ledger from genesis, applying every outcome in order and
reconstructing the final state map. This is useful for auditing, state recovery, and
compliance verification:

```rust
use wll_sdk::{Wll, CommitProposal};

fn main() -> wll_sdk::SdkResult<()> {
    let wll = Wll::init()?;
    wll.commit(CommitProposal::new("set initial config"))?;
    wll.commit(CommitProposal::new("update threshold"))?;

    let replay = wll.replay()?;

    println!("Applied outcomes:    {}", replay.applied_outcomes);
    println!("Evaluated receipts:  {}", replay.evaluated_receipts);
    println!("Reconstructed state:");
    for (key, value) in &replay.state {
        println!("  {} = {}", key, value);
    }

    Ok(())
}
```

### Latest State Projection

`latest_state()` returns a snapshot of the ledger's current position without replaying
the full history. This is the most efficient way to query the head state for
operational use:

```rust
use wll_sdk::{Wll, CommitProposal};

fn main() -> wll_sdk::SdkResult<()> {
    let wll = Wll::init()?;
    wll.commit(CommitProposal::new("initialize system"))?;

    let projection = wll.latest_state()?;

    println!("Worldline:         {}", projection.worldline.short_id());
    println!("Trajectory length: {}", projection.trajectory_length);

    if let Some(ref ts) = projection.last_updated {
        println!("Last updated:      {} ms", ts.physical_ms);
    }
    if let Some(ref cid) = projection.latest_commitment {
        println!("Latest commitment: {:?}", cid);
    }

    println!("State entries:");
    for (key, value) in &projection.state {
        println!("  {} = {}", key, value);
    }

    Ok(())
}
```

#### LatestStateProjection Fields

| Field | Type | Description |
|---|---|---|
| `worldline` | `WorldlineId` | The owning worldline |
| `head` | `Option<ReceiptRef>` | Reference to the head receipt |
| `latest_commitment` | `Option<CommitmentId>` | ID of the most recent commitment |
| `trajectory_length` | `u64` | Total number of receipts in the stream |
| `last_updated` | `Option<TemporalAnchor>` | Timestamp of the most recent receipt |
| `state` | `BTreeMap<String, Value>` | Accumulated key-value state |

---

## 8. Error Handling

All fallible SDK operations return `SdkResult<T>`, which is an alias for
`Result<T, SdkError>`. The error enum covers both SDK-level conditions and propagated
errors from lower-level crates.

### SdkError Variants

```rust
use wll_sdk::SdkError;

// SDK-level errors (directly constructed by wll-sdk):
//
//   SdkError::NotInitialized(String)
//       The repository has not been initialized at the given path.
//
//   SdkError::BranchNotFound(String)
//       Attempted to switch to or query a branch that does not exist.
//
//   SdkError::ObjectNotFound(String)
//       A blob, tree, or receipt with the given ID/hash was not found.
//
//   SdkError::InvalidOperation(String)
//       The operation is not valid in the current state (e.g., detached HEAD).
//
//   SdkError::CommitmentRejected(String)
//       The ledger refused to accept the commitment proposal.
//
// Propagated errors (from lower-level crates, converted via `From`):
//
//   SdkError::Store(wll_store::StoreError)
//   SdkError::Ledger(wll_ledger::LedgerError)
//   SdkError::Ref(wll_refs::RefError)
//
// Catch-all:
//
//   SdkError::Internal(String)
//       Unexpected internal error.
```

### Pattern Matching on Errors

Use Rust's standard pattern matching to handle specific failure modes:

```rust
use wll_sdk::{Wll, SdkError, ObjectId};

fn handle_missing_object(wll: &Wll) {
    let fake_id = ObjectId::null();

    match wll.read_blob(&fake_id) {
        Ok(data) => println!("Got {} bytes", data.len()),
        Err(SdkError::ObjectNotFound(hex)) => {
            eprintln!("Object {} does not exist in the store", hex);
        }
        Err(SdkError::Store(store_err)) => {
            eprintln!("Underlying store error: {}", store_err);
        }
        Err(other) => {
            eprintln!("Unexpected error: {}", other);
        }
    }
}
```

### Propagation with the `?` Operator

Because `SdkError` implements `std::error::Error` and `Display`, it integrates
naturally with `?` and error-handling crates like `anyhow`:

```rust
use anyhow::Result;
use wll_sdk::{Wll, CommitProposal};

fn run_pipeline() -> Result<()> {
    let wll = Wll::init()?;

    let id = wll.write_blob(b"pipeline input")?;
    wll.commit(
        CommitProposal::new("Ingest pipeline data")
            .with_evidence("s3://bucket/input-2024.parquet"),
    )?;

    let report = wll.verify()?;
    anyhow::ensure!(report.is_valid(), "Chain integrity check failed");

    Ok(())
}
```

---

## 9. Advanced Usage

### Accessing Lower-Level Subsystems

The `Wll` facade exposes its underlying stores through accessor methods. This allows
direct interaction with the object store and ledger when the high-level API is
insufficient:

```rust
use wll_sdk::Wll;
use wll_store::ObjectStore;    // Trait from wll-store
use wll_ledger::LedgerReader;  // Trait from wll-ledger

fn main() -> wll_sdk::SdkResult<()> {
    let wll = Wll::init()?;

    // Access the raw object store.
    let store = wll.store();
    // Use ObjectStore trait methods directly, e.g.:
    // store.read(&some_id), store.contains(&some_id), etc.

    // Access the raw ledger.
    let ledger = wll.ledger();
    // Use LedgerReader trait methods directly, e.g.:
    // ledger.read_all(&worldline), ledger.head(&worldline), etc.

    Ok(())
}
```

### Multi-step Content Assembly

For complex content structures, build trees bottom-up and attach the root to a commit:

```rust
use wll_sdk::{Wll, CommitProposal, CommitmentClass, TreeEntry, EntryMode};

fn main() -> wll_sdk::SdkResult<()> {
    let wll = Wll::init()?;

    // Simulate a multi-file documentation update.
    let files: Vec<(&str, &[u8])> = vec![
        ("introduction.md", b"# Introduction\n\nWelcome to the system."),
        ("architecture.md", b"# Architecture\n\nLayered design overview."),
        ("api-reference.md", b"# API Reference\n\nEndpoint documentation."),
    ];

    let entries: Vec<TreeEntry> = files
        .into_iter()
        .map(|(name, content)| {
            let blob_id = wll.write_blob(content).expect("write_blob failed");
            TreeEntry::new(EntryMode::Regular, name, blob_id)
        })
        .collect();

    let docs_tree = wll.write_tree(entries)?;

    // Wrap the docs tree inside a root tree.
    let root = wll.write_tree(vec![
        TreeEntry::new(EntryMode::Directory, "docs", docs_tree),
    ])?;

    wll.commit(
        CommitProposal::new("Add project documentation")
            .with_class(CommitmentClass::ContentUpdate)
            .with_tree(root),
    )?;

    Ok(())
}
```

### Evidence-Rich Commits for Compliance

Attach multiple evidence URIs to create an auditable linkage between the commit and
external artifacts:

```rust
use wll_sdk::{Wll, CommitProposal, CommitmentClass};

fn main() -> wll_sdk::SdkResult<()> {
    let wll = Wll::init()?;

    let result = wll.commit(
        CommitProposal::new("Apply GDPR data retention policy")
            .with_intent("policy:gdpr-retention-v3")
            .with_class(CommitmentClass::PolicyChange)
            .with_evidence("https://legal.example.com/gdpr-policy-v3.pdf")
            .with_evidence("https://jira.example.com/COMPLY-2048")
            .with_evidence("https://slack.example.com/archives/C0LEGAL/p1700000000"),
    )?;

    println!("Policy commit recorded at seq {}", result.commitment_receipt.seq);

    Ok(())
}
```

### Custom Commitment Classes

For domain-specific workflows, use `CommitmentClass::Custom` with a descriptive label:

```rust
use wll_sdk::{Wll, CommitProposal, CommitmentClass};

fn main() -> wll_sdk::SdkResult<()> {
    let wll = Wll::init()?;

    wll.commit(
        CommitProposal::new("ML model checkpoint v3.2.1")
            .with_intent("ml:checkpoint")
            .with_class(CommitmentClass::Custom("ModelCheckpoint".into()))
            .with_evidence("s3://ml-artifacts/model-v3.2.1.safetensors"),
    )?;

    Ok(())
}
```

### Full Audit Workflow

Combine verification, replay, and log inspection for a complete audit:

```rust
use wll_sdk::{Wll, CommitProposal};

fn main() -> wll_sdk::SdkResult<()> {
    let wll = Wll::init()?;

    // Simulate a series of operations.
    wll.commit(CommitProposal::new("Initialize schema"))?;
    wll.commit(CommitProposal::new("Import baseline data"))?;
    wll.commit(CommitProposal::new("Apply transformation"))?;

    // Step 1: Verify chain integrity.
    let report = wll.verify()?;
    assert!(report.is_valid());
    println!("[AUDIT] Chain integrity: PASSED ({} receipts)", report.receipt_count);

    // Step 2: Replay from genesis to reconstruct final state.
    let replay = wll.replay()?;
    println!(
        "[AUDIT] Replay: {} outcomes applied, {} receipts evaluated",
        replay.applied_outcomes,
        replay.evaluated_receipts,
    );

    // Step 3: Inspect the current projection.
    let state = wll.latest_state()?;
    println!("[AUDIT] Trajectory length: {}", state.trajectory_length);

    // Step 4: Walk the log for a human-readable audit trail.
    let log = wll.log(100)?;
    println!("[AUDIT] Receipt log ({} entries):", log.len());
    for entry in &log {
        println!(
            "  #{:<4} {} | {} | accepted={:?}",
            entry.seq,
            entry.kind,
            entry.intent.as_deref().unwrap_or("(none)"),
            entry.accepted,
        );
    }

    Ok(())
}
```

---

## 10. Best Practices

### Worldline Identity Management

- **Production systems** should derive worldlines from stable, well-documented identity
  material (organization ID, system name, environment). Avoid `Wll::init()` in
  production; prefer `Wll::init_with_worldline()` with a deterministic seed.
- **Tests** can use `Wll::init()` for isolation, or derive from a known seed for
  reproducibility.

### Commit Hygiene

- **Always set an intent** for commits that will be queried programmatically. The intent
  field is designed for machine consumption, while the message is for humans.
- **Use specific commitment classes.** `ContentUpdate` is a safe default, but explicitly
  choosing `PolicyChange`, `StructuralChange`, or `IdentityOperation` improves audit
  clarity and can enable class-specific gating rules.
- **Attach evidence** for any commit that needs to be traceable back to an external
  decision. Compliance-critical changes should reference the authorizing ticket, policy
  document, or approval record.

### Content Organization

- **Build trees bottom-up.** Create leaf blobs first, assemble subtrees, then compose
  the root. This matches the natural dependency order and produces clean diffs.
- **Attach trees to commits.** While WLL allows commits without trees, linking a root
  tree to each commit creates a full snapshot of the content at that point, enabling
  richer provenance queries.

### Verification

- **Verify after critical sequences.** Run `wll.verify()` after batch imports, migration
  scripts, or any operation that writes many receipts. Early detection of chain
  corruption is far easier to remediate than late detection.
- **Use replay for reconciliation.** When two systems disagree on state, replay both
  from genesis and compare the resulting state maps.

### Error Handling

- **Match on specific variants** when your logic needs to distinguish between failure
  modes (e.g., retrying on `Store` errors but aborting on `CommitmentRejected`).
- **Use `?` propagation** for pipeline-style code where any error should abort the
  operation.
- **Log the `Display` representation.** All `SdkError` variants implement `Display`
  with actionable messages.

### Performance Considerations

- **The in-memory stores** (`InMemoryObjectStore`, `InMemoryLedger`) are suitable for
  testing, embedded use, and moderate-scale workloads. For high-throughput production
  scenarios, consider the persistent store backends provided by the lower-level crates.
- **`log()` with a limit** avoids scanning the full receipt history. Use the smallest
  limit that satisfies your query.
- **`latest_state()` is cheaper than `replay()`.** Use the projection for operational
  reads and reserve replay for audit or reconciliation workflows.

### Testing

Write deterministic tests by deriving worldlines from known seeds:

```rust
#[cfg(test)]
mod tests {
    use wll_sdk::{Wll, CommitProposal, WorldlineId};
    use wll_types::IdentityMaterial;

    fn test_worldline(seed: u8) -> WorldlineId {
        WorldlineId::derive(&IdentityMaterial::GenesisHash([seed; 32]))
    }

    #[test]
    fn commit_and_verify_roundtrip() {
        let wll = Wll::init_with_worldline(test_worldline(1)).unwrap();

        wll.commit(CommitProposal::new("test commit")).unwrap();

        let report = wll.verify().unwrap();
        assert!(report.is_valid());
        assert_eq!(report.receipt_count, 2);

        let log = wll.log(10).unwrap();
        assert_eq!(log.len(), 2);
        assert!(log[0].seq > log[1].seq);
    }
}
```

---

## Crate Dependency Map

The SDK sits at the top of the WLL crate hierarchy. Understanding the layers helps when
you need to reach below the facade:

```
wll-sdk          High-level facade (this guide)
  +-- wll-store    Content-addressable object storage
  +-- wll-ledger   Append-only receipt ledger, validation, replay
  +-- wll-refs     Branch and HEAD reference management
  +-- wll-dag      Provenance DAG construction
  +-- wll-types    Core types: ObjectId, WorldlineId, CommitmentClass
  +-- wll-crypto   BLAKE3 hashing, Ed25519 signatures
  +-- wll-fabric   Substrate orchestration
  +-- wll-gate     Capability-based access control
  +-- wll-index    Content indexing
  +-- wll-diff     Tree and blob differencing
  +-- wll-merge    Three-way merge engine
  +-- wll-pack     Pack file format (storage efficiency)
  +-- wll-sync     Replication and synchronization
  +-- wll-protocol Wire protocol definitions
```

For most applications, `wll-sdk` is the only dependency you need. The lower-level
crates are available when you require fine-grained control over specific subsystems.
