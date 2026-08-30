---
title: How to set up a fleet
description: Create a fleet configuration naming member repositories and the policy they share.
order: 4
---

A fleet is a set of repositories governed from one place: a configuration repository holds a
`fleet.toml` naming every member, the policy they share, and the files Ordnung manages in them.
Use this guide to create one.

## Create the configuration repository

Make a repository for the fleet configuration. Its conventional shape is a `.ordnung/` directory
with `fleet.toml` inside:

```text
fleet-config/
└── .ordnung/
    ├── fleet.toml
    └── managed/
```

## Name the members

Start `fleet.toml` with a name and one entry per member repository:

```toml
name = "acme"

[[member]]
repo = "acme/api"

[[member]]
repo = "acme/website"
```

## State shared policy

A policy entry sets a check's severity for every member, and says whether a member may request an
exception:

```toml
[policy.checks.website]
severity = "required"
allow_override = true
```

With `allow_override = true`, a member opts out in its own `.ordnung/overrides.toml` under
`[overrides]`, and every exception must carry a reason — see the
[configuration reference](/reference/configuration) for the full resolution order.

Rather than stating every opinion yourself, a fleet can extend a
[shipped tier](/reference/configuration) and write only its differences:

```toml
[[extends]]
git = "https://github.com/PowderworksCode/ordnung"
rev = "<full 40-character commit revision>"
path = "confs/recommended"
```

The revision is pinned deliberately: an inherited layer can write files into every member, so a
moving reference would make plans non-deterministic.

## Manage a shared file

A `[[managed]]` entry distributes one file to every member. Sources resolve relative to the
directory `fleet.toml` sits in:

```toml
[[managed]]
name = "contributing"
source = "managed/CONTRIBUTING.md"
destination = "CONTRIBUTING.md"
```

## Validate the configuration

```sh
ordnung fleet check .ordnung/fleet.toml
```

```text
fleet: acme
members: 2
policies: 1
GitHub settings: 0
managed entries: 1
dependency requirements: 0
stages: supported 2
```

Unknown keys, unknown check identifiers, and unapproved overrides are configuration errors, so a
typo fails here instead of silently doing nothing.

## Audit the members

With the [`gh` CLI](https://cli.github.com/) authenticated, audit every member without changing
anything:

```sh
ordnung fleet github-check .ordnung/fleet.toml
```

When the report shows drift you want to repair, continue with
[How to synchronize a fleet member](/how-to-guides/sync-a-fleet-member).
