# recommended

A strict, opinionated baseline shipped with Ordnung. It is stricter than
Ordnung's built-in check defaults and carries no repository names, so any fleet
can adopt it.

It lives in this repository but is consumed through the same `[[extends]]`
mechanism any third party uses, so the publishing path is exercised by
Ordnung's own baseline rather than only by external configurations.

## The shipped tiers

- **Ordnung's built-in defaults** — the floor. Checks that fail here are close
  to industry consensus, so a fresh repository gets actionable output.
- **`recommended`** (this tier) — stricter practices most teams would accept.
  Deliberately mandates no specific linter.
- **[`paranoid`](../paranoid)** — everything on, including specific tools and
  Ordnung's own conventions. Extends this tier.

`.ordnung/policy.toml` is a policy library: it declares check severities and
GitHub settings, but no `[[member]]`. Members are never inherited, so extending
this repository can only change what "in order" means — never which
repositories a fleet manages.

## Using it

Point a fleet at a published revision:

```toml
[[extends]]
git = "https://github.com/PowderworksCode/ordnung"
rev = "<full 40-character commit revision>"
path = "confs/recommended"
```

With `git`, `path` selects a directory inside the fetched repository, so one
repository can publish several tiers. Without it, the repository's own
`.ordnung` directory is used.

Revisions are pinned deliberately. An inherited layer can write files into every
member repository, so a moving reference would make plans non-deterministic and
turn an upstream edit into an unreviewed change across your fleet.

A sibling checkout can be used during local development instead:

```toml
[[extends]]
path = "../../ordnung/confs/recommended"
```

## Overriding what you inherit

`allow_override` governs member repositories only. A fleet that extends this
library may redefine anything in it — you chose to import it, so you can
un-choose any part:

```toml
[policy.checks]
test-layout = { severity = "off", allow_override = true }
```

Managed entries merge by name. Reuse a name to replace that entry, or drop it
entirely without touching member files:

```toml
[[managed]]
name = "editorconfig"
state = "unmanaged"
destination = ".editorconfig"
```

`unmanaged` stops inheriting. `absent` is different: it asserts the file must
not exist and deletes it from every member.

## Required dependencies

`[[dependency]]` requires packages of every project matching a language or an
ecosystem, so tooling that reasons about installed libraries can rely on them:

```toml
[[dependency]]
name = "rust-refactoring"
language = "rust"
require = ["itertools"]
```

Package names belong to one registry, so each entry selects a single language or
ecosystem. `kind = "development"` narrows which dependency kind satisfies the
requirement; any kind satisfies it by default. Requirements merge by name and
accept `state = "unmanaged"` exactly like managed entries.

The check reports what is missing but never edits a manifest. Adding a dependency
means choosing a version and updating a lockfile, which Ordnung cannot resolve
deterministically without a network, so the fix stays with you.
