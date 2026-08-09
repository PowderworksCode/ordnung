# ordnung

Ordnung checks that a repository is *structurally* in order — and, across many
repositories, keeps shared configuration synchronized from one place.

It does not read your code. It reads what surrounds it: manifests, lockfiles,
workflows, `.gitignore`, README, CODEOWNERS, Dependabot config, branch
protection, and the layout those live in.

## The problem it solves

Every repository accumulates the same small structural debts. A lockfile that
was never committed. CI that tests but does not typecheck. An Action pinned to
a tag instead of a SHA. A second package that no Dependabot entry covers.

None breaks anything today, none is what a code reviewer looks at, and each is
invisible until it matters. Across a dozen repositories, nobody knows which are
in order. Ordnung makes that state checkable, so it can be a CI gate rather than
an incident postmortem.

## What running it looks like

A four-file Rust project — `Cargo.toml`, `src/main.rs`, `README.md`,
`.gitignore`:

```console
$ ordnung check .
fail  recommended artifacts-built        .: compiled binary is not built by any workflow
fail  recommended changelog              CHANGELOG.md: no root changelog found
fail  required    ci-exists              .github/workflows: no GitHub Actions workflows found
pass  recommended ci-scoped              .github/workflows: no heavy pull-request job runs without change scoping
fail  recommended codeowners             .github/CODEOWNERS: no CODEOWNERS file found in .github, the repository root, or docs
fail  recommended codespell              .github/workflows: no push or pull-request workflow runs Codespell
fail  recommended conventional-commits   .github/workflows: no CI enforcement for PR titles or commit messages; Conventional Commits are not mentioned in README or CONTRIBUTING.md
fail  required    dependabot             .github/dependabot.yml: no .github/dependabot.yml or .github/dependabot.yaml found
pass  required    gitignore              target: Cargo ignores target/ at .
fail  recommended license                LICENSE: no root license file found
fail  required    lockfiles              .: Cargo package has no Cargo.lock at its lockfile owner .
pass  recommended pinned-dependencies    .: Cargo advisory: 1 floating reference(s): /:serde 1 cargo
pass  required    project-inventory      .: detected 1 project boundary/boundaries
pass  required    readme                 README.md: README.md opens with an H1 title
fail  recommended readme-quality         README.md: under 150 words (5); no install/getting-started section; no usage/docs section; no License section heading; no Contributing section heading
pass  required    reproducible-toolchain .github/workflows: no GitHub setup action uses an unbounded toolchain version
fail  recommended scripts                scripts/dev.sh: no scripts/dev.sh to stand up the development environment
pass  required    test-retry-masking     .: no rerun-until-green test retry is configured
$ echo $?
1
```

Four columns: **status**, **severity**, **check ID**, then the **scope** and the
reason. Every line names the file it is about. Abridged: 15 `skip` and
`off`-severity lines are omitted; the full run is 33 lines.

The three `required` failures are real: no CI, no Dependabot config, no
committed `Cargo.lock`.

## Reading the output

**Status** is `pass`, `fail`, `skip`, or `error`. **Severity** decides what a
failure costs you:

| Severity | Meaning | Affects exit code |
| --- | --- | --- |
| `required` | Ordnung considers this broken | **yes** |
| `recommended` | Worth fixing, not a gate | no |
| `off` | Reported for information only | no |

Exit `0` means no `required` check failed, `1` means policy drift, `2` means an
operational or configuration error.

Two things are worth knowing before your first run:

- **`off` checks still run and still print `fail`.** They are opinions Ordnung
  holds but does not enforce — one test file per source file, a `.vale.ini`,
  git hooks — and on a large repository they can outnumber the real findings
  several times over. Filter with `ordnung check --json` on `severity`. A rough
  edge in the output layer, not a verdict on your repository.
- **`ordnung check` runs 33 of Ordnung's 47 checks.** The other 14 read GitHub
  state — branch protection, secret scanning, workflow permissions — and need an
  API call. Four are `required`. A clean `ordnung check` is not a clean
  repository; use `repo-check` for that.

## Install

Builds with a stable Rust toolchain; no system dependencies beyond `git`.

```sh
cargo install --git https://github.com/PowderworksCode/ordnung ordnung-cli --locked
```

From a clone:

```sh
git clone https://github.com/PowderworksCode/ordnung
cd ordnung
cargo install --path crates/ordnung-cli --locked
```

The binary is named `ordnung`. Not on crates.io — the name is taken there by an
unrelated crate.

**Anything touching GitHub also needs the [`gh` CLI](https://cli.github.com/)
installed and authenticated** (`gh auth login`) — Ordnung shells out to `gh api`
rather than handling tokens itself. `ORDNUNG_GH` selects a different binary.
Working-tree-only commands (`check`, `inspect`, `fix`, `instructions`) do not
need it.

## The three ways to run it

### One repository, locally

```sh
ordnung inspect .                                  # what Ordnung detected
ordnung check .                                    # local checks only
ordnung repo-check . --repo owner/name             # local + GitHub checks
ordnung fix .                                      # show exact fixes
ordnung fix . --apply                              # write them
ordnung instructions . --write AGENTS.md           # agent rules
```

`fix` is deliberately narrow: it only offers changes it can make exactly,
without guessing, and says so when it has nothing to offer. `instructions`
renders a deterministic Markdown block of the checks in force and injects it
into a marker-delimited region of the files you name, leaving the rest alone.

### In CI, as a GitHub Action

```yaml
- uses: PowderworksCode/ordnung@<pinned-sha>
  with:
    mode: repo-check          # or check, fix, github-check, fleet-check, fleet-sync
    repository: ${{ github.repository }}
```

Outputs are `outcome` (`clean`, `drift`, `error`) and `exit-code`;
`github-token` defaults to `${{ github.token }}`.

> **The Action builds Ordnung from source on every run.** There is no release
> binary yet, so each invocation runs `cargo install` — expect minutes, not
> seconds. Set `ORDNUNG_BIN` to an installed binary to skip the build.

### Across a fleet

A fleet manifest names member repositories and the policy they share; see
[docs/configuration.md](docs/configuration.md) for how layers resolve.

```sh
ordnung fleet check fleet.toml                 # validate manifest
ordnung fleet github-check fleet.toml          # audit every member
ordnung fleet github-sync fleet.toml --repo owner/name
```

**Every mutating command is dry-run by default and requires `--apply`.** Read
[Consent and writes](#consent-and-writes) before pointing it at repositories you
do not own.

## Consent and writes

What Ordnung writes, precisely:

- Without `--apply`, nothing is written anywhere; every command prints its plan.
- `fix --apply` writes files in the local working tree only.
- `fleet github-sync --apply` does two things under one flag: it **changes
  repository settings immediately**, and it pushes a branch and opens a pull
  request. The settings change is live; the file changes are reviewable.
- That pull request always uses a branch named **`ordnung/remediation`**, which
  Ordnung **force-pushes**, re-parented onto the current default branch. Commits
  pushed to that branch by anyone else are discarded. The name is not
  configurable.
- `fleet github-sync-all --apply` does all of the above for every fleet member
  in one invocation.
- Archived repositories are refused.

Requires a `gh` login with write access to the targets.

## Configuring

Configuration is optional; the defaults are useful with no config file at all.
A repository's settings live in `.ordnung/overrides.toml`, and Ordnung ships
three policy tiers under [`confs/`](confs) — built-in defaults, `recommended`,
and `paranoid` — each inherited through the same `[[extends]]` mechanism a
third-party configuration uses.

See [docs/configuration.md](docs/configuration.md) for the resolution order, the
exception mechanism, and every available key.

## How it works

Ordnung walks the repository exactly once. `entl-codebase` turns that walk into
typed facts — packages, projects, workspaces, lockfile ownership, languages,
ecosystems — and `entl-github` derives workflow and tool facts from the same
inventory. Checks read facts; they never rescan.

Each check is one self-registering module under
`crates/ordnung-core/src/checks/`, owning its ID, default severity, category,
scope, agent instructions, and runner. Adding a check means adding one file.

Check fixes and fleet-managed files combine into one deterministic plan before
anything is written. See [docs/design.md](docs/design.md) for the full contract.

## Ordnung or straitjacket?

Both are CI scanners, and they do not overlap. **Ordnung checks the repository
around the code** — that CI exists and gates the right things, that lockfiles
and Dependabot cover every package, that branch protection is on. It never opens
a source file.
**[straitjacket](https://github.com/PowderworksCode/straitjacket) checks the
code itself** for smells and forbidden patterns.

"Is this project set up correctly?" is Ordnung. "Is this code written well?" is
straitjacket. Running both is normal.

## Documentation

[docs/design.md](docs/design.md) is the complete architecture contract;
user-visible changes are in [CHANGELOG.md](CHANGELOG.md).

The documentation site is a static Fumadocs application under `site/`, consuming
`@thepowderworks/fumadocs` from the sibling `../docs` repository locally:

```sh
(cd ../docs && bun install && bun run build)
(cd site && bun install && bun run dev)
```

Each page declares a Diátaxis `mode`; `bun run docs:check` reports content
issues before a build.

## Contributing

Run `./scripts/dev.sh` once after cloning. Gates before opening a pull request:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

When a sibling `../entl` checkout exists, `./scripts/local_dev <cargo-args>`
builds against that moving source without rewriting the lockfile. Plain `cargo`
always uses the Entl revision pinned in `Cargo.toml`, so clones and CI need no
sibling repository.

Pull request titles follow
[Conventional Commits](https://www.conventionalcommits.org/). Keep checks and
their agent instructions together under `crates/ordnung-core/src/checks`;
integration tests belong under each crate's `tests/` directory.

## License

Ordnung is available under the [MIT License](LICENSE).
