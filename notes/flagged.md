# Flagged for decision

Things that are too complicated, not finished, or would surprise a user.
**You decide these; nothing here has been changed.** Each item states what is
true today, why it matters, and the options.

Ordered by what a new user hits first.

---

## A. Correctness and safety

### A1. `ordnung check` silently runs 33 of 47 checks — four of them `required`

**Today.** `check` executes only checks with a repository runner. The 14
GitHub-backed checks need `repo-check` or `github check` with a `--repo`.
Nothing in the output says so. Four of the fourteen are `required`:

| Missing from `check` | Severity |
| --- | --- |
| `branch-protection` | required |
| `ci-green` | required |
| `secret-scanning` | required |
| `workflow-permissions` | required |
| + 10 others | recommended / off |

**Why it matters.** `ordnung check .` exiting `0` reads as "this repository is
in order." It means "the 33 local checks passed." Running it against
`~/powderworks/straitjacket` exits `0` today — with branch protection, secret
scanning and workflow permissions never evaluated. For a published tool whose
value proposition is "know which repositories are in order," this is the most
serious issue in the list.

**Options.**
1. **Print a footer whenever GitHub checks were not run** — *recommended*.
   `note: 14 GitHub-backed checks were not run; use 'ordnung repo-check .
   --repo owner/name'`. One line, no behaviour change.
2. Make `check` auto-detect the remote from `git remote -v` and run the full
   set when `gh` is available, falling back with a warning.
3. Rename: `check` → `check --local`, and make bare `check` mean the full audit.
   Cleanest semantics, breaking change.

### A2. `ordnung check . | head` panics

**Today.** Any early-closed pipe kills the process with a Rust panic:

```console
$ ordnung check . | head -3
thread 'main' panicked at library/std/src/io/stdio.rs:1166:9:
failed printing to stdout: Broken pipe (os error 32)
$ echo ${PIPESTATUS[0]}
101
```

**Why it matters.** With 115 lines of output and no filtering flag, piping into
`head`, `less`, or `grep -m1` is the obvious thing to do, and it crashes. This
is the single worst first impression the tool makes.

**Options.**
1. **Restore default `SIGPIPE` at startup** — *recommended*. Three lines,
   no dependency, standard for CLIs.
2. Handle `BrokenPipe` on every write path and exit `0`. More code, same effect.
3. Leave it. Not defensible for a published binary.

### A3. `fleet github-sync --apply` force-pushes a fixed branch name

**Today.** `materialize_pull_request` (`gh.rs:287`) issues
`PATCH .../git/refs/heads/ordnung/remediation` with `"force": true`, re-parented
onto the current default branch tip. `REMEDIATION_BRANCH` (`gh.rs:20`) is a
constant with no CLI or config override.

**Why it matters.** Any commit anyone pushes to a branch named
`ordnung/remediation` is destroyed without warning or confirmation. Within a
fleet you control this is a defensible convention. As a published tool that
writes to repositories, it needs to be loud, and the name needs to be
changeable.

**Options.**
1. **Document it plainly and make the branch name configurable** —
   *recommended*. Documented in the new README's *Consent and writes* section
   already; the config key is the open part.
2. Additionally refuse to force-push when the branch's tip is not a commit
   Ordnung authored. Safest; needs commit provenance tracking.
3. Leave as is.

### A4. `--apply` conflates two very different blast radii

**Today.** In `sync_fleet_member` (`main.rs:908–915`), a single `--apply` both
calls `apply_setting_changes` — which mutates live repository settings
immediately through the API — and opens or updates a pull request. The settings
change has no review step; the file changes do.

**Why it matters.** There is no way to say "propose the file changes, don't touch
my settings." `fleet github-sync-all --apply` does this for every member in one
invocation.

**Options.**
1. **Split into `--apply-files` and `--apply-settings`**, with `--apply`
   meaning both — *recommended*. Backwards compatible.
2. Require `--apply-settings` explicitly; `--apply` covers only the PR.
   Safer, mildly breaking.
3. Add an interactive confirmation for settings changes when stdin is a TTY.

---

## B. Output quality

### B1. `off`-severity checks print as `fail` — 76 of 84 "failures" on this repo

**Today.** `off` is a severity label, not an execution switch. Checks set to
`off` still run, still emit findings, and still print `fail` in column one,
identical in weight to a real failure.

| `ordnung check .` on this repo | count |
| --- | --- |
| `fail` `off` | **76** |
| `fail` `recommended` | 5 |
| `fail` `required` | 3 |
| pass / skip | 31 |

**You have already chosen option 1 below.** Recorded here for the record and
because it is not yet implemented.

**Options.**
1. **`off` means silent: suppress from default output, add `--all`** —
   *chosen*. Drops this repo's output from 115 lines to ~39 and its "failures"
   from 84 to 8. Also lets the runner skip them entirely, which is a
   performance win at 150 checks.
2. Print them under a separate "informational" heading, never labelled `fail`.
3. Ship a default profile that removes them from the run; leave the engine.

### B2. `test-mirror` emits one line per source file — 60 lines on this repo

**Today.** Every check reports per project or per repository. `test-mirror` is
the exception: one finding per source file. On this repository that is 60 of
115 output lines; on `~/powderworks/entl`, over 100.

**Options.**
1. **Aggregate to one finding per project**, listing counts and a sample —
   *recommended*, and matches every other check's behaviour.
2. Cap at N with "+ 47 more", as `pinned-dependencies` already does.
3. Nothing, if B1 lands — `test-mirror` defaults to `off`, so suppression hides
   it. This fixes the symptom on default settings and leaves it for anyone who
   turns the check on.

### B3. No summary line and no severity filter

**Today.** Output ends on the last check. There is no count, no verdict, no
statement of why the exit code is what it is. `check` has exactly two flags:
`--json` and `--help`.

**Options.**
1. **Add a summary footer and a `--severity` / `--fail-on` filter** —
   *recommended*. Additive; nothing existing changes.
2. Summary only.
3. Neither; document `--json` piping as the answer.

### B4. `readme-quality` counts fenced code blocks as prose — **not a defect**

Raised because the 150–1,500 word window counts every word in the file, and a
README carrying a real sample of Ordnung's output kept crossing the ceiling.

**Decided: working as intended.** The README is meant to be short. Detail belongs
in `docs/`, or does not belong at all. The ceiling is the mechanism that enforces
that, and code blocks counting toward it is part of why it works. No change.

---

## C. Distribution

### C1. The GitHub Action built from source on every run — **done**

**Was.** `scripts/action.sh` ran `cargo install --path` unless `ORDNUNG_BIN` was
set, and no release workflow existed, so there was no binary to download. Every
consumer paid a full Rust build, in their CI minutes, on every invocation.

**Done.** The Action now resolves a binary in three steps: `ORDNUNG_BIN`, then a
published release binary matching its pinned tag and the runner's platform
(checksum-verified), then a source build. Every failure in the middle step is
recoverable and reports why, so nothing that used to work stops working.

`.github/workflows/release.yml` drafts a release on a pushed `v*` tag, builds
`x86_64-unknown-linux-gnu`, `x86_64-apple-darwin` and `aarch64-apple-darwin`,
uploads an archive and `.sha256` per platform, and publishes only once every
platform has succeeded.

**Still open.** `aarch64-unknown-linux-gnu` is not published: it needs a cross
linker or an ARM runner. Those runners fall back to a source build. Worth adding
if anyone runs Ordnung on ARM Linux CI.

**Note.** No tag has been pushed and no release exists, so today every consumer
still gets the source build. The mechanism is in place for the first release;
publishing remains your call.

### C2. The Action was never exercised end to end — **done**

**Correction to what this section first said.** It claimed no test covered
`scripts/action.sh`. That was wrong: `tests/cli.rs` has three
`action_wrapper_*` tests that drive the script with a stub `ORDNUNG_BIN` and
assert the arguments it builds, covering the default mode, `fleet-sync-all`, and
input validation.

What was genuinely untested was `action.yml` itself — the inputs, the
environment it assembles, and the outputs it reports — because no workflow
referenced `./`. The `cargo install` path had never run in CI either.

**Done.** `ci.yml` gained an `action` job that runs the Action the way a consumer
does: once against a generated fixture repository that Ordnung finds clean
(asserting `outcome=clean`, `exit-code=0`), and once against this repository
(asserting `outcome=drift`, `exit-code=1`). The second run sets `ORDNUNG_BIN` to
the binary the first installed, which skips a redundant build and covers that
escape hatch too.

Still uncovered: `fix`, `github-check`, `fleet-check`, and `fleet-sync` modes,
which need GitHub credentials or a fleet manifest.

### C3. crates.io is blocked by more than the git dependency

**Today.** Three independent blockers, in increasing cost:

| Blocker | Cost |
| --- | --- |
| Neither crate has a `description` field, which crates.io requires | minutes |
| Both crates are `version = "0.0.0"` | minutes |
| **The name `ordnung` is taken on crates.io** — registered 2020, 5,060 downloads, an unrelated vector-map crate by `maciejhirsz`. Dormant, but genuine and not squatted | needs a new published name |
| **crates.io rejects git dependencies.** `entl-codebase` and `entl-github` are git-pinned. entl being public does not help; it must itself be *published to crates.io* | weeks, and it is another repository |

**Note.** Making the repository public is blocked by none of this. A git
dependency on a public repository resolves for anyone who clones or runs
`cargo install --git`.

**Status:** you have chosen public-repo-now, crates.io later. Recorded so the
sequencing is not rediscovered.

**Options if crates.io is revisited.**
1. Publish `entl-codebase` and `entl-github` from the entl repository, then
   publish ordnung under an available name (`ordnung-cli` is free today).
2. Vendor the entl sources into this repository. Removes the dependency but
   forks the code.
3. Feature-gate the entl-dependent paths so a reduced crate can publish.
   Complex, and the entl-dependent paths are most of the product.

---

## D. Rough edges

### D1. Inconsistent argument shapes across subcommands

**Today.** The same concept is spelled four ways:

```
ordnung repo-check [PATH] --repo <REPO>
ordnung github inspect <REPO>
ordnung github check --repo-root <REPO_ROOT> <REPO>
ordnung fleet github-sync <FLEET> --repo <REPO>
```

Repository is sometimes positional, sometimes `--repo`; path is sometimes
positional, sometimes `--repo-root`. Discovered by getting it wrong twice in
five minutes of first use.

**Options.** 1. Normalise to `<PATH>` positional + `--repo owner/name`
everywhere, keeping the old spellings as hidden aliases — *recommended*.
2. Normalise without aliases. 3. Leave; document precisely.

### D2. Two undocumented environment variables

`ORDNUNG_GH` (`gh.rs:45`) selects the `gh` binary; `ORDNUNG_BIN`
(`scripts/action.sh:43`) skips the Action's build. Neither appeared in any
Markdown before this branch. Both are genuinely useful — `ORDNUNG_GH` is how the
`gh` boundary gets tested.

**Options.** 1. **Document both** — *recommended*, done for `ORDNUNG_GH` in the
new README and `ORDNUNG_BIN` in the Action section. 2. Add an `--gh-binary`
flag as the supported surface and keep the env var as fallback.

### D3. `fix` almost never has anything to offer

**Today.** 84 findings on this repository produce zero remediations. 21 findings
on `~/powderworks/crawldb` produce one (`create CHANGELOG.md`).

This is *by design* — `fix` refuses to guess — and the design is right. The
problem is presentational: a user runs it once, reads `no automatic
remediations are available`, and never runs it again.

**Options.** 1. **Report the ratio**: `no automatic remediations available (0
of 84 findings are exactly fixable)` — *recommended*. Explains rather than
stonewalls. 2. List which checks *can* self-fix, so expectations are set.
3. Leave.

### D4. `CheckScope` is declared 47 times and read only by a test

See §4e of the refactoring review. Either implement the directory-selection
validation its doc comment promises, or delete the field.

---

## E. Not problems

Checked because the brief asked, and clean:

- **No half-built checks.** All 47 registered checks have a runner.
- **No TODO/FIXME/`todo!()`/`unimplemented!()`** anywhere in `crates/`.
- **No config keys parsed and ignored.** `resolve_policy` (`config.rs:560`)
  rejects unknown IDs, `[checks]` inside a fleet, `[overrides]` outside one,
  reasonless overrides, and overrides the fleet has not permitted.
- **The `gh` shell-out is properly isolated** behind `GhRunner` and tested with a
  fake. The brief's concern does not apply.
- **`tests/check.rs` is not hiding untested seams** — 60 independent integration
  tests, only `project-inventory` unreferenced by ID.
- **Ordnung's three `required` failures against itself are all true.** The
  `site/` TypeScript project genuinely has no typecheck, build or CI task.
