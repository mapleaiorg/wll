<p align="center">
  <strong>WorldLine Ledger</strong><br>
  <em>Next-Generation Version Control with Built-In Provenance</em>
</p>

<p align="center">
  <a href="https://github.com/mapleaiorg/wll/actions"><img src="https://img.shields.io/github/actions/workflow/status/mapleaiorg/wll/ci.yml?branch=main&label=CI" alt="CI"></a>
  <a href="https://crates.io/crates/wll-sdk"><img src="https://img.shields.io/crates/v/wll-sdk.svg" alt="crates.io"></a>
  <a href="https://docs.rs/wll-sdk"><img src="https://img.shields.io/docsrs/wll-sdk" alt="docs.rs"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="License"></a>
  <a href="https://github.com/mapleaiorg/wll"><img src="https://img.shields.io/badge/rust-1.80%2B-orange.svg" alt="Rust Version"></a>
</p>

---

## What is WLL?

**WorldLine Ledger (WLL)** is a cryptographically-anchored version control system that records not just *what* changed, but *why* it changed, *who* authorized it, and *what happened* as a result. Every mutation flows through a commitment boundary that produces an append-only, hash-linked receipt chain — giving you an unforgeable audit trail from genesis.

WLL is designed as a **drop-in replacement for Git** with the same familiar workflow (`init`, `add`, `commit`, `push`, `pull`, `branch`, `merge`) while adding provenance-native primitives that Git cannot express:

| Feature | Git | WLL |
|---------|-----|-----|
| Content tracking | ✅ | ✅ |
| Cryptographic integrity | SHA-1/SHA-256 | BLAKE3 + Ed25519 |
| Commit intent & evidence | ❌ | ✅ Every commit declares *why* |
| Commitment classes | ❌ | ✅ ContentUpdate, PolicyChange, SecurityPatch, … |
| Receipt chain (append-only) | ❌ | ✅ Commitment → Outcome pairs |
| Causal ordering | ❌ | ✅ Hybrid Logical Clock |
| Policy gates | ❌ | ✅ Pre-commit policy pipeline |
| Replay & audit | ❌ | ✅ Deterministic replay from genesis |
| Provenance queries | ❌ | ✅ Trace any change to its root cause |
| WorldLine identity | ❌ | ✅ Cryptographic agent identity |

## Quick Start

### Install from source

```bash
git clone https://github.com/mapleaiorg/wll.git
cd wll
cargo install --path crates/wll-cli
```

### Create your first repository

```bash
# Initialize a new WLL repository
wll init my-project
cd my-project

# Stage and commit with intent
wll add .
wll commit -m "Initial project setup" \
  --intent "Bootstrap project structure" \
  --class ContentUpdate \
  --evidence "issue://PROJ-001"

# View the receipt log
wll log

# Verify the entire receipt chain
wll verify
```

### Use as a library (Rust SDK)

Add to your `Cargo.toml`:

```toml
[dependencies]
wll-sdk = "0.1"
```

```rust
use wll_sdk::{Wll, CommitProposal, CommitmentClass};

fn main() -> anyhow::Result<()> {
    // Initialize a new repository
    let wll = Wll::init()?;

    // Write content
    let blob_id = wll.write_blob(b"Hello, WorldLine!")?;

    // Commit with full provenance
    let result = wll.commit(
        CommitProposal::new("Add greeting")
            .with_intent("Establish initial content")
            .with_class(CommitmentClass::ContentUpdate)
            .with_evidence("ticket://WLL-42")
    )?;

    println!("Receipt: {}", hex::encode(result.receipt_hash));

    // Verify chain integrity
    let report = wll.verify()?;
    assert!(report.is_valid());

    // Replay from genesis
    let replay = wll.replay()?;
    println!("Applied {} outcomes", replay.applied_outcomes);

    Ok(())
}
```

## Architecture

WLL is built as **17 composable crates** organized in six layers:

```
┌─────────────────────────────────────────────────────────────┐
│  Application Layer                                          │
│  ┌─────────┐  ┌─────────┐                                   │
│  │ wll-cli │  │ wll-sdk │                                   │
│  └────┬────┘  └─────┬───┘                                   │
├───────┼─────────────┼───────────────────────────────────────┤
│  Distribution Layer │                                       │
│  ┌──────────┐ ┌─────┴──────┐ ┌────────────┐ ┌────────────┐  │
│  │ wll-pack │ │wll-protocol│ │  wll-sync  │ │ wll-server │  │
│  └────┬─────┘ └─────┬──────┘ └──────┬─────┘ └──────┬─────┘  │
├───────┼─────────────┼───────────────┼──────────────┼────────┤
│  Workflow Layer                                             │
│  ┌─────────┐ ┌──────────┐ ┌─────────┐ ┌──────────┐          │
│  │wll-refs │ │wll-index │ │wll-diff │ │wll-merge │          │
│  └────┬────┘ └────┬─────┘ └────┬────┘ └─────┬────┘          │
├───────┼───────────┼────────────┼────────────┼───────────────┤
│  Policy Layer                                               │
│  ┌──────────┐                                               │
│  │ wll-gate │                                               │
│  └────┬─────┘                                               │
├───────┼─────────────────────────────────────────────────────┤
│  Core Layer                                                 │
│  ┌─────────┐ ┌───────────┐ ┌────────────┐                   │
│  │ wll-dag │ │wll-ledger │ │ wll-fabric │                   │
│  └────┬────┘ └─────┬─────┘ └───────┬────┘                   │
├───────┼────────────┼───────────────┼────────────────────────┤
│  Foundation Layer                                           │
│  ┌───────────┐ ┌───────────┐ ┌───────────┐                  │
│  │ wll-types │ │wll-crypto │ │ wll-store │                  │
│  └───────────┘ └───────────┘ └───────────┘                  │
└─────────────────────────────────────────────────────────────┘
```

### Crate Overview

| Layer | Crate | Purpose |
|-------|-------|---------|
| **Foundation** | `wll-types` | Core types: ObjectId, WorldlineId, TemporalAnchor, CommitmentClass |
| | `wll-crypto` | BLAKE3 hashing with domain separation, Ed25519 signatures |
| | `wll-store` | Content-addressable object store (blob, tree, receipt, snapshot) |
| **Core** | `wll-dag` | Provenance DAG with causal ancestry tracking |
| | `wll-ledger` | Append-only receipt chain: Commitment→Outcome pairs, replay, validation |
| | `wll-fabric` | Temporal fabric: Hybrid Logical Clock ordering |
| **Policy** | `wll-gate` | Commitment boundary: policy pipeline, capability-based access |
| **Workflow** | `wll-refs` | Branch, tag, and remote ref management with HEAD tracking |
| | `wll-index` | Staging area and working-tree state tracking |
| | `wll-diff` | Tree-to-tree and blob-to-blob differencing |
| | `wll-merge` | Three-way merge with conflict detection |
| **Distribution** | `wll-pack` | Packfile format: zstd compression, fan-out index, delta encoding |
| | `wll-sync` | Push/pull/fetch with receipt chain verification |
| | `wll-protocol` | Wire protocol: framed messages, capability negotiation |
| | `wll-server` | HTTP/2 server with auth, hooks, and policy enforcement |
| **Application** | `wll-cli` | Full-featured CLI: `wll init`, `commit`, `push`, `verify`, … |
| | `wll-sdk` | High-level Rust SDK for embedding WLL in applications |

## Key Concepts

### WorldLine Identity

Every repository is anchored to a **WorldLine** — a cryptographic identity derived from Ed25519 key material. A WorldLine is the root of trust for all receipts in a repository.

```
WorldLine: wl:7a3f...c8d2
  └── Receipt Chain
       ├── r#1 Commitment (ContentUpdate) → Accepted
       ├── r#2 Outcome (state effects applied)
       ├── r#3 Commitment (PolicyChange) → Accepted
       └── r#4 Outcome (policy updated)
```

### Receipt Chain

Unlike Git's commit graph, WLL maintains an **append-only receipt chain**. Every mutation produces a pair:

1. **CommitmentReceipt** — Records *what* was proposed, *why* (intent), *by whom*, and the gate's *decision*
2. **OutcomeReceipt** — Records *what actually happened* — the state effects, proofs, and metadata

Receipts are hash-linked (`prev_hash → receipt_hash`), causally ordered via HLC timestamps, and cryptographically signed.

### Commitment Boundary (Gate)

Every change must pass through the **Gate** — a policy pipeline that evaluates:

- **Capability requirements** — Does the agent have permission?
- **Policy rules** — Custom pre-commit checks (format, size, compliance)
- **Evidence validation** — Are referenced artifacts accessible?

The Gate produces an `Accepted` or `Rejected` decision, which is recorded in the commitment receipt. Rejected commitments are still logged for auditability.

### Temporal Fabric

WLL uses a **Hybrid Logical Clock (HLC)** for causal ordering across distributed repositories:

```
TemporalAnchor {
    physical_ms: 1708905600000,  // Wall clock
    logical: 0,                   // Lamport counter
    node_id: 42,                  // Agent ID
}
```

This ensures receipts from different repositories can be correctly ordered even with clock skew.

## CLI Reference

```
wll — WorldLine Ledger CLI

USAGE:
    wll <COMMAND>

CORE COMMANDS:
    init          Initialize a new WLL repository
    status        Show working directory status
    add           Stage files for commitment
    commit        Create a commitment with receipt chain
    log           Show receipt history
    show          Display a specific receipt

BRANCH & TAG:
    branch        List, create, or delete branches
    switch        Switch to a different branch
    tag           Create or list tags
    merge         Merge a branch into current branch
    diff          Show changes between states

REMOTE & SYNC:
    remote        Manage remote repositories
    fetch         Download objects and receipts from a remote
    pull          Fetch and merge from a remote branch
    push          Upload objects and receipts to a remote

PROVENANCE (WLL-specific):
    provenance    Trace the full causal chain for a receipt
    impact        Show downstream impact graph
    verify        Verify receipt chain integrity
    replay        Replay and verify state from genesis
    audit         Show full audit trail

MAINTENANCE:
    gc            Garbage collect unreachable objects
    repack        Repack loose objects into packfiles
    fsck          Full repository integrity check
    config        Get or set configuration values
    serve         Start the WLL server daemon
```

### Commit with Evidence

```bash
# Standard commit
wll commit -m "Fix authentication bypass"

# Commit with full provenance metadata
wll commit \
  -m "Fix authentication bypass" \
  --intent "Patch CVE-2024-1234 by validating session tokens" \
  --class SecurityPatch \
  --evidence "cve://CVE-2024-1234" \
  --evidence "review://PR-567"
```

### Verify & Audit

```bash
# Verify receipt chain integrity
wll verify
# ✓ Receipt chain integrity verified
#   Hash chain: valid
#   Sequences: monotonic
#   Outcomes: attributed
#   Snapshots: anchored

# Replay the entire ledger from genesis
wll replay

# Show full audit trail
wll audit
```

## Server

WLL includes a built-in HTTP/2 server for hosting remote repositories:

```bash
# Start the server
wll serve --bind 0.0.0.0:9418 --root /var/wll/repos

# Or configure via TOML
cat > wll-server.toml <<EOF
bind_addr = "0.0.0.0:9418"
repos_root = "/var/wll/repos"
max_pack_size = 104857600
max_connections = 256
allow_anonymous_read = true

[tls]
cert_path = "/etc/wll/cert.pem"
key_path = "/etc/wll/key.pem"
EOF
```

The server supports:
- **Authentication** — Bearer token, SSH key, mutual TLS, or anonymous
- **Authorization** — Per-repository read/write/admin permissions
- **Server-side hooks** — Pre-receive and post-receive hooks for policy enforcement
- **Receipt verification** — Incoming receipts are verified before storage

## Performance

WLL is written in Rust for maximum performance and safety:

- **BLAKE3 hashing** — 2-3x faster than SHA-256 on modern hardware
- **zstd compression** — Better compression ratios than gzip at similar speeds
- **Memory-mapped I/O** — Zero-copy reads for packfiles and indexes
- **Async I/O** — Non-blocking network operations via Tokio
- **Fan-out index** — O(log n) object lookup in packfiles

## Development

### Build

```bash
git clone https://github.com/mapleaiorg/wll.git
cd wll
cargo build --workspace
```

### Test

```bash
# Run all 435 tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p wll-ledger
cargo test -p wll-sdk
```

### Project Structure

```
wll/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── wll-types/          # Core type definitions
│   ├── wll-crypto/         # BLAKE3 + Ed25519
│   ├── wll-store/          # Content-addressable store
│   ├── wll-dag/            # Provenance DAG
│   ├── wll-ledger/         # Receipt chain + replay
│   ├── wll-fabric/         # Temporal fabric (HLC)
│   ├── wll-gate/           # Commitment boundary
│   ├── wll-refs/           # Branch/tag/remote refs
│   ├── wll-index/          # Staging area
│   ├── wll-diff/           # Tree & blob diffing
│   ├── wll-merge/          # Three-way merge
│   ├── wll-pack/           # Packfile format
│   ├── wll-sync/           # Push/pull/fetch
│   ├── wll-protocol/       # Wire protocol
│   ├── wll-server/         # HTTP/2 server
│   ├── wll-cli/            # Command-line interface
│   └── wll-sdk/            # High-level SDK
├── docs/                   # Documentation
│   ├── architecture.md     # System architecture
│   ├── getting-started.md  # Tutorial
│   ├── cli-reference.md    # CLI command reference
│   └── sdk-guide.md        # SDK programming guide
└── README.md
```

## Why WLL?

### For Software Teams
- **Auditability** — Every change has a recorded intent, evidence chain, and outcome. Compliance-ready from day one.
- **Accountability** — Know not just *who* committed, but *why* they committed, *what evidence* supported it, and *what the policy gate decided*.
- **Replay** — Deterministically replay the entire history to verify correctness.

### For Regulated Industries
- **Tamper-evident** — Hash-linked receipt chain makes unauthorized modifications detectable.
- **Policy enforcement** — Configurable gates ensure every change meets compliance requirements before acceptance.
- **Provenance tracking** — Trace any artifact back to its root cause through the causal DAG.

### For AI/ML Workflows
- **Model provenance** — Track training data, hyperparameters, and model versions with cryptographic receipts.
- **Experiment reproducibility** — Replay training runs from genesis with full evidence chains.
- **Agent accountability** — When AI agents make changes, the commitment boundary ensures they declare intent and evidence.

## Contributing

We welcome contributions! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

```bash
# Development workflow
git clone https://github.com/mapleaiorg/wll.git
cd wll
cargo test --workspace          # Run all tests
cargo clippy --workspace        # Lint
cargo fmt --all -- --check      # Format check
```

## License

Licensed under the [Apache License, Version 2.0](LICENSE).

---

<p align="center">
  Built with ❤️ by <a href="https://github.com/mapleaiorg">MapleAI</a>
</p>
