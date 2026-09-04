---
title: The GitHub Action
description: Inputs, outputs, and binary resolution for the packaged Ordnung Action.
order: 4
---

Ordnung ships as a composite Action, which is the distribution channel most
users touch. `action.yml` maps inputs to environment variables and runs
`scripts/action.sh`.

```yaml
- uses: PowderworksCode/ordnung@<pinned-sha-or-tag>
  with:
    mode: repo-check
    repository: ${{ github.repository }}
```

## Inputs

| Input | Default | Meaning |
| --- | --- | --- |
| `mode` | `repo-check` | One of `repo-check`, `check`, `fix`, `github-check`, `fleet-check`, `fleet-sync`, `fleet-sync-all` |
| `path` | `.` | Repository path for local inspection |
| `fleet` | none | Path to `fleet.toml`, required by the fleet modes |
| `repository` | `${{ github.repository }}` | `owner/name`, required by the GitHub-backed modes |
| `apply` | `false` | Apply exact fixes, or open/update the remediation pull request |
| `format` | `human` | `human` or `json` |
| `github-token` | `${{ github.token }}` | Token `gh` authenticates with |

`apply` and `format` are validated: anything else exits `2`.

## Outputs

| Output | Values |
| --- | --- |
| `outcome` | `clean`, `drift`, or `error` |
| `exit-code` | Ordnung's process exit code |

`outcome` is derived from the exit code: `0` is `clean`, `1` is `drift`, and
anything else is `error`. A `drift` result still fails the step, because the
exit code is passed through. Use `continue-on-error` if you want to inspect the
outputs instead of failing.

## How it finds a binary

In order:

1. **`ORDNUNG_BIN`**, if set, is used as-is. This is the escape hatch for a
   binary you have already installed or built.
2. **A published release binary**, when the Action is pinned to a tag matching
   `vMAJOR.MINOR.PATCH` and a release asset exists for the runner's platform. The
   asset is downloaded with `gh`, and its `.sha256` is verified before it runs; a
   mismatch falls through rather than executing the file. The release is fetched
   from the repository the Action came from, which is not the `repository` input
   being audited.
3. **A source build**, `cargo install --locked`. This takes minutes.

Every failure in step 2 is recoverable and reports why on stderr, so pinning to
a branch or SHA, running on a platform with no published binary, or hitting a
missing asset all degrade to a source build rather than failing.

Published platforms are `x86_64-unknown-linux-musl`,
`aarch64-unknown-linux-musl`, `x86_64-apple-darwin`, and `aarch64-apple-darwin`.
Anything else builds from source. These names must match the release archives
exactly: a triple that names no asset does not fail, it silently builds from
source on every run.

## Releasing

[`.github/workflows/release.yml`](https://github.com/PowderworksCode/ordnung/blob/main/.github/workflows/release.yml) runs only on a pushed `v*` tag. It drafts the
release, builds and uploads one archive and checksum per platform, and publishes
the release only once every platform job has succeeded, so a run that dies
halfway leaves a draft rather than a release advertising binaries that do not
exist.

## What is tested

[`tests/cli.rs`](https://github.com/PowderworksCode/ordnung/blob/main/crates/ordnung-cli/tests/cli.rs) drives `scripts/action.sh` directly for argument mapping, the
default mode, input validation, and all three binary-resolution paths including
a deliberately corrupted download. The `action` job in `ci.yml` runs the Action
itself twice: against a generated fixture repository it finds clean, and against
this repository, asserting the outcome and exit code each time.

The `fix`, `github-check`, `fleet-check`, and `fleet-sync` modes are not covered
end to end; they need GitHub credentials or a fleet manifest.
