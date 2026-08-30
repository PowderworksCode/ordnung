---
title: How to use the GitHub Action
description: Run repository checks or fleet remediation through the packaged Ordnung Action.
order: 2
---

Use this guide to run Ordnung in CI. The Action wraps the same binary the CLI installs, and every
mode maps to a CLI command; the [Action reference](/reference/action) lists every input and
output.

## Check one repository

The default mode, `repo-check`, returns one result covering both the checked-out repository and
its GitHub settings, using the workflow's own scoped token:

```yaml
name: Ordnung
on:
  pull_request:
  push:
    branches: [main]

permissions:
  actions: read
  contents: read
  pull-requests: read
  security-events: read

jobs:
  ordnung:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
      - uses: PowderworksCode/ordnung@<full-commit-sha>
```

Reference third-party Actions — this one included — by full commit SHA, with the tag as a
comment; Ordnung's own `pinned-actions` check requires exactly that. Pinned to a release tag
instead, the Action downloads that release's binary and verifies its checksum; a branch or SHA
ref builds from source with `cargo install --locked`, which takes minutes. Until the first binary
release ships, every ref builds from source.

The Action passes its `github-token` input to `gh` as `GH_TOKEN`. Use `mode: check` only when a
local-only audit is intentional.

## Remediate a fleet centrally

For central fleet remediation, check out the fleet configuration and use `mode: fleet-sync-all`
with `apply: true`. The credential must be able to read each member, update supported repository
settings, push `ordnung/remediation`, and create pull requests:

```yaml
permissions:
  contents: read

steps:
  - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
  - uses: PowderworksCode/ordnung@<full-commit-sha>
    with:
      mode: fleet-sync-all
      fleet: fleet.toml
      apply: true
      github-token: ${{ secrets.FLEET_GITHUB_TOKEN }}
```

A repository-scoped workflow token is not sufficient here: cross-repository fleet writes need a
token with access to every target. Applying changes repository settings immediately and
force-pushes each member's `ordnung/remediation` branch — read
[How to synchronize a fleet member](/how-to-guides/sync-a-fleet-member) before enabling it.

Use `mode: fleet-sync` with `repository: OWNER/NAME` to target one member for a retry or dry run.

## Act on the outcome

The Action outputs `outcome` as `clean`, `drift`, or `error`, plus the numeric `exit-code`. A
`drift` outcome still fails the step, because the exit code passes through; use
`continue-on-error` when you want to inspect the outputs instead of failing.
