# Getting Started with WorldLine Ledger (WLL)

WorldLine Ledger (WLL) is a next-generation version control system built in Rust. Unlike
traditional VCS tools, WLL treats every change as a **commitment** recorded in a
cryptographic **receipt chain** -- an append-only, hash-linked ledger that captures not
only *what* changed but *why* it changed, *who* authorized it, and *what evidence*
supports the decision.

This tutorial walks you through installation, daily workflows, provenance queries, remote
collaboration, server setup, and SDK integration.

---

## Table of Contents

1. [Installation](#1-installation)
2. [Your First Repository](#2-your-first-repository)
3. [Understanding the Receipt Log](#3-understanding-the-receipt-log)
4. [Branching and Merging](#4-branching-and-merging)
5. [Remote Operations](#5-remote-operations)
6. [Provenance Queries](#6-provenance-queries)
7. [Server Setup](#7-server-setup)
8. [Using the SDK](#8-using-the-sdk)
9. [Configuration](#9-configuration)
10. [Comparison with Git](#10-comparison-with-git)

---

## 1. Installation

### Prerequisites

- **Rust toolchain** 1.80 or later (install via [rustup](https://rustup.rs))
- A C linker (provided by your platform's build-essentials package)

### Build from source

Clone the repository and install the CLI binary:

```bash
git clone https://github.com/mapleaiorg/wll.git
cd wll
cargo install --path crates/wll-cli
```

Verify the installation:

```bash
wll --version
```

### Add to an existing Rust project as a library

To use the SDK crate in your own Rust project:

```bash
cargo add wll-sdk
```

---

## 2. Your First Repository

### Initialize a repository

```bash
mkdir my-project && cd my-project
wll init
```

This creates a `.wll/` directory containing the ledger store, receipt chain, and
configuration. WLL also generates a **WorldLine** -- a cryptographic Ed25519 identity --
for the repository. Your WorldLine is displayed as a prefixed hash, for example
`wl:a3f8c1...`.

To create a bare repository (no working directory), pass the `--bare` flag:

```bash
wll init --bare /srv/repos/my-project.wll
```

### Stage and commit with provenance

Create a file and stage it:

```bash
echo "Hello, WorldLine." > README.md
wll add README.md
```

Check what is staged:

```bash
wll status
```

Now create a commitment. Every commitment in WLL records a **message**, and can
optionally include an **intent** (the human-readable reason), a **class** (the category
of change), and one or more **evidence** URIs linking to external justifications:

```bash
wll commit -m "Initial project scaffold" \
    --intent "Bootstrap the repository with a README" \
    --class ContentUpdate
```

The output includes the **receipt ID** -- the unique, content-addressed hash of the
commitment/outcome pair that was appended to the receipt chain.

### Attach evidence

Evidence URIs link a commitment to external artifacts such as issue trackers, code review
URLs, or CVE identifiers:

```bash
wll commit -m "Patch CVE-2025-1234" \
    --intent "Mitigate memory-safety vulnerability in parser" \
    --class SecurityPatch \
    --evidence https://nvd.nist.gov/vuln/detail/CVE-2025-1234 \
    --evidence https://github.com/mapleaiorg/wll/pull/42
```

This creates a fully traceable record that auditors can follow from the code change back
to the vulnerability disclosure and the review that approved the fix.

---

## 3. Understanding the Receipt Log

The receipt chain is the backbone of WLL. Each entry is a **Commitment --> Outcome** pair
linked to the previous entry by its cryptographic hash, forming an append-only ledger.

### View the log

```bash
wll log
```

Sample output:

```
receipt  r:8b3fa10e...
class    ContentUpdate
intent   Bootstrap the repository with a README
author   wl:a3f8c1...
date     2026-02-25T10:32:00Z

    Initial project scaffold
```

Use `--oneline` for a compact listing:

```bash
wll log --oneline
```

```
r:8b3fa10e  Initial project scaffold
r:5c1d90ab  Patch CVE-2025-1234
```

Limit the number of entries:

```bash
wll log -n 5
```

### Inspect a specific receipt

```bash
wll show r:8b3fa10e
```

This displays the full receipt including the commitment message, class, intent, evidence
URIs, author WorldLine, timestamp, parent hash, and outcome snapshot anchor.

### View differences

To see unstaged changes in the working directory:

```bash
wll diff
```

---

## 4. Branching and Merging

WLL supports branches for parallel lines of development. The branching model is similar
to what you may know from Git, but every branch operation is itself recorded in the
receipt chain.

### List branches

```bash
wll branch
```

### Create and switch to a new branch

```bash
wll switch -c feature/auth
```

This creates `feature/auth` and switches to it in one step. You can also create a branch
without switching:

```bash
wll branch feature/auth
```

Then switch later:

```bash
wll switch feature/auth
```

### Merge a branch

When work on a branch is complete, merge it back:

```bash
wll switch main
wll merge feature/auth
```

The merge commitment is recorded in the receipt chain with full provenance, linking the
two parent histories together.

### Delete a branch

```bash
wll branch -d feature/auth
```

---

## 5. Remote Operations

WLL supports remote repositories for collaboration. Remotes can be other WLL servers
or bare repositories accessible over the network.

### Add a remote

```bash
wll remote add origin https://wll.example.com/repos/my-project
```

### Push to a remote

```bash
wll push origin main
```

### Fetch and pull

Fetch downloads new receipts and objects from the remote without modifying your working
directory:

```bash
wll fetch origin
```

Pull fetches and then merges the remote branch into your current branch:

```bash
wll pull origin main
```

### Remove a remote

```bash
wll remote remove origin
```

---

## 6. Provenance Queries

Provenance is what sets WLL apart from traditional version control. The receipt chain
enables powerful queries that trace the lineage of every change.

### Trace a receipt's provenance

Given a receipt ID, walk the chain backwards to the genesis commitment, showing every
ancestor and its metadata:

```bash
wll provenance r:8b3fa10e
```

### Show the impact graph

Determine which files, branches, or downstream receipts were affected by a particular
commitment:

```bash
wll impact r:8b3fa10e
```

### Verify receipt chain integrity

Validate the entire receipt chain, checking hash linkage, sequence monotonicity (receipts
are in strictly increasing order), outcome attribution, and snapshot anchoring:

```bash
wll verify
```

A successful verification produces a clean report. Any inconsistency is flagged with the
offending receipt and the nature of the violation.

### Replay the ledger

Re-execute the ledger from the genesis receipt, confirming that every commitment produces
the expected outcome:

```bash
wll replay
```

### Show the audit trail

Display a chronological audit trail of all operations performed on the repository,
including branch creation, merges, and administrative actions:

```bash
wll audit
```

---

## 7. Server Setup

WLL includes a built-in server for hosting repositories over the network.

### Start the server

```bash
wll serve --bind 0.0.0.0:9807 --root /srv/wll-repos
```

This starts a WLL protocol server listening on port 9807, serving all repositories found
under `/srv/wll-repos`.

Clients can then add this server as a remote:

```bash
wll remote add origin https://wll.example.com:9807/my-project
```

---

## 8. Using the SDK

The `wll-sdk` crate provides a Rust library for programmatic interaction with WLL
repositories. Use it when you need to integrate WLL into build systems, CI pipelines, or
custom tooling.

### Add the dependency

```toml
[dependencies]
wll-sdk = "0.1"
```

### Initialize and write data

```rust
use wll_sdk::{Wll, CommitProposal, CommitmentClass};

fn main() -> anyhow::Result<()> {
    // Open (or initialize) a repository in the current directory
    let wll = Wll::init()?;

    // Write a blob into the object store
    let blob_id = wll.write_blob(b"Hello from the SDK")?;
    println!("Stored blob: {blob_id}");

    Ok(())
}
```

### Create a commitment

```rust
use wll_sdk::{Wll, CommitProposal, CommitmentClass};

fn main() -> anyhow::Result<()> {
    let wll = Wll::init()?;

    let result = wll.commit(
        CommitProposal::new("Automated formatting pass")
            .with_intent("Enforce project style guidelines")
            .with_class(CommitmentClass::ContentUpdate),
    )?;

    println!("Receipt: {}", result.receipt_id());
    Ok(())
}
```

### Verify the receipt chain programmatically

```rust
use wll_sdk::Wll;

fn main() -> anyhow::Result<()> {
    let wll = Wll::init()?;
    let report = wll.verify()?;

    if report.is_valid() {
        println!("Chain integrity verified: {} receipts", report.receipt_count());
    } else {
        for violation in report.violations() {
            eprintln!("Violation: {violation}");
        }
    }

    Ok(())
}
```

---

## 9. Configuration

WLL stores configuration at two levels:

- **Repository-level**: `.wll/config` (applies to the current repository)
- **Global-level**: `~/.config/wll/config` (applies to all repositories for the current
  user)

### Read a value

```bash
wll config user.name
```

### Set a value (repository-level)

```bash
wll config user.name "Ada Lovelace"
wll config user.email "ada@example.com"
```

### Set a value (global)

```bash
wll config --global user.name "Ada Lovelace"
wll config --global user.email "ada@example.com"
```

### Common configuration keys

| Key | Description |
|-----|-------------|
| `user.name` | Author name attached to commitments |
| `user.email` | Author email attached to commitments |
| `core.editor` | Default text editor for commitment messages |
| `remote.<name>.url` | URL for a named remote |

---

## 10. Comparison with Git

The following table maps common Git workflows to their WLL equivalents. If you are
familiar with Git, this should help you get productive quickly.

| Task | Git | WLL |
|------|-----|-----|
| Initialize a repository | `git init` | `wll init` |
| Stage files | `git add <paths>` | `wll add <paths>` |
| Check status | `git status` | `wll status` |
| Commit changes | `git commit -m "msg"` | `wll commit -m "msg"` |
| Commit with metadata | *(no built-in equivalent)* | `wll commit -m "msg" --intent "reason" --class ContentUpdate --evidence <uri>` |
| View history | `git log` | `wll log` |
| Compact history | `git log --oneline` | `wll log --oneline` |
| Inspect a commit | `git show <sha>` | `wll show <receipt>` |
| View diff | `git diff` | `wll diff` |
| Create a branch | `git branch <name>` | `wll branch <name>` |
| Switch branches | `git switch <name>` | `wll switch <name>` |
| Create and switch | `git switch -c <name>` | `wll switch -c <name>` |
| Delete a branch | `git branch -d <name>` | `wll branch -d <name>` |
| Merge | `git merge <branch>` | `wll merge <branch>` |
| Tag a release | `git tag -a v1.0 -m "msg"` | `wll tag v1.0 -m "msg"` |
| Add a remote | `git remote add <n> <url>` | `wll remote add <n> <url>` |
| Push | `git push origin main` | `wll push origin main` |
| Pull | `git pull origin main` | `wll pull origin main` |
| Fetch | `git fetch origin` | `wll fetch origin` |
| Garbage collect | `git gc` | `wll gc` |
| Repack objects | `git repack` | `wll repack` |
| Integrity check | `git fsck` | `wll fsck` |
| Trace provenance | *(no equivalent)* | `wll provenance <receipt>` |
| Impact analysis | *(no equivalent)* | `wll impact <receipt>` |
| Verify chain integrity | *(no equivalent)* | `wll verify` |
| Replay ledger | *(no equivalent)* | `wll replay` |
| Audit trail | *(no equivalent)* | `wll audit` |
| Serve repositories | *(requires separate daemon)* | `wll serve --bind <addr> --root <path>` |

### Key differences from Git

- **Provenance is first-class.** Every commitment records *why* a change was made
  (intent), *what kind* of change it is (class), and *what external evidence* supports it.
  Git has no built-in equivalent.

- **Receipt chain, not commit graph.** WLL uses an append-only, hash-linked receipt chain
  rather than a DAG of commits. This guarantees strict ordering and makes tamper detection
  straightforward.

- **Cryptographic identity.** Authors are identified by Ed25519-derived WorldLines
  (`wl:xxxx...`), not by name/email pairs. This provides non-repudiable attribution.

- **Built-in verification.** `wll verify` validates the entire receipt chain in one
  command -- checking hash linkage, monotonic sequencing, outcome attribution, and snapshot
  anchoring. Git's `fsck` checks object integrity but does not verify causal ordering.

- **Gate policies.** WLL supports configurable gate pipelines that can accept or reject
  commitments based on policy rules, enforced at the ledger level rather than via
  external hooks.

---

## Next Steps

- Run `wll --help` or `wll <command> --help` for detailed usage information on any
  command.
- Explore the `wll-sdk` crate documentation for advanced programmatic workflows.
- Set up a WLL server to enable team collaboration with full provenance tracking.
