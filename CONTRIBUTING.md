# Contributing to WorldLine Ledger

Thank you for your interest in contributing to WorldLine Ledger (WLL). This document provides guidelines and information to make the contribution process smooth and effective for everyone involved.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Reporting Bugs](#reporting-bugs)
- [Suggesting Features](#suggesting-features)
- [Development Setup](#development-setup)
- [Architecture Overview](#architecture-overview)
- [Code Style](#code-style)
- [Testing Requirements](#testing-requirements)
- [Commit Message Conventions](#commit-message-conventions)
- [Pull Request Process](#pull-request-process)
- [License](#license)

## Code of Conduct

This project adheres to the [Contributor Covenant Code of Conduct](https://www.contributor-covenant.org/version/2/1/code_of_conduct/). By participating, you are expected to uphold this code. Please report unacceptable behavior to [dev@mapleai.org](mailto:dev@mapleai.org).

We are committed to providing a welcoming and inclusive experience for everyone, regardless of background or identity.

## Reporting Bugs

If you find a bug, please open an issue on the [GitHub issue tracker](https://github.com/mapleaiorg/wll/issues) with the following information:

1. **Summary** -- A clear and concise description of the bug.
2. **Environment** -- Your operating system, Rust toolchain version (`rustc --version`), and WLL version (`wll --version`).
3. **Steps to Reproduce** -- A minimal sequence of steps that reliably reproduces the issue.
4. **Expected Behavior** -- What you expected to happen.
5. **Actual Behavior** -- What actually happened, including any error messages or stack traces.
6. **Additional Context** -- Any other relevant information such as logs, screenshots, or related issues.

Please search existing issues before opening a new one to avoid duplicates.

## Suggesting Features

Feature requests are welcome. To suggest a new feature:

1. Open an issue on the [GitHub issue tracker](https://github.com/mapleaiorg/wll/issues) with the label `enhancement`.
2. Provide a clear description of the proposed feature and the problem it solves.
3. Explain the use case and who would benefit from the feature.
4. If possible, outline a proposed implementation approach.
5. Indicate whether you are willing to implement the feature yourself.

For significant changes, please open an issue for discussion before starting implementation work. This helps ensure your effort aligns with the project's direction and avoids duplicate work.

## Development Setup

### Prerequisites

- **Rust** 1.80 or later (install via [rustup](https://rustup.rs/))
- **Git** 2.x or later

### Getting Started

```bash
# Clone the repository
git clone https://github.com/mapleaiorg/wll.git
cd wll

# Build the entire workspace
cargo build --workspace

# Run all tests
cargo test --workspace

# Install the CLI locally for manual testing
cargo install --path crates/wll-cli
```

### Useful Development Commands

```bash
# Build in release mode
cargo build --workspace --release

# Run tests for a specific crate
cargo test -p wll-ledger
cargo test -p wll-crypto

# Run tests with output displayed
cargo test --workspace -- --nocapture

# Check for lint warnings
cargo clippy --workspace --all-targets -- -D warnings

# Format all code
cargo fmt --all

# Verify formatting without modifying files
cargo fmt --all -- --check

# Generate documentation
cargo doc --workspace --no-deps --open
```

## Architecture Overview

WLL is organized as **17 composable crates** across **6 layers**. Understanding this structure will help you find the right place for your contribution.

```
Layer              Crates                              Responsibility
---------------------------------------------------------------------------
Foundation         wll-types, wll-crypto, wll-store    Core types, hashing,
                                                       content-addressable storage

Core               wll-dag, wll-ledger, wll-fabric     Provenance DAG, receipt
                                                       chain, temporal ordering

Policy             wll-gate                             Commitment boundary,
                                                       policy pipeline

Workflow           wll-refs, wll-index, wll-diff,       Branches, staging,
                   wll-merge                            diffing, merging

Distribution       wll-pack, wll-sync, wll-protocol,   Packfiles, push/pull,
                   wll-server                           wire protocol, HTTP server

Application        wll-cli, wll-sdk                     CLI and high-level SDK
```

**Dependency flow is strictly top-down.** Lower layers must never depend on higher layers. If you are adding a new dependency between crates, verify that it respects this layering.

### Where to Make Changes

- **Adding a new core type?** Start in `wll-types`.
- **Changing hashing or signing behavior?** Look at `wll-crypto`.
- **Modifying the receipt chain or replay logic?** Work in `wll-ledger`.
- **Updating policy evaluation?** Modify `wll-gate`.
- **Changing CLI commands?** Update `wll-cli`.
- **Updating the public API surface?** Modify `wll-sdk` and ensure backward compatibility.

## Code Style

All code must conform to the project's style standards before it can be merged.

### Formatting

Run `cargo fmt --all` before committing. CI will reject any code that does not pass `cargo fmt --all -- --check`.

### Linting

Run `cargo clippy --workspace --all-targets -- -D warnings` and resolve all warnings. CI treats clippy warnings as errors.

### General Guidelines

- Write idiomatic Rust. Prefer the standard library and well-known ecosystem crates over custom implementations.
- Use `thiserror` for library error types and `anyhow` only in binary targets (`wll-cli`, `wll-server`).
- Prefer strong typing over stringly-typed interfaces. Use newtypes and enums where appropriate.
- Document all public items with `///` doc comments. Include examples for non-trivial functions.
- Keep functions focused and small. If a function exceeds roughly 50 lines, consider refactoring.
- Avoid `unsafe` code unless absolutely necessary. If `unsafe` is required, document the safety invariants with a `// SAFETY:` comment.
- Use `tracing` for logging instead of `println!` or `eprintln!`.

## Testing Requirements

WLL currently has **435 tests** across the workspace. All tests must pass before a pull request can be merged.

```bash
# Run the full test suite
cargo test --workspace
```

### What to Test

- **New features** must include unit tests that cover the primary use case and edge cases.
- **Bug fixes** must include a regression test that reproduces the bug before the fix and passes after.
- **Public API changes** must update or add integration tests in the relevant crate's `tests/` directory.

### Test Guidelines

- Use descriptive test names that explain the scenario: `test_receipt_chain_rejects_duplicate_sequence_number`.
- Use `proptest` for property-based testing where applicable, especially in `wll-crypto`, `wll-types`, and `wll-ledger`.
- Test both success and failure paths. Ensure that error types are correct and error messages are meaningful.
- Keep tests deterministic. Avoid reliance on wall-clock time, filesystem ordering, or network access.
- Use `tempfile` for any tests that require filesystem operations.

### Running Benchmarks

```bash
cargo bench --workspace
```

Benchmarks use `criterion`. If your change is performance-sensitive, include benchmark results in your pull request description.

## Commit Message Conventions

We follow a structured commit message format to maintain a clean and navigable history.

### Format

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Type

| Type       | Description                                      |
|------------|--------------------------------------------------|
| `feat`     | A new feature                                    |
| `fix`      | A bug fix                                        |
| `docs`     | Documentation changes only                       |
| `style`    | Code style changes (formatting, no logic change) |
| `refactor` | Code change that neither fixes a bug nor adds a feature |
| `perf`     | Performance improvement                          |
| `test`     | Adding or updating tests                         |
| `build`    | Changes to the build system or dependencies      |
| `ci`       | Changes to CI configuration                      |
| `chore`    | Other changes that do not modify source or tests |

### Scope

The scope should be the name of the affected crate without the `wll-` prefix. Examples: `types`, `ledger`, `gate`, `cli`, `sdk`.

For changes spanning multiple crates, use a descriptive scope like `workspace` or omit the scope.

### Examples

```
feat(ledger): add receipt chain compaction

Implement periodic compaction of the receipt chain to reduce
storage overhead for long-running repositories. Compaction
preserves all commitment receipts and collapses intermediate
outcome receipts into summary snapshots.

Closes #142
```

```
fix(crypto): handle empty input in blake3 domain separation

Previously, hashing an empty byte slice with domain separation
would panic due to an unchecked length assertion. This commit
adds a guard clause and a regression test.

Fixes #238
```

```
test(gate): add property tests for policy evaluation
```

### Rules

- The subject line must be at most 72 characters.
- Use the imperative mood in the subject line ("add", not "added" or "adds").
- Do not end the subject line with a period.
- Separate the subject from the body with a blank line.
- Reference related issues in the footer using `Closes #N` or `Fixes #N`.

## Pull Request Process

1. **Fork the repository** and create a feature branch from `main`:
   ```bash
   git checkout -b feat/my-feature main
   ```

2. **Make your changes** following the code style and testing guidelines above.

3. **Ensure all checks pass locally:**
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```

4. **Push your branch** and open a pull request against `main`.

5. **Fill out the PR template** with:
   - A clear description of what the PR does and why.
   - A summary of the changes made.
   - Instructions for testing or verifying the changes.
   - References to any related issues.

6. **Respond to review feedback.** Maintainers may request changes. Please address all comments and push additional commits to your branch.

7. **Merge.** Once approved and all CI checks pass, a maintainer will merge your PR. We use squash merges for single-purpose PRs and merge commits for multi-commit PRs that benefit from preserved history.

### PR Checklist

Before submitting, confirm:

- [ ] Code compiles without warnings (`cargo build --workspace`)
- [ ] All 435+ tests pass (`cargo test --workspace`)
- [ ] New code is formatted (`cargo fmt --all`)
- [ ] No clippy warnings (`cargo clippy --workspace --all-targets -- -D warnings`)
- [ ] New features include tests
- [ ] Bug fixes include a regression test
- [ ] Public API changes are documented
- [ ] Commit messages follow the conventions above

## License

By contributing to WorldLine Ledger, you agree that your contributions will be licensed under the [Apache License, Version 2.0](LICENSE).

All new files must include the following header comment:

```rust
// Copyright 2024 MapleAI
// SPDX-License-Identifier: Apache-2.0
```

---

Thank you for contributing to WorldLine Ledger. Your work helps build a more transparent and accountable foundation for version control.
