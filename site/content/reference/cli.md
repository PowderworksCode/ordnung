---
title: CLI reference
description: Commands, arguments, and options for the Ordnung command-line application.
order: 1
---

Ordnung uses the following top-level command structure:

```text
ordnung <COMMAND>
```

Use `ordnung <COMMAND> --help` for help corresponding to the installed version.

## Listing checks

```sh
ordnung --list-checks [--json]
```

Prints every check the binary carries, grouped by category, with its default
severity, scope, and instructions. With `--json` it emits the check manifest —
the same document the [checks reference](/reference/checks) is tested against.

## Repository commands

```sh
ordnung inspect [PATH] [--json]
ordnung check [PATH] [--all] [--severity LEVEL] [--json]
ordnung repo-check [PATH] --repo OWNER/NAME [--all] [--severity LEVEL] [--json]
ordnung fix [PATH] [--apply] [--json]
ordnung instructions [PATH] [--write FILE]...
```

- `inspect` prints the read-only inventory: detected projects, packages,
  workspaces, languages, and the evidence behind each classification.
- `check` evaluates the inventory against the effective policy and prints one
  line per finding. It runs the local checks only, and ends by counting the
  GitHub-backed checks it could not run.
- `repo-check` runs the local and GitHub-backed checks as one repository
  audit; it needs an authenticated `gh` CLI.
- `fix` plans exact, non-guessing repository fixes, and applies them only with
  `--apply`.
- `instructions` prints concise repository rules for coding agents; each
  `--write` injects them into a marker-delimited region of that
  repository-relative Markdown file, leaving everything outside the markers
  alone.

Two options narrow or widen any check report. `--severity required|recommended|off`
reports findings at that severity or above without changing the verdict;
`--all` includes the checks the effective policy has switched off, which run
either way but are hidden by default.

`check` and `instructions` also accept `--fleet FLEET_TOML` with
`--repo OWNER/NAME`, evaluating one repository under centralized fleet policy.

## Fleet commands

```sh
ordnung fleet check FLEET_TOML [--json]
ordnung fleet sync FLEET_TOML --repo OWNER/NAME --repo-root PATH [--apply] [--json]
ordnung fleet github-check FLEET_TOML [--json]
ordnung fleet github-sync-settings FLEET_TOML --repo OWNER/NAME [--apply] [--json]
ordnung fleet github-sync FLEET_TOML --repo OWNER/NAME [--apply] [--json]
ordnung fleet github-sync-all FLEET_TOML [--apply] [--json]
```

`fleet check` validates the manifest. `fleet sync` writes managed files into
one member's local checkout. The `github-*` commands audit or remediate
members through the GitHub API: `github-sync-settings` covers repository
settings only, `github-sync` also opens or updates the member's consolidated
remediation pull request, and `github-sync-all` does that for every member.

## GitHub commands

```sh
ordnung github inspect OWNER/REPO [--json]
ordnung github check OWNER/REPO [--repo-root PATH] [--all] [--severity LEVEL] [--json]
ordnung github sync-settings OWNER/REPO [--repo-root PATH] [--apply] [--json]
```

The standalone equivalents of the fleet `github-*` commands, for one
repository outside any fleet.

Commands that can mutate state remain dry-run operations unless `--apply` is
supplied explicitly.

## Machine-readable output

Every `--json` response has the same envelope:

```json
{
  "schema_version": 1,
  "command": "check",
  "ok": false,
  "data": {}
}
```

The command-specific payload is always under `data`. Exit code `0` means clean or successfully
applied local state, `1` means policy drift, and `2` means an operational or configuration error.
