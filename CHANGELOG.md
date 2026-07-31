# Changelog

Notable changes to Ordnung are recorded here, newest first.

## 2026-07-31

- Added typed, conflict-checked remediation plans that combine exact check fixes,
  fleet-managed files, and supported GitHub setting changes.
- Added local `fix` and remote `fleet github-sync` commands.
- Added idempotent Git tree, branch, and pull-request materialization through
  authenticated `gh` calls.
- Added the composite GitHub Action and a versioned JSON and exit-code contract.
- Added aggregate `repo-check` and `fleet github-sync-all` modes for per-repository
  auditing and centralized fleet distribution.

## 2026-07-30

- Began the Rust implementation of repository and fleet policy checks.
- Added typed project, workflow, GitHub, and managed-configuration inventory.
- Added deterministic agent instructions and repository field guides.
- Added typed CI scoping, retry masking, Stylelint, auto-merge policy, and
  ruleset bypass checks.
- Completed the check roadmap with typed dependency pins, Marketplace Action
  links, guarded Dependabot auto-merge, and stale pull-request and branch facts.
- Moved TODO collection out of Ordnung while retaining notes and root-file
  corralling.
