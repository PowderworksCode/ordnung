// Tests for `src/checks/hawk.rs`.
use crate::support::*;
use crate::support_lint::*;

/// Hawk reads a Rust public API. A repository without one is not failing to run
/// it, so the check has nothing to say rather than something to complain about.
#[test]
fn skips_a_repository_with_no_rust() {
    let repo = repo_with(&[("README.md", "# Fixture\n")]);
    assert_eq!(status(repo.path(), "hawk"), CheckStatus::Skip);
}

#[test]
fn fails_when_rust_is_present_and_no_workflow_runs_it() {
    let repo = repo_with(&[CARGO, LIB, CI]);
    assert_eq!(status(repo.path(), "hawk"), CheckStatus::Fail);
}

/// The pinned toolchain is part of the invocation: hawk builds against a
/// compiler version rather than tracking stable, so the pin is how it is run.
#[test]
fn passes_when_a_workflow_runs_cargo_hawk() {
    let workflow = lint_workflow("run: cargo +1.98.0 hawk check -D warnings");
    let repo = repo_with(&[CARGO, LIB, CI, (".github/workflows/lint.yml", &workflow)]);
    assert_eq!(status(repo.path(), "hawk"), CheckStatus::Pass);
}
