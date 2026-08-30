---
title: How to check an existing repository
description: Check a repository locally, run the full GitHub-backed audit, and hand findings to another tool.
order: 1
---

Use this guide when Ordnung is on your PATH and you want to check a repository you already have.

## Check from the repository root

```sh
ordnung check .
```

Ordnung inventories the repository, resolves its effective policy, and prints one line per
finding. Use the stable check identifier in each line when filtering, discussing, or tracking a
finding. A path works too — `ordnung check ../path/to/repository` — and nothing is written either
way.

## Narrow or widen the report

Show only the findings that gate the exit code:

```sh
ordnung check . --severity required
```

Or include the checks the effective policy has switched off — they still run, they are just
hidden by default:

```sh
ordnung check . --all
```

## Run the full audit

A local check ends by counting the GitHub-backed checks it could not run. Branch protection,
secret scanning, and workflow permissions live in repository settings, so auditing them takes an
API call through an authenticated [`gh` CLI](https://cli.github.com/):

```sh
ordnung repo-check . --repo owner/name
```

The result is one report covering both the working tree and the GitHub settings.

## Apply the exact fixes

`fix` plans only changes it can make without guessing, and shows the complete plan before
anything is written:

```sh
ordnung fix .            # show the plan
ordnung fix . --apply    # carry it out
```

## Capture machine-readable findings

Use JSON when another program will consume the result:

```sh
ordnung check . --json > ordnung-findings.json
```

Every `--json` response uses the same envelope; see the [CLI reference](/reference/cli) for it,
and [The Ordnung model](/explanation/model) for how inventory and effective policy become
findings.
