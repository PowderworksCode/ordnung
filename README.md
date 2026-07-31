# ordnung

Ordnung checks that a GitHub repository is structurally in order and keeps
fleet-owned configuration synchronized across repositories.

The project is a Rust workspace with two initial crates:

- `ordnung-core`: recursive project inventory, policy resolution, checks, and
  deterministic change planning.
- `ordnung-cli`: standalone repository commands and fleet configuration
  commands.

Ordnung inspects repository layout, structured manifests, lockfiles,
configuration, source-file extensions, and workflows. It does not perform
static code analysis. The local `entl-codebase` dependency walks the repository
once and produces typed package, project, workspace, lockfile-ownership,
language, ecosystem, and facet facts. `entl-github` derives workflow, trigger,
tool, and automation-task facts from that inventory without walking again.
The two crates own the distributed profiles that Ordnung consumes. Checks use
the same distributed-registry pattern: each module in
`ordnung-core/src/checks` owns its ID, default severity, agent instructions, and
optional runner. The CLI also shells out to authenticated `gh api` calls for
typed repository, branch, security, and workflow facts. Exact check fixes and
fleet-managed files are combined into one deterministic plan. The GitHub
adapter materializes that plan on a reusable `ordnung/remediation` branch and
opens or updates one pull request per member.

## Getting Started

Run `./scripts/dev.sh` once after cloning to build the Rust workspace and
install the documentation site's locked Bun dependencies.

When a sibling `../entl` checkout exists, the development script builds against
that moving source instead of the pinned Git revision. Run other local Cargo
commands through the repository's lockfile-preserving `local_dev` command:

```sh
./scripts/local_dev test --workspace
./scripts/local_dev clippy --workspace --all-targets -- -D warnings
```

Ordinary `cargo` commands continue to use the exact Entl revision in
`Cargo.toml`, so standalone clones and CI do not require a sibling repository.

Pull request titles follow [Conventional Commits](https://www.conventionalcommits.org/)
using `type(scope): summary`; CI enforcement will return with the repository's
workflows.

## Usage

```sh
cargo test --workspace
cargo run -p ordnung-cli -- inspect .
cargo run -p ordnung-cli -- check .
cargo run -p ordnung-cli -- repo-check . --repo PowderworksCode/ordnung
cargo run -p ordnung-cli -- fix .
cargo run -p ordnung-cli -- instructions .
cargo run -p ordnung-cli -- instructions . \
  --write AGENTS.md --write CLAUDE.md
cargo run -p ordnung-cli -- fleet check ../fleet-configuration/fleet.toml
cargo run -p ordnung-cli -- fleet sync ../fleet-configuration/fleet.toml \
  --repo PowderworksCode/ordnung --repo-root .
cargo run -p ordnung-cli -- github check PowderworksCode/ordnung
cargo run -p ordnung-cli -- fleet github-check ../fleet-configuration/fleet.toml
cargo run -p ordnung-cli -- fleet github-sync-settings \
  ../fleet-configuration/fleet.toml --repo PowderworksCode/ordnung
cargo run -p ordnung-cli -- fleet github-sync \
  ../fleet-configuration/fleet.toml --repo PowderworksCode/ordnung
cargo run -p ordnung-cli -- fleet github-sync-all \
  ../fleet-configuration/fleet.toml
```

Mutation commands are dry-run by default. `--apply` writes local exact fixes,
applies supported GitHub settings, or opens/updates the consolidated
remediation pull request. JSON responses use a versioned envelope with
`schema_version`, `command`, `ok`, and `data`. Exit code `0` means clean or
successfully applied local state, `1` means policy drift, and `2` means an
operational or configuration error.

## Documentation

See [docs/design.md](docs/design.md) for the complete product and architecture
contract. User-visible project changes are recorded in [CHANGELOG.md](CHANGELOG.md).

The user documentation site is a static Fumadocs application under `site/`. During local package
development it consumes `@thepowderworks/fumadocs` from the sibling `../docs` repository:

```sh
(cd ../docs && bun install && bun run build)
(cd site && bun install && bun run dev)
```

Each user-facing page declares a Diátaxis `mode`. `bun run docs:check` reports advisory content
architecture issues, and `bun run docs:check:strict` is available when the site is ready to make
the shared contract blocking. The advisory check also runs automatically before a site build.

`ordnung instructions` renders a short deterministic Markdown block containing
the effective checks and repository conventions for coding agents. Repeated
`--write` arguments inject or refresh an Ordnung-owned marker block while
preserving the rest of each file. Fleet-aware generation accepts `--fleet` and
`--repo` together so centralized policy, GitHub settings, explicit exceptions,
and effective managed paths are included.

## Contributing

Use `./scripts/dev.sh` to prepare a checkout, run the workspace tests before
opening a pull request, and use a Conventional Commit title. Use
`./scripts/local_dev` while changing local dependencies such as Entl alongside
Ordnung. Keep checks and their agent instructions together under
`ordnung-core/src/checks`; integration tests belong under each crate's `tests/`
directory.

## License

Ordnung is available under the [MIT License](LICENSE).
