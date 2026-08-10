# Refactoring review

Written against `prep-for-publishing` at 16,383 lines of Rust across
`ordnung-core` and `ordnung-cli`. **Nothing in this document has been applied.**
Every item is a proposal.

## Summary

The core is in better shape than the file sizes suggest. The check registry
holds up, the `gh` shell-out is properly isolated, policy resolution is
well-layered, and there are no TODOs or dead checks. The problems are
concentrated in three places: **`main.rs` mixes four unrelated jobs**, **fleet
orchestration logic lives in the binary crate where it cannot be tested**, and
**the output layer has no aggregation, filtering, or summary**.

Ranked by what I would do first:

| # | Item | Size | Risk |
| --- | --- | --- | --- |
| 1 | Move fleet orchestration out of `main.rs` into core | large | medium |
| 2 | Split `main.rs` rendering into `render.rs` | medium | low |
| 3 | Split `gh.rs` into `gh/{mod,runner,wire}.rs` | medium | none |
| 4 | Split `fleet.rs`; put its `git` shell-out behind a trait | large | medium |
| 5 | Collapse the five `run_repository_checks*` entry points | small | low |
| 6 | Pass `&RepoConfig` instead of enumerating fields in the context | small | low |
| 7 | De-duplicate `ensure_no_symlink_path` | small | low |
| 8 | Delete the no-op `apply_policy` call | trivial | none |
| 9 | Split `tests/check.rs` by category | medium | none |

---

## 1. `main.rs` (1,247 lines) is four files

Four distinct responsibilities, cleanly separable along existing line
boundaries:

| Lines | Job | Approx |
| --- | --- | --- |
| 23–197 | clap surface: `Cli`, `Command`, seven `Args` structs, two `Subcommand` enums | 175 |
| 221–1005 | command handlers and fleet orchestration | 785 |
| 1006–1247 | printing and formatting | 240 |
| 290–351 | filesystem side effects (`write_instructions`) | 60 |

The rendering block (`print_inventory`, `print_report`,
`print_github_setting_changes`, `print_remediation_plan`, `print_json`,
`pull_request_body`, `status_name`, `severity_name`, `display_scope`,
`file_operation_name`) has no dependency on clap or on command dispatch. Moving
it to `crates/ordnung-cli/src/render.rs` is mechanical, changes no behaviour, and
is the single highest ratio of clarity gained to risk taken in this repository.

**Recommendation:** `main.rs` keeps the clap surface and a dispatch `match`
(~250 lines). Handlers move to `commands/` or stay. Rendering moves to
`render.rs`.

### 1a. The important part: orchestration is in the wrong crate

`sync_fleet_member` (`main.rs:856`) and `github_sync_fleet` are not CLI code.
`sync_fleet_member` fetches repository facts, refuses archived repositories,
clones to a tempdir, loads config, resolves policy, runs both check suites,
plans managed changes, plans setting changes, builds a remediation plan, and
decides whether to mutate settings and open a pull request. That is the entire
fleet sync product, sitting in a binary crate.

Consequences:

- It cannot be unit tested. `tests/gh_adapter.rs` tests the `gh` client
  underneath it with a `FakeRunner`, and `tests/fleet.rs` tests planning above
  it, but the decision logic joining them is only reachable by running the
  binary.
- `fleet_requirements` (`main.rs:842`) and `pull_request_body` (`main.rs:1191`)
  are policy and content decisions expressed in the CLI.
- The same sequence is partially re-implemented in `github_sync_fleet_all`.

**Recommendation:** move `sync_fleet_member` into `ordnung-core` (or a new
`ordnung-cli/src/sync.rs`) taking `&GhClient<R: GhRunner>`, which the existing
trait already permits. The CLI shrinks to argument parsing and rendering, and
the sync path becomes testable against `FakeRunner` end to end. This is the
largest item here and the only one I would call structurally important.

---

## 2. `gh.rs` (1,256 lines): the boundary is right, the layout is not

**The brief asked whether the `gh` shell-out is isolated behind a trait so it can
be tested and swapped. It is.** `GhRunner` (`gh.rs:832`) has one required
method; `ProcessRunner` is the production implementation; `GhClient<R =
ProcessRunner>` is generic over it; `tests/gh_adapter.rs` drives the whole client
through a `FakeRunner`. The binary is selectable at runtime via `ORDNUNG_GH`.
This is the best-factored seam in the codebase and needs no redesign.

The file is still three things stacked:

| Lines | Content |
| --- | --- |
| 24–831 | `GhClient` methods — the API surface |
| 832–879 | `GhRunner`, `ProcessRunner`, `GhOutput` |
| 881–1076 | ~25 private serde wire DTOs |
| 1077–1240 | date, encoding, validation helpers |

**Recommendation:** `gh/mod.rs` (client), `gh/runner.rs`, `gh/wire.rs`,
`gh/util.rs`. Pure code movement; the DTOs are already private.

**Constraint to document, not fix:** `ProcessRunner` requires a `gh` binary on
`PATH` and an authenticated session. The failure is legible but unhelpful:

```console
$ ORDNUNG_GH=/nonexistent-gh ordnung github inspect owner/name
error: could not execute gh: No such file or directory (os error 2)
```

It names `gh` but does not say to install the GitHub CLI or run `gh auth login`.
Worth one sentence in the error.

---

## 3. `fleet.rs` (1,171 lines) is five files, one of which breaks the project's own rule

| Lines | Job |
| --- | --- |
| 26–143, 628–811 | manifest schema and types |
| 321–456, 527–627 | layer resolution and policy merging |
| **457–526** | **`git` shell-out and on-disk layer cache** |
| 813–1081 | change planning and application |
| 1082–1171 | path safety helpers |

`fetch_git_layer` (`fleet.rs:457`) shells out to `git` through a private `git()`
helper and writes into a cache directory. Ordnung put its *other* subprocess
dependency behind `GhRunner` and tests it with a fake — and then did not do the
same here. This is the one place where the design does not meet its own
standard: resolving `[[extends]]` cannot be tested without network and a real
git, so the layer-inheritance path, which is how every shipped `confs/` tier is
consumed, is the least testable part of the system.

**Recommendation:** split the file, and introduce a `LayerFetcher` trait
mirroring `GhRunner`. Same shape, same benefit.

---

## 4. Smaller structural items

### 4a. Five entry points where one belongs (`check.rs:271–316`)

```
run_repository_checks
  └─ run_repository_checks_with_config
       └─ run_repository_checks_with_repo_config
            └─ run_repository_checks_with_requirements
                 └─ run_repository_checks_for_state
```

Each adds one parameter and delegates. Classic parameter accretion. Replace with
one function taking an options struct, or a small builder. Three of the five have
no caller outside tests.

### 4b. The context struct is the registry's real scaling limit (`check.rs:168–177`)

```rust
pub struct RepositoryCheckContext<'a> {
    pub root: &'a Path,
    pub inventory: &'a Inventory,
    pub dependencies: &'a [DependencyRequirement],
    pub ci_exists: &'a CiExistsConfig,
    pub codegen: &'a [CodegenConfig],
    pub scripts: &'a ScriptsConfig,
    pub stray_files: &'a StrayFilesConfig,
    pub test_layout: &'a TestLayoutConfig,
}
```

Six of these eight fields are config sections, hand-copied out of `RepoConfig` by
the caller, which already holds a `&RepoConfig`. Adding a check that needs new
configuration means editing a struct every check module sees. **This — not the
module count — is what makes the registry harder at 150 checks than at 49.**

**Recommendation:** hold `config: &'a RepoConfig` and let checks read the section
they need. One field replaces six, and adding config stops touching shared code.

### 4c. `ensure_no_symlink_path` exists twice and has drifted

`fleet.rs:1145` and `main.rs:328` implement the same symlink-traversal guard. The
`fleet.rs` copy additionally verifies the path does not escape the repository
root; the `main.rs` copy does not, and relies on its only caller
(`write_instructions`, `main.rs:290`) rejecting absolute paths and non-`Normal`
components first.

**This is not a live vulnerability** — the caller's validation is sufficient
today. It is a maintenance hazard: two copies of a path-traversal guard, already
divergent, where the safety of one depends on a check in a different function.

**Recommendation:** one implementation in core, exported and used by both.

### 4d. A guaranteed no-op (`check.rs:345`)

```rust
report.apply_policy(&default_policy());
```

`result()` (`check.rs:375`) already stamps every result with
`definition.default_severity`, and `default_policy()` maps each ID to exactly
that. The call cannot change anything. `run_github_checks_with_settings` does not
make it, so the two paths are also inconsistent. Delete it; the CLI applies the
real resolved policy at all ten of its call sites.

### 4e. `CheckScope` is declared 47 times and read only by a test

Its doc comment says policy that selects directories "can only apply to the
latter, and declaring the scope is what lets that be validated instead of
silently misfiring." No such validation exists. The only consumer is
`tests/check_registration.rs:54`, which asserts the count is 12.

Either implement the validation the comment describes, or delete the field. Note
that `CheckCategory` **is** genuinely consumed — `instructions.rs:254` groups
generated agent rules by it — so this is not an argument against definition
metadata generally.

### 4f. Policy resolution repeats at ten call sites

`apply_policy` is called ten times in `main.rs` (379, 431, 434, 467, 484, 602,
669, 893, 895, 991), each preceded by its own `RepoConfig::load_optional` +
`resolve_policy` sequence. A single `fn resolved_context(path) -> (Inventory,
Policy, RepoConfig)` would remove the repetition and the chance of a command path
forgetting a step.

---

## 5. Is the 49-module check registry the right shape?

**Yes, and it is one of the better decisions in the codebase.** Corrections to
the framing first:

- There are **47 checks**, not 49. `checks/` holds 49 files: 47 check modules,
  `mod.rs`, and `test_layout.rs`, which is shared helper code for `test-inline`
  and `test-mirror` and registers no check.
- **No check is registered without a runner.** 33 have a repository runner, 16 a
  GitHub runner, 2 (`dependabot`, `license`) have both.
- Total 4,971 lines across the modules — about 100 lines each, largest 232. No
  module is too big.

What works: `registry::collect!` self-registration (the `inventory` crate, aliased `profile_inventory`) means adding a check is
adding one file, with no central list to edit and no merge conflict surface.
`CHECK_DEFINITIONS` sorts once and asserts ID uniqueness at startup.
`check_definition` binary-searches. Instructions live next to the runner that
enforces them, which is why `ordnung instructions` cannot drift from behaviour.

What will hurt at 150:

1. **The context struct** (4b) — the real limit.
2. **Everything runs, always.** `run_repository_checks_for_state` iterates every
   definition with a runner and executes it, then relabels severities afterward.
   A check set to `off` still does its filesystem work. There is no way to run a
   subset. At 150 checks that is real time on every invocation.
3. **No dependencies between checks.** Several already re-derive the same facts;
   `ci-*` checks each re-walk workflow structures. A "these checks share a
   derived fact" mechanism would be wanted before 150.

None of these argue for abandoning the pattern. I would keep it.

---

## 6. `tests/check.rs` (2,216 lines): not hiding anything

The brief flagged this as possibly concealing untested seams. It is not.
60 `#[test]` functions, ~37 lines each, 6 shared helpers, each test building a
temporary repository and asserting on one check's findings. It is long because
there are many checks, not because it is tangled. Every check ID but
`project-inventory` appears by name in some test.

**Recommendation:** split by category (`tests/checks_ci.rs`,
`tests/checks_docs.rs`, …) purely for navigability. No restructuring.

---

## 7. What I did not find

Worth recording, because their absence is a quality signal:

- **No TODO, FIXME, XXX, `todo!()`, or `unimplemented!()` anywhere** in
  `crates/`.
- **No registered-but-dead checks.**
- **No config keys parsed and then ignored.** `resolve_policy`
  (`config.rs:560`) is careful: unknown check IDs error, `[checks]` is refused
  inside a fleet, `[overrides]` is refused outside one, an override without a
  reason errors, and an override the fleet layer has not marked
  `allow_override` errors.
- Ordnung's three `required` failures against itself are all true (the `site/`
  TypeScript project has no typecheck, build, or CI tasks). The tool is correct
  about its own repository.
