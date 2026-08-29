---
title: Checks
description: Every check Ordnung carries, what it wants from a repository, and its default severity.
order: 2
---

Every finding Ordnung reports carries a stable check identifier from the list
on this page. The identifier is the name you use everywhere: filtering output,
setting a severity in `.ordnung/overrides.toml`, granting a fleet exception, or
discussing a finding in review.

Run `ordnung --list-checks` to see the checks your installed binary carries,
with their one-line instructions. That output comes from the binary, so it is
the authority if this page and your installed version ever disagree.

## How to read the tables

**Severity** is the default the built-in policy assigns; any layer of
[configuration](/reference/cli) may raise, lower, or switch a check off.

| severity | meaning |
| --- | --- |
| `required` | A failure gates the exit code. Ordnung considers this broken. |
| `recommended` | Reported, worth fixing, not a gate. |
| `off` | Opt-in. Runs only when a policy layer switches it on. |

**Scope** says what one finding covers. A `repository` check has one verdict
per repository — there is one README, one default branch, one Dependabot
configuration. A `project` check reports once per detected project root, so a
monorepository receives one finding per directory.

Some checks read the local working tree, some read GitHub repository settings
through the `gh` CLI, and some do both. `ordnung check` runs the local side;
`ordnung repo-check --repo owner/name` is the full audit.

## Repository shape

| check | severity | scope | what it wants |
| --- | --- | --- | --- |
| `codeowners` | recommended | repository | Keep a valid CODEOWNERS file in `.github/CODEOWNERS`, `CODEOWNERS`, or `docs/CODEOWNERS`, in GitHub precedence order, with at least one rule that assigns an @account, @organization/team, or email owner. |
| `conventional-commits` | recommended | repository | Enforce Conventional Commits in a pull-request or push workflow with a recognized semantic-title action, commitlint, cocogitto, convco, or an explicit failing PR-title validator; mention Conventional Commits in the root README or CONTRIBUTING.md. |
| `gitignore` | required | project | Ignore each ecosystem's build junk at every package scope: Cargo requires `target/` and Bun, npm, pnpm, and Yarn require `node_modules/`; applicable ancestor `.gitignore` files may provide the rule. |
| `project-inventory` | required | repository | Keep supported project boundaries and manifests detectable by Ordnung. |
| `repo-meta` | recommended | repository | Keep repository description and issue tracking configured. |
| `scripts` | recommended | repository | Keep detected shell scripts under the configured `scripts.directory`, except exact `scripts.allow` paths; provide the configured development script there and name its repository-relative path in the root README. |
| `stray-files` | off (opt-in) | repository | Keep root Markdown and text files conventional or explicitly listed in `stray_files.allow`; keep working notes under `stray_files.notes`. |

## Documentation and text

| check | severity | scope | what it wants |
| --- | --- | --- | --- |
| `action-badge` | off (opt-in) | repository | For a public repository that publishes a root GitHub Action, link its exact Marketplace listing from the root README. |
| `changelog` | recommended | repository | Keep a root CHANGELOG.md, CHANGELOG, CHANGELOG.txt, CHANGES.md, or HISTORY.md; format and versioning style are repository choices. |
| `codespell` | recommended | repository | Run Codespell from a push or pull-request workflow using its command or registered GitHub Action. |
| `field-guide` | off (opt-in) | repository | At the start of work, find and read `field_guide.md`; append concise, durable discoveries that will help future agents. Keep the file in the repository, preferably at `notes/field_guide.md`. |
| `license` | recommended | repository | Keep a root LICENSE, LICENSE.md, LICENSE.txt, COPYING, or UNLICENSE file; GitHub SPDX classification is useful but nonstandard license text is allowed. |
| `readme` | required | repository | Keep a root README that opens with an H1 title in its first ten nonblank lines. |
| `readme-quality` | recommended | repository | Keep the root README between 150 and 1,500 words with install/getting-started, usage/docs, contributing, and license sections, and no broken repository-relative Markdown links. |
| `vale` | off (opt-in) | repository | Keep a root `.vale.ini` with an existing relative StylesPath when declared, and run Vale from a push or pull-request workflow. |
| `website` | off (opt-in) | repository | Keep the repository's GitHub homepage setting pointed at its reachable HTTP(S) website. |

## GitHub safeguards

These read repository settings through the `gh` CLI, so they report in
`ordnung repo-check` and `ordnung github check` rather than in a plain local
`ordnung check`.

| check | severity | scope | what it wants |
| --- | --- | --- | --- |
| `allow-auto-merge` | recommended | repository | Keep GitHub auto-merge equal to the effective `github.allow_auto_merge` policy; an unmanaged setting is left alone. |
| `auto-update-pr-branches` | recommended | repository | Allow and automate pull-request branch updates when strict checks require freshness. |
| `branch-protection` | required | repository | Require pull requests and block force pushes and deletion on the default branch. |
| `required-checks` | recommended | repository | Require every check posted by pull-request workflows before default-branch changes merge. |
| `ruleset-bypass` | recommended | repository | Give every active branch ruleset that gates merging at least one explicit bypass actor for emergency administration. |
| `secret-scanning` | required | repository | Keep secret scanning and push protection enabled where available. |
| `strict-status-checks` | recommended | repository | Require status checks to run against the latest default-branch state. |

## CI safety

| check | severity | scope | what it wants |
| --- | --- | --- | --- |
| `ci-continue-on-error` | required | repository | Do not let jobs or gating test, lint, format, typecheck, and build steps hide failures with `continue-on-error`. |
| `ci-exists` | required | project | Keep a push or pull-request workflow with test, lint, and format tasks for every detected language; exempt scratch project paths explicitly with `ci_exists.ignore`. |
| `ci-green` | required | repository | Keep latest default-branch runs green for active repository-owned workflows. |
| `ci-job-timeout` | recommended | repository | Give every push and pull-request CI job an explicit finite timeout; reusable-workflow jobs are exempt because GitHub does not allow the setting there. |
| `ci-matrix-scoped` | recommended | repository | Let every pull-request matrix job short-circuit: scope the workflow with path filters, condition the job, or expand the matrix from a fanout job that inspects the change. |
| `ci-scheduled-run` | recommended | repository | Run validation on a schedule when periodic coverage should expose repository bitrot between changes. |
| `ci-scoped` | recommended | repository | Gate heavy pull-request jobs with workflow path filters, a job condition, or a dependency on an output-producing fanout job. |
| `git-hooks` | off (opt-in) | repository | Commit the repository's Git hooks under `.githooks`, keep every hook file executable, and have the development script point `core.hooksPath` at it; a declared hook manager installs itself instead. |
| `zizmor` | off (opt-in) | repository | Run [zizmor](https://zizmor.sh) static analysis over the repository's GitHub Actions workflows from a push or pull-request workflow, using its command or the zizmor-action. |
| `test-retry-masking` | required | repository | Do not configure test commands or standard Rust and TypeScript test-runner configuration to rerun failures until they pass. |
| `workflow-permissions` | required | repository | Keep the repository's default `GITHUB_TOKEN` read-only and prevent workflows from approving pull requests; jobs that need write access must grant it explicitly with a permissions block. |

## Build and toolchain

| check | severity | scope | what it wants |
| --- | --- | --- | --- |
| `artifacts-built` | recommended | project | Build every detected binary, site bundle, napi-rs addon, and Tauri application in GitHub Actions; run full Tauri builds on a scheduled workflow. |
| `builds` | required | project | Run every declared `build`, `build:*` or `*:build` package target on push or pull requests; Tauri projects also need a change-triggered compile check and a scheduled full build. |
| `codegen-drift` | recommended | project | Declare each committed generator under `[[codegen]]` with its project root, command, and output patterns; run it in CI and follow it in the same job with `git diff --exit-code` or `git diff --quiet`. |
| `hawk` | off (opt-in) | repository | In Rust repositories, run [Astral's hawk](https://github.com/astral-sh/hawk) (`cargo hawk`) from a push or pull-request workflow to flag unnecessarily public APIs. |
| `reproducible-toolchain` | required | repository | Keep GitHub setup-action toolchain inputs off unbounded latest and wildcard versions; explicit versions and bounded stable channels are allowed. |
| `shellcheck` | off (opt-in) | repository | When the repository carries shell scripts, run [ShellCheck](https://www.shellcheck.net) over them from a push or pull-request workflow using its command or a registered GitHub Action. |
| `stylelint` | off (opt-in) | project | For each package containing CSS, SCSS, Sass, or Less, keep a Stylelint configuration in that package or an ancestor and run Stylelint on pushes or pull requests. |
| `test-inline` | off (opt-in) | project | Keep tests out of source files; move an inline test module under the configured test root. |
| `test-mirror` | off (opt-in) | project | Give every source file a mirrored test file under the configured test root, matching its path and a configured test suffix. |
| `typecheck` | required | project | Keep JavaScript and TypeScript projects on an explicit type layer and run their typechecker on push or pull requests. |

## Dependencies

| check | severity | scope | what it wants |
| --- | --- | --- | --- |
| `dependabot` | required | repository | Keep a valid `.github/dependabot.yml` version 2 configuration with a scheduled update covering every detected package ecosystem at its owning directory and GitHub Actions at the repository root; directory globs may be used explicitly. |
| `dependabot-automerge` | recommended | repository | When `github.allow_auto_merge` is explicitly enabled, use a Dependabot-only pull-request workflow that fetches update metadata, excludes major updates, and enables auto-merge behind required checks. |
| `lockfiles` | required | project | Commit the correct lockfile for every detected package ecosystem. |
| `pinned-actions` | required | repository | Reference third-party GitHub Actions by commit SHA; local actions are exempt and first-party release channels are allowed. |
| `pinned-dependencies` | recommended | project | Use exact npm/Bun dependency versions; local dependencies are exempt and Cargo ranges stay advisory because Cargo.lock owns resolution. |
| `required-dependencies` | required | project | Declare every package the effective policy requires for a project's language or ecosystem; a workspace member may inherit the declaration from its workspace root. |

## Maintenance automation

| check | severity | scope | what it wants |
| --- | --- | --- | --- |
| `stale` | recommended | repository | Keep open pull requests active within 30 days, remove branches already merged into the default branch, and enable automatic branch deletion after merge. |

## Changing a severity

Outside a fleet, a repository sets check severities directly in
`.ordnung/overrides.toml`:

```toml
[checks]
codespell = { severity = "off" }
test-mirror = { severity = "required" }
```

Inside a fleet, a member requests an exception under `[overrides]` instead, and
every exception must carry a reason. Unknown check IDs are an error at every
layer, so a typo fails loudly instead of silently doing nothing.
