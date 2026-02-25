# WLL CLI Reference

Complete command-line reference for `wll`, the WorldLine Ledger CLI.

**Version:** 0.1.0
**Binary:** `wll`
**License:** Apache-2.0
**Minimum Rust Version:** 1.80

---

## Table of Contents

- [Global Options](#global-options)
- [Core Commands](#core-commands)
  - [wll init](#wll-init)
  - [wll status](#wll-status)
  - [wll add](#wll-add)
  - [wll commit](#wll-commit)
  - [wll log](#wll-log)
  - [wll show](#wll-show)
- [Branch and Tag Commands](#branch-and-tag-commands)
  - [wll branch](#wll-branch)
  - [wll switch](#wll-switch)
  - [wll tag](#wll-tag)
  - [wll diff](#wll-diff)
  - [wll merge](#wll-merge)
- [Remote and Sync Commands](#remote-and-sync-commands)
  - [wll remote](#wll-remote)
  - [wll fetch](#wll-fetch)
  - [wll pull](#wll-pull)
  - [wll push](#wll-push)
- [Provenance Commands](#provenance-commands)
  - [wll provenance](#wll-provenance)
  - [wll impact](#wll-impact)
  - [wll verify](#wll-verify)
  - [wll replay](#wll-replay)
  - [wll audit](#wll-audit)
- [Maintenance Commands](#maintenance-commands)
  - [wll gc](#wll-gc)
  - [wll repack](#wll-repack)
  - [wll fsck](#wll-fsck)
- [Configuration](#configuration)
  - [wll config](#wll-config)
- [Server](#server)
  - [wll serve](#wll-serve)
- [Commitment Classes](#commitment-classes)
- [Common Workflows](#common-workflows)
- [Exit Codes](#exit-codes)
- [Environment Variables](#environment-variables)
- [Output Formats](#output-formats)

---

## Global Options

These options can be placed before any subcommand and apply globally.

| Option | Short | Description |
|--------|-------|-------------|
| `--verbose` | `-v` | Enable verbose output with detailed tracing information. |
| `--format <FORMAT>` | | Set the output format. Accepted values: `text` (default), `json`. |

```
wll --verbose status
wll --format json log
wll -v commit -m "update"
```

---

## Core Commands

### wll init

Initialize a new WLL repository.

```
wll init [PATH] [--bare]
```

**Arguments:**

| Argument | Required | Description |
|----------|----------|-------------|
| `PATH` | No | Directory path in which to initialize the repository. Defaults to the current directory (`.`). |

**Flags:**

| Flag | Description |
|------|-------------|
| `--bare` | Create a bare repository (no working directory). Used for shared server-side repositories. |

**Output:**

```
✓ Initialized WLL repository in .
  WorldLine: wl:a1b2c3d4...
  Branch: main
```

For bare repositories:

```
✓ Initialized bare WLL repository in /srv/repos/project.wll
  WorldLine: wl:a1b2c3d4...
  Branch: main
```

**Examples:**

```bash
# Initialize in the current directory
wll init

# Initialize in a specific path
wll init /home/user/projects/my-ledger

# Initialize a bare repository for remote hosting
wll init --bare /srv/repos/shared.wll
```

---

### wll status

Show the current state of the working directory, staging area, and receipt chain.

```
wll status
```

**Arguments:** None.

**Output:**

```
On branch main
WorldLine: wl:a1b2c3d4...
Receipt chain: 12 receipts, integrity ✓

Changes staged for commitment:
  new file:   src/feature.rs

Changes not staged:
  modified:   README.md

Untracked files:
  docs/notes.txt
```

When the working directory is clean:

```
On branch main
WorldLine: wl:a1b2c3d4...
Receipt chain: 12 receipts, integrity ✓

No changes staged. Working directory clean.
```

**Examples:**

```bash
wll status
wll --format json status
```

---

### wll add

Stage one or more files for the next commitment.

```
wll add <PATHS>...
```

**Arguments:**

| Argument | Required | Description |
|----------|----------|-------------|
| `PATHS` | Yes | One or more file paths to add to the staging area. Accepts multiple arguments. |

**Output:**

```
  staged: src/feature.rs
  staged: tests/feature_test.rs
```

**Examples:**

```bash
# Stage a single file
wll add src/main.rs

# Stage multiple files
wll add src/lib.rs src/utils.rs tests/integration.rs

# Stage all Rust source files (using shell globbing)
wll add src/*.rs
```

---

### wll commit

Create a new commitment with a receipt chain entry. Commitments in WLL differ from traditional VCS commits by including structured intent, classification, and optional evidence.

```
wll commit [-m <MESSAGE>] [--intent <INTENT>] [--class <CLASS>] [--evidence <URI>...]
```

**Options:**

| Option | Short | Required | Description |
|--------|-------|----------|-------------|
| `--message <MESSAGE>` | `-m` | No | The commit message. If omitted, defaults to `"No message"`. |
| `--intent <INTENT>` | | No | Human-readable description of the intent behind this commitment. Defaults to the value of `--message` if not specified. |
| `--class <CLASS>` | | No | The commitment class for policy gating. Defaults to `ContentUpdate`. See [Commitment Classes](#commitment-classes) for the full list. |
| `--evidence <URI>` | | No | URI pointing to supporting evidence (issue tracker link, review URL, test report, etc.). Can be specified multiple times. |

**Output:**

```
✓ Commitment accepted
  Intent: Fix null pointer in authentication module
  Class: SecurityPatch
  Evidence: https://issues.example.com/SEC-1234
  Evidence: https://ci.example.com/runs/5678
  Receipt: r#42 e7a3b1c9
```

**Examples:**

```bash
# Simple commitment with a message
wll commit -m "Add user authentication module"

# Commitment with explicit intent and class
wll commit -m "Rotate signing keys" \
  --intent "Scheduled quarterly key rotation per security policy" \
  --class IdentityOperation

# Commitment with evidence URIs
wll commit -m "Fix CVE-2025-1234" \
  --class SecurityPatch \
  --evidence https://nvd.nist.gov/vuln/detail/CVE-2025-1234 \
  --evidence https://github.com/org/repo/pull/567

# Policy change with full metadata
wll commit -m "Update branch protection rules" \
  --intent "Require two approvals for main branch" \
  --class PolicyChange \
  --evidence https://jira.example.com/GOV-89
```

---

### wll log

Display the receipt history for the current branch.

```
wll log [--oneline] [--graph] [-n <LIMIT>] [--format <FORMAT>]
```

**Options:**

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--oneline` | | `false` | Display each receipt on a single line in compact format. |
| `--graph` | | `false` | Display an ASCII graph of the receipt chain topology. |
| `-n <LIMIT>` / `--limit <LIMIT>` | `-n` | `20` | Maximum number of entries to display. |

The global `--format` option also applies, enabling JSON output.

**Output (default):**

```
r#3  f4e2a1b8  (main)
  ✓ Accepted | PolicyChange
  Intent: Update access control policy

r#2  c9d8e7f6  (main)
  ✓ Accepted | ContentUpdate
  Intent: Add error handling to API layer

r#1  abc123de  (main)
  ✓ Accepted | ContentUpdate
  Intent: Initial commit
```

**Output (oneline):**

```
r#3 f4e2a1b8 Update access control policy
r#2 c9d8e7f6 Add error handling to API layer
r#1 abc123de Initial commit
```

**Examples:**

```bash
# Show the last 20 receipts (default)
wll log

# Compact format, last 5 entries
wll log --oneline -n 5

# Full log in JSON format
wll --format json log

# Graph view
wll log --graph
```

---

### wll show

Display detailed information about a specific receipt.

```
wll show <RECEIPT>
```

**Arguments:**

| Argument | Required | Description |
|----------|----------|-------------|
| `RECEIPT` | Yes | Receipt identifier. Accepts a receipt hash (e.g., `e7a3b1c9`) or a sequence number (e.g., `r#42` or `42`). |

**Output:**

```
Receipt r#42 — Type: Commitment, Seq: 42, Decision: Accepted
  Hash:      e7a3b1c9d8f7e6a5b4c3d2e1f0a9b8c7
  Parent:    d6e5f4a3b2c1d0e9f8a7b6c5d4e3f2a1
  Intent:    Fix null pointer in authentication module
  Class:     SecurityPatch
  Evidence:  https://issues.example.com/SEC-1234
  Timestamp: 2025-05-15T14:32:07Z
  Author:    developer@example.com
```

**Examples:**

```bash
# Show by hash prefix
wll show e7a3b1c9

# Show by sequence number
wll show 42
```

---

## Branch and Tag Commands

### wll branch

List, create, or delete branches.

```
wll branch [NAME] [-d/--delete]
```

**Arguments:**

| Argument | Required | Description |
|----------|----------|-------------|
| `NAME` | No | Branch name to create or delete. Omit to list all branches. |

**Flags:**

| Flag | Short | Description |
|------|-------|-------------|
| `--delete` | `-d` | Delete the named branch. Requires `NAME`. |

**Output (list):**

```
  feature/auth
* main
  release/v1.0
```

The active branch is marked with `*`.

**Output (create):**

```
Created branch feature/auth
```

**Output (delete):**

```
Deleted branch feature/auth
```

**Examples:**

```bash
# List all branches
wll branch

# Create a new branch
wll branch feature/new-auth

# Delete a branch
wll branch -d feature/old-experiment
```

---

### wll switch

Switch the working directory to a different branch.

```
wll switch <BRANCH> [-c/--create]
```

**Arguments:**

| Argument | Required | Description |
|----------|----------|-------------|
| `BRANCH` | Yes | Name of the target branch. |

**Flags:**

| Flag | Short | Description |
|------|-------|-------------|
| `--create` | `-c` | Create the branch if it does not already exist, then switch to it. |

**Output:**

```
Switched to feature/auth
```

With `--create`:

```
Created and switched to feature/auth
```

**Examples:**

```bash
# Switch to an existing branch
wll switch main

# Create and switch in one step
wll switch -c feature/new-module
```

---

### wll tag

Create, list, or delete tags. Tags are immutable references to specific receipt chain positions.

```
wll tag [NAME] [-m <MESSAGE>] [-d/--delete] [-l/--list]
```

**Arguments:**

| Argument | Required | Description |
|----------|----------|-------------|
| `NAME` | No | Tag name. Omit to list all tags (same as `--list`). |

**Options:**

| Option | Short | Description |
|--------|-------|-------------|
| `--message <MESSAGE>` | `-m` | Annotated tag message. When provided, creates an annotated tag with metadata. |
| `--delete` | `-d` | Delete the named tag. Requires `NAME`. |
| `--list` | `-l` | List all tags explicitly. |

**Output (list):**

```
v0.1.0
v0.2.0
v1.0.0-rc1
```

When no tags exist:

```
No tags.
```

**Output (create):**

```
Created tag v1.0.0
```

**Output (delete):**

```
Deleted tag v1.0.0-rc1
```

**Examples:**

```bash
# List all tags
wll tag

# Create a lightweight tag
wll tag v1.0.0

# Create an annotated tag with a message
wll tag v1.0.0 -m "First stable release"

# Delete a tag
wll tag -d v1.0.0-rc1
```

---

### wll diff

Show differences between the working directory and the last committed state.

```
wll diff [--staged]
```

**Flags:**

| Flag | Description |
|------|-------------|
| `--staged` | Show only changes that have been staged (added via `wll add`). |

**Output:**

```
diff --wll a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -10,3 +10,5 @@
 fn main() {
+    init_logging();
+    run_server();
 }
```

When there are no changes:

```
No changes.
```

**Examples:**

```bash
# Show all unstaged changes
wll diff

# Show staged changes only
wll diff --staged
```

---

### wll merge

Merge a branch into the current branch.

```
wll merge <BRANCH> [--strategy <STRATEGY>]
```

**Arguments:**

| Argument | Required | Description |
|----------|----------|-------------|
| `BRANCH` | Yes | Name of the branch to merge into the current branch. |

**Options:**

| Option | Required | Description |
|--------|----------|-------------|
| `--strategy <STRATEGY>` | No | Merge strategy to use. Strategy selection depends on the nature of the changes. |

**Output:**

```
✓ Merged feature/auth.
```

**Examples:**

```bash
# Merge a feature branch into main
wll switch main
wll merge feature/auth

# Merge with a specific strategy
wll merge feature/refactor --strategy recursive
```

---

## Remote and Sync Commands

### wll remote

Manage remote repository connections.

```
wll remote [SUBCOMMAND] [-v/--verbose]
```

**Subcommands:**

| Subcommand | Arguments | Description |
|------------|-----------|-------------|
| `add` | `<NAME> <URL>` | Register a new remote with the given name and URL. |
| `remove` | `<NAME>` | Remove an existing remote by name. |
| *(none)* | | List all configured remotes. |

**Flags:**

| Flag | Short | Description |
|------|-------|-------------|
| `--verbose` | `-v` | Show remote URLs alongside names when listing. |

**Output (list):**

When no remotes are configured:

```
No remotes configured.
```

**Output (add):**

```
Added remote origin → https://wll.example.com/org/repo
```

**Output (remove):**

```
Removed remote staging
```

**Examples:**

```bash
# List all remotes
wll remote

# List remotes with URLs
wll remote -v

# Add a remote
wll remote add origin https://wll.example.com/org/repo

# Add a second remote
wll remote add upstream https://wll.example.com/upstream/repo

# Remove a remote
wll remote remove staging
```

---

### wll fetch

Fetch objects and receipts from a remote repository without modifying the working directory.

```
wll fetch [REMOTE]
```

**Arguments:**

| Argument | Required | Default | Description |
|----------|----------|---------|-------------|
| `REMOTE` | No | `origin` | Name of the remote to fetch from. |

**Output:**

```
Fetching from origin... up to date
```

**Examples:**

```bash
# Fetch from the default remote (origin)
wll fetch

# Fetch from a specific remote
wll fetch upstream
```

---

### wll pull

Fetch from a remote and integrate changes into the current branch.

```
wll pull [REMOTE] [BRANCH]
```

**Arguments:**

| Argument | Required | Default | Description |
|----------|----------|---------|-------------|
| `REMOTE` | No | `origin` | Name of the remote to pull from. |
| `BRANCH` | No | `main` | Name of the remote branch to pull. |

**Output:**

```
Pulling origin/main... up to date
```

**Examples:**

```bash
# Pull from origin/main (defaults)
wll pull

# Pull a specific branch from a specific remote
wll pull upstream release/v2.0
```

---

### wll push

Push local receipts and objects to a remote repository.

```
wll push [REMOTE] [BRANCH]
```

**Arguments:**

| Argument | Required | Default | Description |
|----------|----------|---------|-------------|
| `REMOTE` | No | `origin` | Name of the remote to push to. |
| `BRANCH` | No | `main` | Name of the remote branch to push to. |

**Output:**

```
Pushing to origin/main... up to date
```

**Examples:**

```bash
# Push to origin/main (defaults)
wll push

# Push to a specific remote and branch
wll push upstream feature/auth
```

---

## Provenance Commands

These commands are unique to WLL and provide causal traceability, impact analysis, and integrity verification across the entire receipt chain.

### wll provenance

Trace the full causal provenance chain for a given receipt, walking backwards through parent links to show the complete history of how a particular state came to exist.

```
wll provenance <RECEIPT>
```

**Arguments:**

| Argument | Required | Description |
|----------|----------|-------------|
| `RECEIPT` | Yes | Receipt hash or sequence number to trace. |

**Output:**

```
Provenance for receipt e7a3b1c9
  r#42 e7a3b1c9 SecurityPatch  "Fix CVE-2025-1234"
    └─ r#41 d6e5f4a3 ContentUpdate  "Refactor auth module"
        └─ r#38 b4c3d2e1 ContentUpdate  "Add auth module"
            └─ r#1  abc123de ContentUpdate  "Initial commit"
```

**Examples:**

```bash
# Trace provenance by hash
wll provenance e7a3b1c9

# Trace provenance by sequence number
wll provenance 42
```

---

### wll impact

Show the downstream impact graph for a receipt, revealing all receipts that were causally influenced by or derived from a given change.

```
wll impact <RECEIPT>
```

**Arguments:**

| Argument | Required | Description |
|----------|----------|-------------|
| `RECEIPT` | Yes | Receipt hash or sequence number to analyze. |

**Output:**

```
Impact for receipt abc123de
  r#1 abc123de ContentUpdate  "Initial commit"
    ├─ r#2 c9d8e7f6 ContentUpdate  "Add core types"
    │   ├─ r#5 a1b2c3d4 ContentUpdate  "Add serialization"
    │   └─ r#8 f0e9d8c7 PolicyChange   "Add type policies"
    └─ r#3 e5f4a3b2 ContentUpdate  "Add build system"
```

**Examples:**

```bash
wll impact abc123de
wll impact 1
```

---

### wll verify

Verify the complete integrity of the receipt chain. Checks four independent properties to ensure no tampering or corruption has occurred.

```
wll verify
```

**Arguments:** None.

**Verification checks:**

| Check | Description |
|-------|-------------|
| Hash chain | Every receipt's hash correctly derives from its content and parent. |
| Sequence monotonicity | Sequence numbers are strictly increasing with no gaps. |
| Outcome attribution | Every decision (Accepted/Rejected) is traceable to a policy evaluation. |
| Snapshot anchoring | Snapshot receipts correctly anchor the state at their declared points. |

**Output (success):**

```
✓ Receipt chain integrity verified
  Hash chain: valid
  Sequences: monotonic
  Outcomes: attributed
  Snapshots: anchored
```

**Output (failure):**

```
✗ Receipt chain integrity check FAILED
  Hash chain: BROKEN at r#17 (expected a1b2c3, got d4e5f6)
  Sequences: monotonic
  Outcomes: attributed
  Snapshots: anchored
```

**Examples:**

```bash
wll verify
wll --format json verify
```

---

### wll replay

Replay the entire ledger from the genesis receipt, reconstructing state at each step and verifying consistency.

```
wll replay [--from-genesis]
```

**Flags:**

| Flag | Description |
|------|-------------|
| `--from-genesis` | Explicitly start replay from the genesis receipt (this is the default behavior). |

**Output:**

```
Replaying from genesis...
  r#1  abc123de ✓
  r#2  c9d8e7f6 ✓
  r#3  e5f4a3b2 ✓
  ...
✓ Replay complete. 42 receipts verified, state consistent.
```

**Examples:**

```bash
wll replay
wll replay --from-genesis
```

---

### wll audit

Display the full audit trail for the repository, providing a chronological record of all operations.

```
wll audit [WORLDLINE]
```

**Arguments:**

| Argument | Required | Description |
|----------|----------|-------------|
| `WORLDLINE` | No | Specific WorldLine ID to audit. Omit to audit the current WorldLine. |

**Output:**

```
Audit trail for wl:a1b2c3d4
  2025-05-01T09:00:00Z  r#1  Init           main
  2025-05-01T09:15:00Z  r#2  Commitment     ContentUpdate     "Add initial files"
  2025-05-02T14:30:00Z  r#3  Commitment     PolicyChange      "Set branch policy"
  2025-05-03T10:00:00Z  r#4  BranchCreate   feature/auth
  ...
```

When there are no receipts:

```
Audit trail: no receipts.
```

**Examples:**

```bash
# Audit the current WorldLine
wll audit

# Audit a specific WorldLine
wll audit wl:a1b2c3d4e5f6
```

---

## Maintenance Commands

### wll gc

Garbage collect unreachable objects. Removes objects that are no longer referenced by any receipt in the chain.

```
wll gc
```

**Arguments:** None.

**Output:**

```
✓ GC: 14 objects removed.
```

When nothing to collect:

```
✓ GC: 0 objects removed.
```

**Examples:**

```bash
wll gc
```

---

### wll repack

Repack loose objects into packfiles for improved storage efficiency and read performance.

```
wll repack
```

**Arguments:** None.

**Output:**

```
✓ Repack done. 238 objects packed into 1 packfile.
```

**Examples:**

```bash
wll repack
```

---

### wll fsck

Run a full integrity check on the repository storage layer, verifying object hashes, packfile indices, and structural consistency beyond what `wll verify` checks at the receipt chain level.

```
wll fsck
```

**Arguments:** None.

**Output (clean):**

```
✓ No issues.
```

**Output (issues found):**

```
✗ Issues found:
  CORRUPT  objects/a1/b2c3d4... (hash mismatch)
  MISSING  objects/e5/f6a7b8... (referenced by r#12)
  DANGLING objects/c9/d0e1f2... (not referenced)
```

**Examples:**

```bash
wll fsck
wll --verbose fsck
```

---

## Configuration

### wll config

Read or write configuration values. Configuration follows a hierarchical key-value model with dotted keys (e.g., `user.name`, `core.compression`).

```
wll config [KEY] [VALUE]
```

**Arguments:**

| Argument | Required | Description |
|----------|----------|-------------|
| `KEY` | No | Configuration key to read or write (e.g., `user.name`). Omit to list all configuration. |
| `VALUE` | No | Value to set. Omit to read the current value of `KEY`. |

**Output (read):**

```
user.name = Alice Developer
```

When not set:

```
user.name = (not set)
```

**Output (list all):**

```
user.name = Alice Developer
user.email = alice@example.com
core.compression = zstd
```

When empty:

```
No configuration keys set.
```

**Output (write):**

```
Set user.name = Alice Developer
```

**Examples:**

```bash
# List all configuration
wll config

# Read a specific key
wll config user.name

# Set a value
wll config user.name "Alice Developer"

# Set user email
wll config user.email "alice@example.com"

# Configure compression
wll config core.compression zstd
```

**Common Configuration Keys:**

| Key | Description |
|-----|-------------|
| `user.name` | Author name for commitments. |
| `user.email` | Author email for commitments. |
| `core.compression` | Compression algorithm for object storage. |

---

## Server

### wll serve

Start the WLL server daemon, enabling remote clients to fetch, pull, and push over the WLL protocol.

```
wll serve [--bind <ADDR>] [--root <PATH>]
```

**Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--bind <ADDR>` | `127.0.0.1:9418` | Address and port to bind the server to. |
| `--root <PATH>` | `.` | Path to the repository root to serve. |

**Output:**

```
WLL server on 127.0.0.1:9418 (root: .)
```

**Examples:**

```bash
# Start with defaults (localhost:9418, current directory)
wll serve

# Bind to all interfaces on a custom port
wll serve --bind 0.0.0.0:8080

# Serve a specific repository
wll serve --root /srv/repos/project.wll

# Bind to a custom address and serve a specific path
wll serve --bind 0.0.0.0:9420 --root /srv/repos/shared.wll
```

---

## Commitment Classes

Commitment classes control policy gating and determine what level of evidence, review, or signing is required. They are specified with the `--class` flag on `wll commit`.

| Class | Risk Level | Description |
|-------|------------|-------------|
| `ReadOnly` | 0 (lowest) | Read-only operations. No state mutation. |
| `ContentUpdate` | 1 | Normal file content changes. This is the default class. |
| `StructuralChange` | 2 | Directory reorganization, schema changes, or architectural modifications. |
| `PolicyChange` | 3 | Modifications to governance rules, branch policies, or access controls. |
| `IdentityOperation` | 4 (highest) | Key rotation, delegation changes, or identity-related operations. |
| `Custom(<name>)` | 2 (medium) | Domain-specific custom classes. Specify as a string (e.g., `--class deploy`). Defaults to medium risk. |

Higher risk levels may trigger additional policy checks such as mandatory evidence, multi-party review, or elevated signing requirements, depending on the repository's configured gate policies.

---

## Common Workflows

### Starting a New Project

```bash
mkdir my-project && cd my-project
wll init
wll config user.name "Alice Developer"
wll config user.email "alice@example.com"
wll add README.md src/main.rs
wll commit -m "Initial project setup" --class ContentUpdate
```

### Feature Branch Workflow

```bash
# Create and switch to a feature branch
wll switch -c feature/user-auth

# Make changes, stage, and commit
wll add src/auth.rs src/middleware.rs
wll commit -m "Add authentication module" \
  --intent "Implement OAuth2-based user authentication" \
  --evidence https://jira.example.com/FEAT-123

# Switch back and merge
wll switch main
wll merge feature/user-auth

# Clean up
wll branch -d feature/user-auth
```

### Security Patch with Full Provenance

```bash
wll switch -c hotfix/cve-2025-9999

wll add src/validation.rs
wll commit -m "Fix input validation bypass" \
  --class SecurityPatch \
  --intent "Patch CVE-2025-9999: input validation bypass in API layer" \
  --evidence https://nvd.nist.gov/vuln/detail/CVE-2025-9999 \
  --evidence https://github.com/org/repo/security/advisories/GHSA-xxxx

# Verify integrity after the fix
wll verify

# Trace the full provenance chain
wll provenance $(wll log --oneline -n 1 | awk '{print $1}')

wll switch main
wll merge hotfix/cve-2025-9999
wll push
```

### Remote Collaboration

```bash
# Set up remotes
wll remote add origin https://wll.example.com/org/project
wll remote add upstream https://wll.example.com/upstream/project

# Pull latest changes
wll pull upstream main

# Push your branch
wll push origin feature/my-change
```

### Repository Integrity Audit

```bash
# Verify receipt chain integrity
wll verify

# Full state replay from genesis
wll replay

# Low-level storage integrity
wll fsck

# View complete audit trail
wll audit

# Trace impact of a specific change
wll impact abc123de
```

### Routine Maintenance

```bash
# Clean up unreachable objects
wll gc

# Optimize storage
wll repack

# Full integrity check
wll fsck
```

---

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success. The command completed without errors. |
| `1` | General error. The command failed due to an operational error (e.g., file not found, invalid argument, merge conflict). |
| `2` | Usage error. Invalid command-line syntax or missing required arguments. Typically produced by the argument parser. |
| `101` | Integrity failure. A verification, replay, or fsck command detected corruption or inconsistency. |
| `128` | Fatal internal error. An unexpected panic or unrecoverable condition. |

---

## Environment Variables

| Variable | Description |
|----------|-------------|
| `WLL_DIR` | Override the repository directory. When set, WLL uses this path instead of searching for a `.wll` directory in the current or parent directories. |
| `WLL_CONFIG` | Path to a global configuration file. Overrides the default location. |
| `WLL_LOG` | Set the logging level for tracing output. Accepts `trace`, `debug`, `info`, `warn`, `error`. Requires `--verbose` to take effect. |
| `WLL_AUTHOR_NAME` | Override the author name for commitments. Takes precedence over `user.name` in configuration. |
| `WLL_AUTHOR_EMAIL` | Override the author email for commitments. Takes precedence over `user.email` in configuration. |
| `WLL_COMPRESSION` | Override the compression algorithm. Accepted values: `zstd`, `none`. |
| `NO_COLOR` | When set to any non-empty value, disables colored terminal output. Follows the [NO_COLOR](https://no-color.org/) convention. |

---

## Output Formats

WLL supports two output formats, selectable via the global `--format` option.

### Text (default)

Human-readable colored terminal output. Uses ANSI escape codes for syntax highlighting (receipt hashes in yellow, branches in green, status indicators with color). Color output is automatically disabled when stdout is not a terminal or when `NO_COLOR` is set.

### JSON

Machine-readable JSON output suitable for scripting, CI/CD pipelines, and tool integration. Every command that produces output will emit a JSON object or array.

Example (`wll --format json status`):

```json
{
  "branch": "main",
  "worldline": "wl:a1b2c3d4e5f6a7b8",
  "receipt_chain": {
    "count": 12,
    "integrity": "valid"
  },
  "staged": [],
  "unstaged": [],
  "untracked": []
}
```

Example (`wll --format json log -n 2`):

```json
[
  {
    "sequence": 3,
    "hash": "f4e2a1b8c9d0e3f2",
    "class": "PolicyChange",
    "intent": "Update access control policy",
    "decision": "Accepted"
  },
  {
    "sequence": 2,
    "hash": "c9d8e7f6a5b4c3d2",
    "class": "ContentUpdate",
    "intent": "Add error handling to API layer",
    "decision": "Accepted"
  }
]
```

Example (`wll --format json verify`):

```json
{
  "result": "pass",
  "checks": {
    "hash_chain": "valid",
    "sequences": "monotonic",
    "outcomes": "attributed",
    "snapshots": "anchored"
  }
}
```
