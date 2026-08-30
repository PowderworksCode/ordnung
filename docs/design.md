# Ordnung Design

Status: implementation contract for the work leading to `v0.1.0`.

## Purpose

Ordnung determines whether a GitHub repository is in order. It recursively
inventories the repository, identifies the projects that live at each path,
derives the structural and operational expectations that apply to them, checks
GitHub repository settings, and produces deterministic findings and fixes.

Ordnung also distributes canonical configuration from one fleet configuration
repository to an explicit list of member repositories. File changes are grouped
into one reviewable pull request per member. GitHub settings that cannot be
represented in a pull request may be changed directly when apply mode is
explicitly enabled.

The first release supports Rust, TypeScript and JavaScript, static sites built
from that ecosystem, GitHub Actions, and repository-wide quality policy.

## Principles

1. Inventory once. Every check consumes the same typed repository model.
2. Checking is read-only. Mutation requires an explicit apply operation.
3. Detection is evidence-based. A project records why it was classified.
4. Unknown is not pass. Missing access or unsupported structure is surfaced.
5. Fleet policy is the default, not an unbreakable law. Local exceptions must
   be permitted centrally, requested explicitly, and carry a reason.
6. Managed paths have precise ownership. Plans show every create, update, and
   delete before anything changes.
7. Core logic is independent of terminals, GitHub, databases, and web servers.
8. Program behavior and source semantics are out of scope. Bounded structural
   checks may inspect test placement, comment markers, and tool configuration
   without judging implementation. Manifests, workflows, configuration, and
   layout are repository structure and may be parsed.

## Non-goals

- Source-code linting, syntax analysis, program-behavior analysis, or prose
  judgment.
- Coverage measurement or coverage dashboards.
- Repository scaffolding.
- A hosted control plane in the initial implementation.
- Languages outside Rust and the TypeScript/JavaScript ecosystem before
  `v0.1.0`.

A hosted service may later use the same core as a private or paid control plane.
It must not require policy logic to move out of `ordnung-core`.

## Workspace

```text
ordnung/
  crates/
    ordnung-core/
      src/inventory.rs  compatibility façade over Entl inventories
      src/profile.rs  re-exported Entl profile API
      src/            config, plans, and registries
      src/checks/     one module per check
      tests/          core integration tests
    ordnung-cli/
      src/            terminal and gh process adapters
      tests/          CLI and adapter integration tests
  docs/design.md
```

Future adapters may add a GitHub client, Action packaging, or server without
changing the inventory, policy, finding, or change-plan contracts.

## Operating Modes

### Standalone

`ordnung inspect` and `ordnung check` operate on one local repository. An
optional `.ordnung/overrides.toml` is authoritative because no fleet policy is present.
This includes monorepositories: every detected project receives its applicable
checks.

### Fleet

A central `.ordnung/fleet.toml` names every repository. The fleet runner obtains
each member's tree and GitHub facts, audits it, resolves allowed local overrides,
and computes one complete remediation plan.

A fleet configuration repository contains policy and canonical files. It is one
consumer of Ordnung, not configuration built into the tool. Ordnung ships policy
tiers under `confs/`, but they are consumed through the same inheritance
mechanism any third-party configuration uses, so no tier is privileged.

## Configuration Layout

Every Ordnung configuration file for a repository lives in a `.ordnung`
directory, which holds up to three distinct roles:

| File | Role |
| --- | --- |
| `fleet.toml` | A fleet instance: members plus the policy applied to them. |
| `policy.toml` | A reusable policy library. Declares no members. |
| `overrides.toml` | A member repository's local exceptions. |

Managed sources live under `.ordnung/managed/` and resolve against the directory
that declares them. The directory is therefore the unit of publication: an
inherited layer is fetched whole and is self-contained, so resolution needs no
separate repository-root concept.

A configuration repository that is also a member of its own fleet holds
`fleet.toml` and `overrides.toml` side by side without collision.

## Policy Layers

A fleet instance or a policy library may inherit other layers:

```toml
[[extends]]
path = "../../other-conf/.ordnung"
```

```toml
[[extends]]
git = "https://github.com/owner/repo"
rev = "0000000000000000000000000000000000000000"
path = "confs/recommended"
```

With `git`, `path` selects a directory inside the fetched repository, so one
repository can publish several tiers. Without it, the repository's own `.ordnung`
directory is used. A fetched revision is cached by URL and revision; because the
revision is pinned the cache is immutable and reused without network access.

Git references must name a full 40-character commit revision. A moving reference
cannot produce a deterministic plan, and an inherited layer can write files into
every member repository, so the pin is the reviewable boundary of that trust.

`extends` resolves depth first, so a layer always appears after everything it
inherits and the layer nearest the fleet wins. Cycles are rejected.

**Members are never inherited.** `extends` must reference a `policy.toml`;
pointing it at a directory containing `fleet.toml` is an error. Inheriting members
would silently enrol another fleet's repositories.

Merge semantics:

- Check severities merge per check ID, and GitHub settings per field, with the
  importing layer winning.
- Managed entries and dependency requirements merge by `name`. Reusing a name
  replaces the inherited entry; that is the only way to override one.
- `allow_override` governs member repositories only. A layer that extends another
  may redefine anything it inherits, because importing was a choice it can unmake.

### Shipped Tiers

| Tier | Intent |
| --- | --- |
| built-in defaults | The floor. Close to industry consensus, so a repository with no configuration gets actionable output. |
| `confs/recommended` | Stricter practices most teams would accept. Mandates no specific linter. |
| `confs/paranoid` | Every check on, including specific tools and Ordnung's own conventions. Extends `recommended`. |

Each tier extends the one below it, so a tier file is the difference between
tiers rather than a restatement of the whole check list.

### Repository Stage

A member declares how much it owes the people who use it:

```toml
[[member]]
repo = "owner/project"
stage = "incubating"
```

`supported` is the default, so omitting the field changes nothing. `incubating`
means the repository is still finding its shape: committing straight to the default
branch is how the work happens, and there are no consumers yet owed a changelog,
a licence, or a stable commit convention.

The stage is the one axis nothing can detect. Visibility, archived state, language,
ecosystem, and project shape are already facts, and letting a fleet declare those by
hand would let the declaration drift from reality. Whether a repository is *intended*
to be supported is not a property of its contents.

It is assigned by the fleet rather than requested by the member, so graduating is a
reviewable change in one file rather than something a repository grants itself.

A layer supplies severity deltas per stage:

```toml
[policy.stages.incubating.checks]
branch-protection = { severity = "off", allow_override = false }
changelog = { severity = "off", allow_override = true }
```

The overlay applies after every inherited layer merges, so a stage relaxation holds
even where a tier escalated the same check. Unknown stage names are configuration
errors.

A stage relaxes ceremony only. Hygiene and security are never part of it: a secret
committed to an incubating repository is just as leaked, and an unpinned Action is
just as exploitable. `lockfiles`, `gitignore`, `pinned-actions`,
`workflow-permissions`, `secret-scanning`, `builds`, `typecheck`, and `ci-exists`
stay exactly where they were.

### Archived Repositories

`archived` is a GitHub fact, so it is never declared. GitHub refuses writes to an
archived repository and Ordnung refuses to open a pull request against one, which
makes every finding against it unactionable. Both check runners therefore
short-circuit: each applicable check reports one `skip` explaining the state, and
the report is clean. Reporting the state once is more useful than reporting what
cannot be fixed.

## Repository Inventory

The inventory is a graph rooted at one repository. `PackageInstance` nodes name
their package root, manifest, ecosystem, base language, optional workspace root,
lockfile owner, discovered lockfile, and evidence. Directory-level `Project`
nodes aggregate additive language IDs, ecosystem IDs, capability IDs, and
evidence. These axes stay separate: a TypeScript project may use the Bun
ecosystem and expose the static-site capability, while a Rust project may expose
the Cargo-workspace capability.

Initial evidence includes:

- `Cargo.toml`, including `[workspace]` and `[package]` distinctions.
- `package.json` and the lockfile in the same directory.
- `tsconfig.json` and TypeScript dependency declarations.
- Vite, Next.js, Astro, SvelteKit, and similar static-site build signals.
- `.github/workflows/*.yml` and `*.yaml` at repository scope.

The filesystem is walked once by `entl-codebase` into a deterministic
`CodebaseTree` before manifest and relationship discovery. The walker uses
gitignore semantics, includes hidden configuration such as `.github`, skips
dependency, vendored, and generated trees, and never follows symlinks. Ordnung
filters hidden package and project roots from its policy inventory. Repository
`.gitignore` files participate even outside a Git checkout. Additional
repository-relative gitignore-style exclusions may be declared in
`.ordnung/overrides.toml`.

The reusable discovery API returns an Entl `CodebaseInventory`; it does not
depend on Ordnung configuration or repository policy facts. `entl-github`
derives a `GithubInventory` from that completed inventory without walking the
tree again. It also supplies the typed remote repository facts populated by
Ordnung's `gh` adapter. Ordnung keeps a small compatibility façade containing
the local models, while settings policy and mutation planning remain in
`ordnung-core`.
Workspace paths are normalized to forward-slash form before glob matching so
detection behaves the same on Unix and Windows.

Workspace relationships are structural facts, not path guesses. Cargo
`members`, `exclude`, and explicit `package.workspace` links are resolved.
`package.json` workspaces and pnpm workspace inclusions/exclusions are resolved.
Every package names its lockfile owner, so checks do not reconstruct workspace
membership. Conflicting lockfiles, unsupported `packageManager` declarations,
invalid workspace patterns, and malformed manifests become inventory issues.
Project-relative managed configuration applies once to every matching project
root.

## Language Profiles

Language-specific discovery knowledge lives in Entl's typed distributed
registry. A language profile owns its stable ID, display name, detection and
source extensions, config and dependency signals, syntax, and any superseded
language.

Reusable language expectations live in each Entl `LanguageProfile` as an
optional typed `LanguageConventions` field. They have no separate registry or
string join. Conventions own inline-test indicators, mirrored-test defaults,
and required type-layer configuration. Ordnung decides whether and how those
defaults are enforced. Rust, JavaScript, and TypeScript are the
initial supported Ordnung policy languages; Entl may inventory additional
languages without enabling Ordnung checks for them.

Package-manager facts live in a separate ecosystem registry. An ecosystem
profile owns its stable ID, roles, manifest, lockfile alternatives, selector
files, and zero or more implied languages. The initial ecosystem profiles are
`cargo`, `bun`, `pnpm`, `yarn`, and `npm`. TypeScript is intentionally a language overlay rather
than a package manager; a TypeScript package therefore has both `javascript`
and `typescript` language IDs. For source-tree checks, the TypeScript profile
supersedes the JavaScript profile so the same files are not checked twice.

Entl discovery and Ordnung checks iterate the Entl profile registries.
Discovery uses typed profile references internally and stores IDs only in its
serializable result. Checks
must ask package instances, profiles, and conventions for lockfile ownership,
extensions and required configuration instead of branching on language or
package-manager names. `entl-github` tool profiles classify workflow commands and
expanded package scripts into typed test, lint, format, typecheck, and build
tasks. Ordnung checks decide which task kinds each policy requires. Project capabilities, such as
`cargo-workspace` and `static-site`, remain independent because they describe
project shape rather than language identity.

Ordnung re-exports Entl's registry types and sorted accessors from `profile.rs`.
Every built-in declaration remains colocated in Entl:

```text
entl-codebase/src/profiles/
  languages/{rust,javascript,typescript,...}.rs
  ecosystems/{cargo,bun,npm,pnpm,yarn}.rs
entl-github/src/profiles/
  {rust,javascript}.rs
```

`language_profiles()` and `ecosystem_profiles()` collect linked registrations,
reject duplicate IDs, and sort by stable ID before exposing them. Consumers
therefore receive deterministic data even though distributed `inventory`
registration order is unspecified. Entl's registry macro is re-exported as
`ordnung_core::profile::registry`, so a linked downstream crate can submit an
additional profile without editing a central list.

Relationships between profiles are typed references, not ID joins. For example,
Cargo's `EcosystemProfile.implied_languages` contains `&rust::PROFILE`, and TypeScript's
`supersedes` collection contains `&javascript::PROFILE`. Registry initialization
also rejects references to profiles that were not registered. String IDs remain
only at configuration, lookup, and serialization boundaries.

Manifest ambiguity is explicit through `ManifestSelection`. Bun, pnpm, and Yarn
may be selected by a same-directory lockfile, selector file, or supported
`packageManager` declaration. `Default` entries are selected when a manifest
exists without recognized manager evidence: Cargo for `Cargo.toml`, and npm for
`package.json`. Selecting the default allows the lockfile check to report the
expected missing lockfile rather than failing to identify the ecosystem.

## Findings and Checks

A check returns a stable identifier, status, severity, scope, explanation, and
optional remediation. Status is one of `pass`, `fail`, `skip`, or `error`.
Severity is `required`, `recommended`, or `off`.

Checks use a typed distributed registry built with the Rust `inventory` crate.
Each `src/checks/<check>.rs` module owns one `CheckDefinition`: its stable ID,
default severity, engineering category, agent-facing instructions, and optional
repository or GitHub runner. Every currently registered check has at least one
runner. There is no separate check-ID list or instruction catalog to keep
synchronized.

`check_definitions()` collects linked registrations, rejects duplicate IDs, and
sorts by stable ID. Policy defaults, configuration validation, local execution,
GitHub execution, and agent instruction rendering all consume this registry.

`skip` always explains why a check does not apply. `error` means the check could
not reach a trustworthy verdict. Required failures and errors produce a nonzero
CLI exit code. Demoting a check therefore also demotes its errors: an unreadable
GitHub setting on an advisory check does not by itself make a report unclean.

A check's `default_severity` is the position Ordnung takes with no configuration
at all, so it is reserved for what is close to industry consensus and actionable.
A specific tool mandate, an Ordnung-invented convention, a contested practice, or
a claim that depends on context the repository cannot express belongs in a shipped
tier instead. A default that fires on a legitimate choice teaches its reader to
ignore output.

Where one check would otherwise bundle a consensus rule with a preference, it is
split so each half carries its own severity: `pinned-actions` and
`pinned-dependencies`, `readme` and `readme-quality`, `test-inline` and
`test-mirror`. Splitting also makes each position expressible in fleet policy,
because a severity travels through policy layers while a boolean on a
repository-local config does not.

Each check also declares a `CheckScope`. A repository-scoped check has one verdict
per repository: there is one README, one default branch, one Dependabot
configuration. A project-scoped check reports per project root, so a monorepository
receives one finding per directory. Any check reading GitHub facts is necessarily
repository-scoped.

The scope is declared rather than inferred because policy that selects directories
can only apply to project-scoped checks. Without the declaration, a path-scoped rule
aimed at `branch-protection` would silently do nothing, which is the worst failure
mode for a policy file that looks authoritative.

Checks read either repository inventory, parsed repository files, GitHub facts,
or a declared combination. GitHub access and filesystem access are supplied as
interfaces to the core.

## Agent Instructions

`ordnung instructions <repo>` emits a concise Markdown summary of effective
repository rules. It includes detected language/ecosystem/capability profiles,
enabled required and recommended checks, ignore paths, active external-test
layout, desired GitHub settings, approved exceptions with reasons, and effective
fleet-owned paths.

Checks are grouped by engineering intent. Each exact policy ID is paired with
the plain-language guidance colocated with that check's definition. The block
states that configured policy is the target state even when a roadmap check
does not yet have mechanical enforcement.

The output is ordinary text, not an agent skill. `--write AGENTS.md` and
`--write CLAUDE.md` inject the same generated block between stable HTML comment
markers. Existing text outside that block is preserved, repeated generation is
idempotent, and malformed duplicate markers are errors. Fleet-aware generation
requires both the fleet manifest and explicit member name:

```sh
ordnung instructions . \
  --fleet ../conf/.ordnung/fleet.toml \
  --repo PowderworksCode/ordnung \
  --write AGENTS.md --write CLAUDE.md
```

The generated text is deterministic and carries no timestamps, so teams may
commit it and review policy changes as normal diffs.

## Code Generation

Repositories with committed generated output declare each generator locally:

```toml
[[codegen]]
name = "bindings"
root = "crates/bindings"
command = "bun run generate"
outputs = ["src/generated/**"]
```

`root` is repository-relative and may be omitted for the repository root. A
declaration is healthy only when the normalized command runs for that project
in a GitHub Actions job and a later command in the same job runs
`git diff --exit-code` or `git diff --quiet`. Output patterns document the
committed ownership surface for agents and future remediation.

## Dependency Updates

`entl-github` parses `.github/dependabot.yml` or
`.github/dependabot.yaml` into typed update entries. Dependabot ecosystem
profiles are inventory registrations linked directly to `entl-codebase`
ecosystem profiles, so Cargo, Bun, npm, pnpm, and Yarn support cannot drift
through misspelled string joins. Bun uses the native `bun` package ecosystem;
an existing `npm` entry remains accepted for compatibility. pnpm and Yarn use
Dependabot's `npm` package ecosystem.

The `dependabot` check requires coverage at each dependency-owning location.
Node packages are graded at their manifest directory, while Cargo workspace
members are graded once at their shared lockfile owner. Repositories with
GitHub Actions workflows also require a `github-actions` update at `/`.
`directory` and `directories` entries may use explicit one-level `/*` and
recursive `/**` patterns.

The local runner grades configuration syntax and directory coverage. The
GitHub runner separately reads vulnerability-alert and automated-security-fix
settings through `gh`; a known disabled setting fails, while a setting hidden
by token permissions is reported as unavailable rather than disabled.

## Dependency Pins

Entl dependency facts retain the declared requirement and distinguish registry,
Git, local-path, and workspace sources. Each ecosystem profile explicitly owns
its pin syntax and whether floating requirements are blocking or advisory.
Ordnung does not reparse manifests or maintain ecosystem-name conditionals.

Two separate claims live here, so they are two checks.

`pinned-actions` is required by default. `entl-github` emits every workflow Action
reference with its pin classification. Local and Docker actions are exempt,
40-character commit SHAs are pinned, and the explicit `stable` and `oldstable`
channels are allowed. All other tags, branches, and missing references fail. A
mutable tag lets an upstream owner change what runs in this repository's CI, which
makes this a supply-chain boundary rather than a preference.

`pinned-dependencies` is advisory by default. It requires exact three-component
semver declarations for npm, Bun, pnpm, and Yarn dependencies. Local and workspace
dependencies are exempt, and peer dependency compatibility ranges are not graded.
Cargo registry ranges and unpinned Git sources are reported as advisory because
`Cargo.lock` owns the resolved build; an `=` requirement or 40-character Git
revision is pinned. The committed lockfile already fixes what gets installed, and
exact requirements work against automated dependency updates, so this is a
position a fleet takes rather than a default.

## Required Dependencies

A policy layer may require packages of every project in a language or ecosystem,
so tooling that reasons about installed libraries can rely on them existing:

```toml
[[dependency]]
name = "rust-refactoring"
language = "rust"
require = ["itertools"]
```

Package names belong to one registry, so an entry must select a language, an
ecosystem, or both. `kind` restricts which dependency kind satisfies the
requirement; any kind satisfies it by default. Entries merge by `name` and accept
`state = "unmanaged"`, exactly like managed entries. `state = "absent"` is rejected:
removing a dependency is never safe to infer, because other code may use it.

Selectors match a discovered package, which carries exactly one language and one
ecosystem. A package's own language comes from its manifest, so a Node or Bun
package reports `javascript` even when the project around it is TypeScript. Both
its manifest language and the languages of the project rooted at the same path are
accepted, otherwise `language = "typescript"` would silently match nothing.

A workspace root is a synthesized aggregation entry with no manifest dependencies
of its own and is skipped. Workspace members resolve their own inherited
declarations, so nothing is borrowed from the root.

`required-dependencies` reports what is missing and never edits a manifest. Adding
a dependency means choosing a version and updating a lockfile, which cannot be
resolved deterministically without network access.

## Workflow Token Permissions

The repository-level default `GITHUB_TOKEN` permission is a typed GitHub fact.
The `workflow-permissions` check requires the default to be `read` and requires
`can_approve_pull_request_reviews` to be false. This keeps ambient workflow
authority minimal; a workflow or job that needs write access declares a narrow
`permissions:` block in its own YAML.

This is a GitHub runner check because the effective default may be inherited
from organization or enterprise settings and cannot be established from local
workflow files. When the authenticated token lacks administration-read access,
the check skips with the API reason instead of treating invisibility as a safe
configuration.

## CI Scoping and Retry Masking

Entl tool profiles declare whether their CI workload is heavy and may declare
typed retry arguments and configuration signatures. Cargo, Tauri, native addon
builds, system package installation, and container builds are heavy; JavaScript
bundling and linting remain light. Workflow facts retain pull-request path
filters plus each job's condition, dependencies, and output-producing role.

`ci-scoped` requires a heavy pull-request job to be covered by workflow path
filters, a job condition, or a dependency on an output-producing fanout job.
`test-retry-masking` rejects retry arguments and positive retry counts in the
registered Rust and TypeScript test-runner configurations. Ordnung consumes
those profiles and does not maintain its own tool-name table.

## Stylesheet Linting

Stylelint is a typed Entl tool linked directly to CSS, SCSS, Sass, and Less
language profiles. Its conventional configuration filenames and
`package.json` key live on the same tool profile. The `stylelint` check assigns
stylesheets to the nearest JavaScript package, accepts an applicable ancestor
configuration, and requires a typed Stylelint task on pushes or pull requests.

## Auto-Merge and Ruleset Bypass

The `allow-auto-merge` GitHub runner compares the observed repository setting
with the resolved standalone or fleet GitHub policy. An unmanaged setting is
skipped; it is not assigned an implicit desired value. Existing settings sync
uses the same resolved policy and remains the mutation path.

`dependabot-automerge` applies only when the effective policy explicitly sets
`github.allow_auto_merge = true`. It requires an active pull-request workflow
gated to `dependabot[bot]` that runs `dependabot/fetch-metadata`, excludes major
updates, and invokes GitHub auto-merge. The repository setting must be enabled
and the default branch must have required status checks, so auto-merge cannot
turn into an immediate unguarded merge.

The GitHub adapter reads active branch rulesets and retains their rule types
and bypass actors as typed facts. `ruleset-bypass` applies only to rulesets with
pull-request or required-status-check gates. Every such ruleset needs at least
one explicit bypass actor; classic branch protection is outside this check.

The `stale` GitHub runner fails for pull requests idle longer than 30 days,
branches already merged into the default branch, or disabled automatic branch
deletion after merge. The adapter reads at most 100 open pull requests and 100
branches and compares at most 20 non-default branches. Any truncation is stated
in the finding rather than implying a complete scan.

## Ignored Build Outputs

Build-junk expectations live on `entl-codebase` ecosystem profiles through
`gitignore_patterns`. Cargo currently requires `target/`; Bun, npm, pnpm, and
Yarn require `node_modules/`. Ordnung does not maintain a parallel ecosystem
table.

The `gitignore` check grades each package at the location where its build junk
would appear. Cargo workspace members share their lockfile owner's target
directory, while JavaScript packages are checked at each manifest directory.
The matcher applies every `.gitignore` from the repository root through that
scope using Git semantics, including anchored rules and negation. A nested
`.gitignore` may therefore provide or override coverage for its subtree.

## Code Ownership

`entl-github` recognizes CODEOWNERS at `.github/CODEOWNERS`, `CODEOWNERS`,
and `docs/CODEOWNERS`, using GitHub's precedence order when more than one is
present. The selected file is parsed into typed rules containing the source
line, pattern, and owners. Unsupported negation and character-range syntax,
invalid owner tokens, unreadable files, and files above GitHub's 3 MB limit
produce diagnostics scoped to CODEOWNERS rather than workflow diagnostics.

Ownerless rules remain in the inventory because GitHub uses them to clear
ownership for a path. The `codeowners` check nevertheless requires at least
one rule that assigns an `@account`, `@organization/team`, or email owner.
Lower-priority CODEOWNERS files are retained as facts and reported as shadowed,
but their presence is not itself a policy failure.

## Repository Scripts

Entl classifies shell files by registered Shell language identity, including
standard shell extensions and extensionless files with shell shebangs. Ordnung
retains those paths as inventory facts; the `scripts` check does not perform a
second recursive filesystem walk.

By default, shell scripts live under `scripts/`, `scripts/dev.sh` stands up the
development environment, and the root README names `scripts/dev.sh`. Hidden
directories and configured generated or vendored directory names are excluded.
Intentional exceptions are exact repository-relative paths rather than
ambiguous basenames:

```toml
[scripts]
directory = "scripts"
development = "dev.sh"
allow = ["install.sh"]
```

The directory, development entry, allowlist, and ignored directory names are
validated when `.ordnung/overrides.toml` is loaded. Custom development entries still have
to classify as Shell.

## Conventional Commits

`entl-github` owns a registered conventional-commit enforcer catalog. It
recognizes semantic pull-request title actions and commit-message tools such as
commitlint, cocogitto, and convco from parsed workflow steps and normalized
commands. Each enforcement fact retains its target, workflow, job, step, and
enforcer profile. A custom shell validator counts only when it reads the GitHub
pull-request title, contains the conventional type prefix declared by its
profile, invokes a validator, and has an explicit failure path. Merely naming a
workflow or job "conventional" does not count.

The `conventional-commits` check requires at least one enforcement fact and a
mention of Conventional Commits in the root `README.md`, `README`, or
`CONTRIBUTING.md`. Existing commit history is not graded retroactively; the
enforcement point establishes the convention for future merges.

## README Floor

Both checks select a root file whose stem is `README`, using inventory path order
to make selection deterministic.

`readme` is required by default and is the floor: the file exists and carries an
H1 title within the first ten nonblank lines.

`readme-quality` is advisory by default and applies Ordnung's definition of a good
README: between 150 and 1,500 whitespace-delimited words, headings for installation
or setup, usage or documentation, contributing, and license information, and no
broken repository-relative links. This is a documentation-size guardrail, not an
assessment of prose style, and the length band and expected sections are a house
style rather than a universal one. It skips when no README exists, because absence
is the floor check's finding and reporting it twice adds no signal.

Ordnung parses the document as GitHub-flavored Markdown and derives headings,
links, and images from parser events. Repository-relative destinations are
checked against the existing inventory after query strings and fragments are
removed. Anchor-only and URI destinations are ignored; absolute paths and
paths that escape the repository fail. A destination may name either an exact
file or a directory containing inventoried files.

## Git Hooks

`git-hooks` checks that a repository *provides* hooks and wires them up. It never
checks that hooks are active: `core.hooksPath` and `.git/hooks` are local machine
state rather than repository content, and a fleet audit reads a fresh clone where
the setting is always unset. "Are hooks installed here?" is not a question the
repository can answer, so it is not asked.

What is structural, and therefore graded:

- Hooks are committed under `.githooks` and named for a client-side Git hook. Other
  files there are documentation or helpers.
- Every hook file is executable. Git silently ignores a hook without the execute
  bit, which is the worst way for a gate to fail: it looks present and never runs.
- The configured development script sets `core.hooksPath`, so a fresh clone gets
  the hooks rather than a README instruction to run a command by hand.

A declared hook manager — Husky, Lefthook, pre-commit, `simple-git-hooks`,
`cargo-husky` — passes instead. Those install through their own lifecycle, so
requiring the development script to repeat that would be wrong.

The check is off by default and required by the `paranoid` tier, alongside the other
tool mandates. Its intent is that the fast half of the CI gate also runs before a
commit lands. Verifying that the hook and CI run the *same* tasks is a further step
that needs the workflows to exist first.

## GitHub Action Marketplace

For a public repository with a root `action.yml` or `action.yaml`,
`action-badge` derives the Marketplace slug from the action's parsed name and
requires the root README selected by GitHub to link that exact Marketplace URL.
Private repositories and repositories that do not publish a root action skip.
An invalid action manifest or unavailable repository content is an error.

## Agent Field Guide

The recommended `field-guide` check requires an exact `field_guide.md` filename
somewhere in the inventoried repository. `notes/field_guide.md` is the preferred
location, but repositories may place it where their documentation structure
makes sense. Ignored, generated, and vendored paths do not satisfy the check.

The check's generated agent instruction tells agents to find and read the field
guide at the start of work, then append concise, durable discoveries that will
help future agents. The file is shared operational memory, not a task backlog or
a replacement for maintained user and architecture documentation.

## Website Reachability

The `website` check uses the repository homepage configured in GitHub settings
as its sole source of truth. The GitHub runner requires that metadata to be
nonempty and probes that URL. Local README links do not influence the check.

Requests follow redirects and have a ten-second total timeout. A final 2xx
response passes. Definite HTTP failures and malformed URLs are findings;
transport, DNS, TLS, and timeout failures are errors because Ordnung could not
reach a trustworthy verdict.

## License Presence

The `license` repository runner accepts `LICENSE`, `LICENSE.md`, `LICENSE.txt`,
`COPYING`, or `UNLICENSE` at the repository root, in that priority order. A
similarly named file in a nested project does not satisfy the repository-level
requirement.

The GitHub runner reports a detected SPDX identifier when available. Missing or
`NOASSERTION` classification is a skip rather than a failure because valid
custom license text may not match GitHub's classifier; the repository runner
remains authoritative for file presence.

## Changelog Presence

The `changelog` check requires one root `CHANGELOG.md`, `CHANGELOG`,
`CHANGELOG.txt`, `CHANGES.md`, or `HISTORY.md`, in that priority order. It does
not prescribe release headings, versions, dates, or entry format.

## Text Corralling

`stray-files` checks root Markdown and text files only. Conventional community
files and exact `stray_files.allow` entries are accepted; working notes belong
under `stray_files.notes`. `TODO` collection belongs to a dedicated external tool.

## Prose Tooling

Codespell and Vale are registered language-independent tools in `entl-codebase`.
`entl-github` links their known GitHub Actions to those profiles and emits typed
workflow-tool invocations for both actions and shell commands. The checks require
the respective tool to run from a push or pull-request workflow.

Vale additionally requires a root `.vale.ini`. A nonempty global `StylesPath`
must be a safe repository-relative path represented in the inventory.

## Check Roadmap

The phases below record the implemented check surface. Both phases remain
required for the `v0.1.0` release.

### Foundation Phase

- `project-inventory`
- `branch-protection`
- `required-checks`
- `strict-status-checks`
- `ci-exists`
- `ci-continue-on-error`
- `ci-scheduled-run`
- `ci-job-timeout`
- `reproducible-toolchain`
- `ci-green`
- `typecheck`
- `builds`
- `artifacts-built`
- `codegen-drift`
- `dependabot`
- `secret-scanning`
- `workflow-permissions`
- `lockfiles`
- `gitignore`
- `codeowners`
- `scripts`
- `conventional-commits`
- `readme`
- `field-guide`
- `website`
- `license`
- `changelog`
- `repo-meta`

### Completion Phase

- `auto-update-pr-branches`
- `ruleset-bypass`
- `ci-scoped`
- `pinned-actions`
- `pinned-dependencies` (advisory by default)
- `test-retry-masking`
- `test-inline` (off by default)
- `test-mirror` (off by default)
- `required-dependencies`
- `readme-quality` (advisory by default)
- `stray-files`
- `stylelint`
- `vale`
- `codespell`
- `action-badge`
- `allow-auto-merge`
- `dependabot-automerge`
- `stale`

## Configuration

### Standalone Configuration

A repository's `.ordnung/overrides.toml` may define ignores and complete check policy:

```toml
ignore = ["experiments/**"]

[checks.website]
severity = "off"

[ci_exists]
ignore = ["spikes/**"]
```

`ci_exists.ignore` exempts matching project instances only from CI task grading.
The project remains visible to every other repository check, and a language is
still graded when any non-exempt project uses it.

### Fleet Configuration

The fleet manifest uses an explicit repository list, and may inherit policy layers:

```toml
name = "powderworks"

[[extends]]
path = "../../ordnung/confs/paranoid"

[[member]]
repo = "owner/project"

[policy.checks.website]
severity = "required"
allow_override = true
```

Fleet mode does not merge arbitrary local values over fleet values. A member
requests a permitted exception under `overrides`:

```toml
[overrides.website]
severity = "off"
reason = "internal monorepo with no public site"
```

An override fails configuration validation when the fleet did not permit that
check to be overridden or the local declaration has no reason. In standalone
mode, `[checks]` is authoritative and `[overrides]` is unnecessary.

Standalone repository-level GitHub settings are also explicit:

```toml
[github]
delete_branch_on_merge = true
allow_update_branch = true
```

Fleet GitHub setting policy carries a desired value and independently controls
whether a member may request an exception:

```toml
[policy.github.allow_auto_merge]
value = false
allow_override = true
```

A permitted member exception must use the dedicated override namespace and
include its reason:

```toml
[github_overrides.allow_auto_merge]
value = true
reason = "dependency updates may merge after required checks pass"
```

Fleet members cannot set `[github]` directly, and standalone repositories cannot
use `[github_overrides]`.

The optional external-test layout checks are enabled through normal check policy.
They are two independent positions, so each carries its own severity:

```toml
[checks.test-inline]
severity = "required"

[checks.test-mirror]
severity = "required"

[test_layout]
ignore = ["src/generated/**"]

[test_layout.rust]
source_roots = ["src"]
test_root = "tests"
test_suffixes = [""]

[test_layout.typescript]
source_roots = ["src"]
test_root = "tests"
test_suffixes = [".test", ".spec"]
```

`test-inline` flags language-specific inline-test indicators in configured source
roots. Rust's inline `#[cfg(test)]` module is idiomatic, so requiring its absence
is a position rather than a consensus, and the check is off by default.

`test-mirror` requires a corresponding file under the configured test root. The
relative directory is preserved and a configured suffix is inserted before the
test file extension. One test file per source file is a considerably stronger claim
than keeping tests out of source files: it fires on entry points, module roots, and
files already covered by a shared suite. It is off by default for that reason.

Both share layout resolution, path validation, source collection, and `ignore`
filtering; only the verdict differs.

Unknown keys, unknown checks, malformed repository names, duplicate managed
ownership, unresolvable inherited layers, unpinned Git references, `extends` cycles,
and unsafe paths are errors rather than ignored input.

## Managed Configuration

A managed entry describes exact desired state. No templates or substitutions
are supported initially.

```toml
[[managed]]
name = "biome-base"
source = "managed/biome/base.json"
destination = "biome.base.json"
relative_to = "project"
when = { language = "typescript" }
```

`relative_to = "repo"` addresses one path from repository root.
`relative_to = "project"` applies the destination to every detected project
matching all fields in `when`. Selectors may use `language`, `ecosystem`, and
`capability` independently or together. For example, either of these selectors
may appear on a managed entry:

```toml
when = { language = "typescript" }
```

```toml
when = { ecosystem = "cargo", capability = "cargo-workspace" }
```

A source file owns one destination file and requires byte-for-byte equality. A
source directory owns the complete destination subtree: files missing or stale
are written, and extra files are deleted. Local extension files must live
outside an owned subtree.

Removing a managed declaration stops managing that destination and does not
delete it. Removing a declared source is a fleet configuration error. Intentional
fleet-wide deletion uses an explicit tombstone:

```toml
[[managed]]
name = "retire-old-config"
destination = ".old-tool.toml"
state = "absent"
relative_to = "repo"
```

Tombstones remain until the fleet no longer needs the absence enforced.

`state` has three values, and the distinction between the last two matters:

| State | Meaning |
| --- | --- |
| `present` | The destination must hold exactly the source content. |
| `absent` | The destination must not exist, and is deleted from every member. |
| `unmanaged` | Stop inheriting this entry. No member file is touched. |

`absent` is an assertion that deletes; `unmanaged` only drops an inherited
declaration. Opting out of an upstream entry must not silently delete files across
a fleet, which is why they cannot share one keyword. Declaring `unmanaged` for a
name nothing inherited is an error, so a typo cannot quietly do nothing.

Between taking an inherited entry whole and dropping it sits a third option.
Reusing a name *without* a `source` refines the inherited entry rather than
replacing it: the content, the layer it comes from, and its destination all stay
put, and the reusing layer supplies only `only` and `when`.

```toml
[[managed]]
# The tier's workflow, on public members alone.
name = "fleet-lint-workflow"
only = ["owner/one", "owner/two"]
```

This exists so that narrowing an entry's audience does not require copying the
entry's content down a layer to say so. A copy would be a second thing to keep in
step with the original, and the whole point of inheriting the entry was to avoid
having one. A refinement that names a destination other than the inherited one is
an error, as is a refinement of a name no layer declares, as is one that narrows
nothing — an entry that reuses a name and supplies neither `only` nor `when` has
almost certainly lost the list it meant to carry, and treating it as an override
would quietly widen the file's audience to every member.

Ownership is exclusive: exactly one entry owns a destination, which the planner
relies on. Within a layer, overlapping destinations are an error. Across layers,
reusing an entry's `name` replaces it, and that is the only sanctioned override.
Two differently named entries that resolve to the same destination remain an error
even from different layers, because a silent cross-layer collision would mean
editing a file in one configuration and receiving content from a repository the
author never opened. Managed entry names are consequently the public interface of
a published configuration, and renaming one is a breaking change for importers.

Managed entries may target all members or an explicit member subset. Project
selectors are evaluated from inventory, not duplicated path lists.

## Planning and Mutation

An audit and an apply operation use the same immutable plan:

1. Inventory the target tree.
2. Load and validate configuration.
3. Fetch GitHub facts where required.
4. Run all applicable checks.
5. Resolve fixes into file and GitHub-setting changes.
6. Display or serialize the complete plan.
7. In apply mode, make idempotent GitHub setting changes and open or update one
   consolidated file-change pull request for the member.
8. Re-audit after changes land.

Fleet drift remains a failure until the member's default branch matches desired
state. An open remediation pull request does not convert failure into success.

Direct GitHub mutation is allowed only in explicit apply mode. Every setting
adapter must support current-state reads, desired-state comparison, and an
idempotent write. File changes are never pushed directly to a default branch.

## CLI

The initial command surface is intentionally small:

```text
ordnung inspect [PATH] [--json]
ordnung check [PATH] [--json]
ordnung repo-check [PATH] --repo OWNER/NAME [--json]
ordnung fix [PATH] [--apply] [--json]
ordnung github inspect OWNER/REPO [--json]
ordnung github check OWNER/REPO [--repo-root PATH] [--json]
ordnung github sync-settings OWNER/REPO [--repo-root PATH] [--apply] [--json]
ordnung fleet check FLEET_TOML [--json]
ordnung fleet sync FLEET_TOML --repo OWNER/NAME --repo-root PATH [--apply] [--json]
ordnung fleet github-check FLEET_TOML [--json]
ordnung fleet github-sync-settings FLEET_TOML --repo OWNER/NAME [--apply] [--json]
ordnung fleet github-sync FLEET_TOML --repo OWNER/NAME [--apply] [--json]
ordnung fleet github-sync-all FLEET_TOML [--apply] [--json]
```

`inspect` reports detected projects and evidence. `check` audits one local
repository. `repo-check` combines that local report with GitHub-backed checks
under one exit status. `fix` previews or applies exact check remediations
without guessing.
`fleet check` validates fleet policy and canonical sources before touching
members. The local form of `fleet sync` exercises deterministic change planning.
The GitHub adapter shells out to `gh` to inspect and audit repository settings,
clone the default branch for structural inventory, and materialize file plans on
one reusable branch and pull request per member.
`fleet github-sync-all` processes every explicit member, continues after an
individual member error, and reports one member outcome in the fleet result.
Member drift yields exit code `1`; any member operational error yields `2` after
all members have been attempted.

JSON responses have a stable top-level envelope containing `schema_version`,
`command`, `ok`, and `data`. Exit code `0` means clean or successfully applied
local state, `1` means policy drift, and `2` means a configuration or operational
error.

## Authentication and GitHub Integration

Standalone local inspection requires no GitHub credentials. Initial GitHub
commands use the active `gh` authentication or the token recognized by `gh`.
A single-repository Action can use its scoped workflow token. Central unattended
fleet reads and writes should eventually use installation tokens from a
fleet-owned GitHub App.

The GitHub adapter belongs outside the policy engine. It supplies repository
trees and settings, applies direct setting changes, and materializes the core's
file plan as one branch and pull request per member.

## Security

- Repository-relative paths cannot be absolute or contain `..` traversal.
- Managed sources must remain under the fleet configuration root.
- Directory ownership and every proposed deletion are explicit in the plan.
- Symlinks are never followed during inventory or managed-directory expansion.
- External commands use argument arrays, controlled working directories, and no
  shell interpolation.
- GitHub permissions are least-privilege and missing access is reported.
- Logs and serialized plans never contain tokens, private keys, or managed file
  contents unless explicitly requested for a local diff.

## Testing

The core is exercised with filesystem fixtures and fake GitHub fact providers.
Required coverage includes nested Cargo workspaces, mixed Rust/TypeScript
repositories, static sites below the root, ignored/generated trees, override
authorization, project-relative managed files, directory mirror deletion,
tombstones, idempotent plans, unsafe-path rejection, multi-level policy
inheritance, and fetching a pinned layer from a real Git repository.

Integration tests exercise the CLI against temporary repositories. GitHub tests
use a dedicated installation and disposable repositories; unit tests never need
network access.

## `v0.1.0` Exit Criteria

- Both check-roadmap phases are implemented and tested.
- Rust, TypeScript/JavaScript, and static-site inventories are trustworthy in
  nested repositories.
- Standalone and fleet policy resolution is complete, including inherited policy
  layers, the shipped tiers, and their merge and override semantics.
- Exact file, directory mirror, project-relative, and tombstone synchronization
  is complete.
- All file fixes produce one idempotent pull request per member.
- Supported GitHub setting fixes are idempotent and explicitly applied.
- CLI human and JSON output are stable enough for the Action contract.
- The GitHub Action and fleet authentication path are documented and exercised.
