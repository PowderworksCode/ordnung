# The site cannot build outside one machine

Ordnung reports three `required` failures against its own repository. All three
have the same cause, and it is not a missing CI job.

## The evidence

```console
$ cd site && bun install --frozen-lockfile
372 packages installed [900.00ms]
Failed to install 1 package
```

The package that fails is declared in `site/package.json` as:

```json
"@thepowderworks/fumadocs": "file:../../docs/packages/fumadocs"
```

That path resolves to a sibling of the repository checkout. On this machine it
exists. Everywhere else it does not:

| Check | Result |
| --- | --- |
| `~/powderworks/docs/packages/fumadocs` exists locally | yes |
| `~/powderworks/docs` is a git repository | **no** — `fatal: not a git repository` |
| `@thepowderworks/fumadocs` on npm | **404** |
| `PowderworksCode/docs` reachable from here | cannot tell; the exe.dev proxy only serves repositories it is configured for, and refuses this one the same way it refuses a name that does not exist |

So the dependency lives in an unversioned directory on one developer's disk.
`actions/checkout` cannot fetch it, and no registry has a copy. A GitHub runner
cannot install the site's dependencies at all.

## What that costs

```
fail  required  builds     site/package.json: build target "build" does not run on push or pull_request
fail  required  ci-exists  .github/workflows: typescript CI is missing test, lint, format tasks on push or pull_request
fail  required  typecheck  site: TypeScript CI has no typecheck task on push or pull_request
```

`ordnung check .` exits 1 against Ordnung's own repository. For a tool whose
claim is "know which repositories are structurally in order", that is the first
thing a sceptical reader will run.

Ordnung is not wrong. It is correctly reporting that this repository contains a
project nobody else can build.

## What the package is

Measured, not assumed:

| | |
| --- | --- |
| Source | 11 files, ~632 KB including `dist` |
| Manifest | `"private": true`, `"version": "0.0.0"` |
| Build | `tsc -p tsconfig.json`, plus a `postbuild` marking the CLI executable |
| Ships | `files: ["dist", "src/theme.css"]` |
| Entry points | 9 exports, one `powderworks-docs` bin |
| Peer dependencies | `@orama/orama`, `fumadocs-core` 16.11.1, `fumadocs-ui` 16.11.1, `next`, `react`, `react-dom` |

**Every peer dependency is already satisfied by `site/package.json` at a
compatible version.** The package itself is the only missing piece.

The site uses eight of the nine exports plus the CLI, so there is no "depend on
less of it" option:

```
@thepowderworks/fumadocs/config    @thepowderworks/fumadocs/search
@thepowderworks/fumadocs/i18n      @thepowderworks/fumadocs/provider
@thepowderworks/fumadocs/layout    @thepowderworks/fumadocs/mdx
@thepowderworks/fumadocs/theme.css @thepowderworks/fumadocs
@thepowderworks/fumadocs/dist/cli.js  (via docs:check, prebuild)
```

## Routes

### 1. Publish to npm — recommended if more than one repository consumes it

The name and the sibling-`docs` convention both suggest this package is meant to
be shared. Publishing is the only route that scales past two consumers.

In the docs repository:
- drop `"private": true` and set a real version;
- add `description`, `license`, and `repository` fields;
- add `prepack` (or `prepublishOnly`) running the existing build, so `dist` ships;
- publish under the `@thepowderworks` scope — needs the npm org and a token.

In this repository:
- change the dependency to a pinned version;
- `bun install` to refresh `site/bun.lock`.

**Cost:** hours, plus an ongoing release process for a second package.
**Note:** Ordnung's own `pinned-dependencies` check will want it pinned exactly,
not floating — which is the behaviour you would want for a package that writes
into the site's build anyway.

### 2. Git dependency — fastest, if a docs repository exists

Requires `docs` to become a real git repository with a remote, which it is not
today.

The catch is that the package sits at `packages/fumadocs` inside that tree, and
npm-style git dependencies do not address subdirectories portably — `pnpm`
supports a path fragment, `bun` and `npm` do not. In practice this route means
giving `fumadocs` its own repository rather than pointing at a monorepo path.

Bun does run `prepare` for git dependencies, so `dist` need not be committed.

**Cost:** low if `fumadocs` gets its own repository; blocked otherwise.

### 3. Vendor into this repository — unblocks today, forks the code

Copy `packages/fumadocs` to `site/vendor/fumadocs` and point the dependency at
`file:./vendor/fumadocs`. Everything then installs and builds on a runner with
no external state.

**Cost:** lowest, immediate. **Downside:** two copies that drift, and the
vendored source needs its own build step and dev dependencies in CI. Vendoring
only the built `dist` avoids the build step but commits build output.

Reasonable as a stopgap that unblocks Ordnung's CI now while route 1 proceeds —
provided the vendored copy is deleted the moment the package is published,
rather than becoming permanent by accident.

## After the dependency is obtainable, three checks still need work

Unblocking the install does not by itself turn the three failures green. What
remains, in this repository:

1. **`typecheck` and `builds`** — satisfied by a `site` CI job running
   `bun run types:check` and `bun run build` on push and pull request. About
   twenty lines. Ready to write.
2. **`ci-exists`** wants test, lint, and format for TypeScript.
   - *lint and format*: add Biome. `biome ci` is recognised by Entl as both, so
     one config file, one dev dependency, and one CI step covers two of three.
   - *test*: **there are no tests for the site, and none can be written today**
     because every file under `site/src/` imports the blocked package. Once it
     installs, real tests can be written. `bun test` is recognised as the test
     task.

I deliberately did not write a placeholder test to turn that check green. Gaming
your own scanner is the one change that would make its output worthless.

## Recommendation

Route 1, with route 3 as an explicit stopgap if you want Ordnung's own audit
clean before the package is published. Either way the follow-up work in this
repository is small and I can do it as soon as `bun install` succeeds on a
runner.

If publishing is not going to happen soon, the honest alternative is to move the
site out of this repository — it drags three required failures and thirteen
floating-dependency advisories into Ordnung's audit for a component that is not
part of the tool.
