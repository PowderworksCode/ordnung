---
title: How to check an existing repository
description: Evaluate a repository locally and capture its findings for another tool.
order: 1
---

Use this guide when Ordnung is installed and you want to evaluate a repository without changing it.

## Check from the repository root

Change into the repository and run:

```sh
ordnung check .
```

Ordnung inventories the repository, resolves its effective policy, and prints one line per finding.
Use the stable check identifier in each line when filtering, discussing, or tracking a finding.

## Check from another directory

Pass the repository path explicitly:

```sh
ordnung check ../path/to/repository
```

## Capture machine-readable findings

Use JSON when another program will consume the result:

```sh
ordnung check . --json > ordnung-findings.json
```

The check remains read-only. See the [CLI reference](/reference/cli) for the command shape and
[The Ordnung model](/explanation/model) for how inventory and effective policy produce findings.
