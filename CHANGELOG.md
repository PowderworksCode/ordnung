# Changelog

Notable changes to Ordnung are recorded here, newest first.

## Unreleased

- Checks the effective policy has switched `off` are no longer reported. They
  still run, so raising one to `required` needs no other change, but a first run
  against a repository now shows what Ordnung enforces rather than everything it
  has an opinion about. On Ordnung's own repository this is 32 lines of output
  instead of 115, and 8 findings instead of 84. Pass `--all` to `check`,
  `repo-check`, or `github check` to see them. Exit codes are unchanged: only
  `required` findings ever gated those.

## 2026-08-01

- Added the `git-hooks` check: hooks committed under `.githooks`, every hook file
  executable, and the development script setting `core.hooksPath`. A declared hook
  manager passes instead, because it installs through its own lifecycle. Whether
  hooks are active on a given machine is local state and is deliberately not graded.

- Added a member `stage` of `incubating` or `supported`, with per-stage severity
  deltas under `[policy.stages.<name>.checks]`. The stage is assigned by the fleet,
  not requested by the member, so graduating is a reviewable change in one file. It
  relaxes pull-request governance, consumer-facing documentation, and team process;
  it never relaxes hygiene or security.
- Archived repositories now report one `skip` per check explaining the state instead
  of failures nobody can act on, because GitHub refuses writes to them and Ordnung
  refuses to open a pull request against one.
- Every check now declares a `CheckScope` of `repository` or `project`, so policy
  that selects directories can be validated instead of silently misfiring.

- Split `test-layout` into `test-inline`, which keeps tests out of source files,
  and `test-mirror`, which requires a mirrored test file per source file. They were
  previously two booleans on one check's repository-local configuration, which fleet
  policy could not carry; as separate checks each is a severity a fleet distributes.
  Both are off by default and both are required by the `paranoid` tier.
- Removed `test_layout.scan_inline` and `test_layout.require_mirror`, which the
  split supersedes. `test_layout.ignore` and the per-language roots are unchanged.

- Rebalanced built-in check defaults toward an industry floor: 17 required, 21
  recommended, 7 off, where 36 of 43 were previously required. Specific linter
  mandates (Vale, Stylelint), Ordnung-specific conventions (`field-guide`,
  `stray-files`), contested practices (`stale`, `strict-status-checks`,
  `ruleset-bypass`), and context-dependent files (`license`, `changelog`,
  `codeowners`) no longer fail a run by default.
- Split `pinned-versions` into `pinned-actions`, which is required because a
  mutable Action tag lets an upstream owner change what runs in CI, and
  `pinned-dependencies`, which is advisory because the lockfile already fixes
  resolution and exact requirements work against automated updates.
- Split `readme` into `readme`, which requires a root README with an early H1,
  and `readme-quality`, which judges length, sections, and relative links against
  Ordnung's definition of a good README and is advisory by default.

- Added the `required-dependencies` check and `[[dependency]]` policy. A fleet or
  a repository can require packages for every project of a language or ecosystem,
  so automated tooling can rely on those libraries being available. Requirements
  inherit and override exactly like managed entries.
- Moved every Ordnung configuration file into a `.ordnung` directory. A member
  repository's `ordnung.toml` is now `.ordnung/overrides.toml`, and a fleet's
  `fleet.toml` and `managed/` sources live in `.ordnung/`.
- Added composable configuration: `[[extends]]` inherits a policy library by local
  path or by pinned Git revision. Members are never inherited, so importing a
  layer cannot enrol another fleet's repositories.
- With `git`, `path` selects a directory inside the fetched repository, so one
  repository can publish several policy tiers.
- Added shipped policy tiers under `confs/`: `recommended` for stricter practices
  that mandate no specific linter, and `paranoid` for everything on, including
  specific tools and Ordnung's own conventions. `paranoid` extends `recommended`,
  so each tier is the difference from the one below it. Both are consumed through
  the same `[[extends]]` mechanism a third party would use.
- Added downstream override semantics. Check severities and GitHub settings
  merge with the importing configuration winning; managed entries merge by name.
  `allow_override` continues to govern member repositories only.
- Added the `unmanaged` managed state, which drops an inherited entry without
  deleting files. `absent` remains an assertion that deletes.
- Cross-layer destination collisions between differently named entries are
  rejected, so name-keyed replacement is the single way to override an entry.

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
