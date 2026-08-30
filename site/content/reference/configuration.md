---
title: Configuration
description: Every configuration layer, key, and override mechanism, from a single repository to a fleet.
order: 3
---

Configuration is optional. Ordnung's built-in defaults are meant to be useful
against a repository with no configuration file at all.

## Repository-level configuration

A repository's own settings live in `.ordnung/overrides.toml`.

Outside a fleet, a repository sets check severities directly:

```toml
[checks]
codespell = { severity = "off" }
test-mirror = { severity = "required" }
```

Inside a fleet, `[checks]` is refused — a member requests an exception under
`[overrides]` instead, and every exception must carry a reason:

```toml
[overrides]
codespell = { severity = "off", reason = "vendored corpus has intentional typos" }
```

An override is rejected unless the fleet policy that governs the check sets
`allow_override = true`. This is deliberate: it makes an exception a negotiated,
documented act rather than a silent local edit.

## Policy resolution order

Policy resolves in layers, each overriding the one before it:

1. Ordnung's built-in defaults
2. Inherited configuration, in `[[extends]]` order
3. Fleet policy for the specific repository
4. Local `[overrides]`, if the layer above permits them

Unknown check IDs are an error at every layer, so a typo fails loudly instead of
silently doing nothing.

## Shipped tiers

Ordnung ships policy tiers under `confs/`, each consumed through the same
`[[extends]]` mechanism a third-party configuration uses, so nothing about
publishing a configuration is special-cased for Ordnung:

| Tier | Intent |
| --- | --- |
| built-in defaults | The floor. Close to industry consensus, so a fresh repository gets actionable output. |
| [`confs/recommended`](https://github.com/PowderworksCode/ordnung/tree/main/confs/recommended) | Stricter practices most teams would accept. Mandates no specific linter. |
| [`confs/paranoid`](https://github.com/PowderworksCode/ordnung/tree/main/confs/paranoid) | Everything on, including specific tools and Ordnung's own conventions. Extends `recommended`. |

```toml
[[extends]]
git = "https://github.com/PowderworksCode/ordnung"
rev = "<full 40-character commit revision>"
path = "confs/paranoid"
```

Each tier extends the one below, so a file states the difference between tiers
rather than the whole check list.

The tiers carry content as well as severities. A tier that turns a check on and
leaves every adopter to write the file that satisfies it has done half a job, so
`recommended` ships the Git hooks, the Dependabot configuration, the TypeScript
bases, and a codespell workflow, and `paranoid` adds the Vale and Stylelint
configuration and replaces that workflow with one running every linter it
mandates. Extending a tier distributes those files to members on the next
`fleet sync`.

None of it is compulsory. A fleet that wants its own version of a file reuses the
entry's `name` with its own `source`; a fleet that wants none of it says
`state = "unmanaged"`; a fleet that wants the file on some members only reuses the
name with an `only` list and no source. What each tier ships is listed in its own
`policy.toml`.

Revisions are pinned deliberately: an inherited layer can write files into every
member repository, so a moving reference would make plans non-deterministic.

## Substituting managed files

A `[[managed]]` entry with `substitute = true` writes the member into its
source before comparing and applying, so one fleet file can carry
repository-specific content — a release workflow that names its binary, an
install script that names its repository:

```toml
[[managed]]
name = "release-workflow"
source = "managed/publishing/release.yml"
destination = ".github/workflows/release.yml"
substitute = true
only = ["PowderworksCode/straitjacket"]
```

<!-- The site generator performs its own brace substitution on source, so
the literal placeholders in this section are spelled with the `open` var the
build script defines. The rendered page and its markdown twin carry the real
braces. -->
Four placeholders exist: `{{open}}repo}}` (owner/name), `{{open}}name}}` (the repository
name), `{{open}}NAME}}` (the name uppercased with `-` as `_`, for environment
variables), and `{{open}}website}}` — the member's own site, declared by the fleet:

```toml
[[member]]
repo = "PowderworksCode/straitjacket"
website = "https://straitjacket.dev"
```

A repository name is not an address, so `{{open}}website}}` comes from the fleet
rather than being derived. A file that substitutes it into a member with no
declared website fails the plan, naming the member: an install URL nobody
answers at is worse than no install URL. GitHub expressions (`${{ ... }}`) pass through untouched; any
other bare `{{` fails the plan, so a misspelled placeholder cannot ship
literally to every member. Substitution requires a UTF-8 file source, not a
directory.

## Selecting projects

A `relative_to = "project"` entry selects with `when`, which accepts `language`,
`capability`, `ecosystem`, or `ecosystems` — the last naming several, where any
one of them matches:

```toml
[[managed]]
name = "typescript-tsconfig-base"
source = "managed/typescript/tsconfig.base.json"
destination = "tsconfig.base.json"
relative_to = "project"
when = { ecosystems = ["npm", "bun"] }
```

The other fields combine with `and`; `ecosystems` is the one that reads as
`or`, because a package belongs to one package manager rather than several.

Prefer it to `language` when the file belongs to a package. A tree-sitter
grammar for TypeScript is written in TypeScript and is not a TypeScript
package: selecting on the language puts a tsconfig in a Cargo crate that has no
`package.json` anywhere in it.

## Other configuration keys

`.ordnung/overrides.toml` also accepts keys consumed directly by checks:

| Key | Used by |
| --- | --- |
| `ignore` | paths excluded from the repository walk entirely |
| `[ci_exists]` | `ignore` list of scratch project paths exempt from requiring CI |
| `[[codegen]]` | generator declarations checked for drift by `codegen-drift` |
| `[scripts]` | shell script directory, development script, allowances |
| `[stray_files]` | notes directory and allowed root files |
| `[test_layout]` | source roots, test root, and suffixes per language |
| `[[dependencies]]` | packages a project must declare, by language or ecosystem |
| `[github]` | repository settings Ordnung manages |

See [design.md](https://github.com/PowderworksCode/ordnung/blob/main/docs/design.md) for the full contract.
