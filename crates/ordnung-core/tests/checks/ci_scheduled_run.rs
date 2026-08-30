// Tests for `src/checks/ci_scheduled_run.rs`.
use crate::support::*;
use crate::support_lint::*;

/// A repository with no workflows has no schedule to keep.
#[test]
fn skips_without_workflows() {
    let repo = repo_with(&[("README.md", "# Fixture\n")]);
    assert_eq!(status(repo.path(), "ci-scheduled-run"), CheckStatus::Skip);
}

/// Validation that only runs on change never notices the world moving under a
/// repository nobody has touched — which is the whole point of the schedule.
#[test]
fn fails_when_nothing_runs_periodically() {
    let repo = repo_with(&[CI]);
    assert_eq!(status(repo.path(), "ci-scheduled-run"), CheckStatus::Fail);
}

#[test]
fn passes_on_a_cron_trigger() {
    let repo = repo_with(&[(
        ".github/workflows/ci.yml",
        "on:\n  pull_request:\n  schedule:\n    - cron: \"23 5 * * 1\"\njobs:\n  build:\n    steps:\n      - run: cargo test\n",
    )]);
    assert_eq!(status(repo.path(), "ci-scheduled-run"), CheckStatus::Pass);
}
