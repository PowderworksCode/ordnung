# Configuring Ordnung

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
| [`confs/recommended`](../confs/recommended) | Stricter practices most teams would accept. Mandates no specific linter. |
| [`confs/paranoid`](../confs/paranoid) | Everything on, including specific tools and Ordnung's own conventions. Extends `recommended`. |

```toml
[[extends]]
git = "https://github.com/PowderworksCode/ordnung"
rev = "<full 40-character commit revision>"
path = "confs/paranoid"
```

Each tier extends the one below, so a file states the difference between tiers
rather than the whole check list.

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

Three placeholders exist: `{{repo}}` (owner/name), `{{name}}` (the repository
name), and `{{NAME}}` (the name uppercased with `-` as `_`, for environment
variables). GitHub expressions (`${{ ... }}`) pass through untouched; any
other bare `{{` fails the plan, so a misspelled placeholder cannot ship
literally to every member. Substitution requires a UTF-8 file source, not a
directory.

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

See [design.md](design.md) for the full contract.
