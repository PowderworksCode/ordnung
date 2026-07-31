# Agent Field Guide

Read this file before changing the repository. Add concise entries when work
reveals a durable constraint, non-obvious convention, or recurring failure mode
that would help a future agent. Keep temporary plans and task-specific notes out
of the field guide.

## Repository Conventions

- Rust integration tests live under each crate's `tests/` directory rather than
  beside implementation code.
- Each Ordnung check owns its registration, default severity, instructions, and
  runner in `crates/ordnung-core/src/checks/`.
- GitHub Actions workflows are intentionally disabled during initial
  development. Do not restore them merely to clear local Ordnung findings.
- Tool identities and codebase conventions belong in `entl-codebase`; GitHub
  workflow and remote repository facts belong in `entl-github`.
- Ordnung pins Entl by exact Git revision. For a cross-repository API change,
  use `scripts/local_dev` to verify moving sibling sources without lockfile
  churn. The command is the shared entry point for local dependency overrides.
  Publish Entl and update the revision only with explicit user approval.
- Do not create, amend, revert, or push a commit without explicit user approval.
