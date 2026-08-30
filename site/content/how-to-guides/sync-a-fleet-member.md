---
title: How to synchronize a fleet member
description: Preview and apply centrally managed configuration to one local repository.
order: 5
---

Use this guide when a repository belongs to a [configured fleet](/how-to-guides/set-up-a-fleet)
and you want to bring its managed files into line with central policy.

You need the fleet configuration, the member's `OWNER/NAME`, and a local checkout of that member.
The examples assume you are standing in the member checkout with the fleet configuration checked
out beside it.

## Validate the fleet configuration

```sh
ordnung fleet check ../fleet-config/.ordnung/fleet.toml
```

Resolve configuration errors before continuing.

## Preview the synchronization plan

```sh
ordnung fleet sync ../fleet-config/.ordnung/fleet.toml \
  --repo acme/api \
  --repo-root .
```

Without `--apply`, the command only displays the plan. Review every proposed file change before
continuing.

## Apply the reviewed plan

Run the same command with the explicit mutation flag:

```sh
ordnung fleet sync ../fleet-config/.ordnung/fleet.toml \
  --repo acme/api \
  --repo-root . \
  --apply
```

`fleet sync` writes files in the local working tree only. Inspect the working tree after the
command completes and review the resulting diff before committing.

For automation, append `--json`. See the [CLI reference](/reference/cli) for the complete fleet
command surface.

## Open or update the fleet pull request

To operate on the GitHub repository without a prepared member checkout, use:

```sh
ordnung fleet github-sync ../fleet-config/.ordnung/fleet.toml \
  --repo acme/api
```

Review the complete dry-run plan, then repeat it with `--apply`. Two things happen under that one
flag, and they are worth knowing precisely:

- Supported repository settings are **changed immediately**; only the file changes wait in a pull
  request for review.
- The pull request always uses a branch named `ordnung/remediation`, which Ordnung
  **force-pushes**, re-parented onto the current default branch. Commits anyone else pushes to
  that branch are discarded, and the name is not configurable.

Archived repositories are refused. The command continues to report drift until the pull request
lands and a later run observes a clean default branch.
