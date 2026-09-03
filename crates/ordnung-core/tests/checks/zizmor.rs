// Tests for `src/checks/zizmor.rs`.
use crate::support::*;
use crate::support_lint::*;

/// zizmor audits workflows, so a repository with none has nothing to audit.
#[test]
fn skips_a_repository_with_no_workflows() {
    let repo = repo_with(&[("README.md", "# Fixture\n")]);
    assert_eq!(status(repo.path(), "zizmor"), CheckStatus::Skip);
}

#[test]
fn fails_when_workflows_exist_and_none_runs_it() {
    let repo = repo_with(&[CI]);
    assert_eq!(status(repo.path(), "zizmor"), CheckStatus::Fail);
}

#[test]
fn passes_on_the_command() {
    let workflow = lint_workflow("run: uvx zizmor@1.14.2 .");
    let repo = repo_with(&[CI, (".github/workflows/lint.yml", &workflow)]);
    assert_eq!(status(repo.path(), "zizmor"), CheckStatus::Pass);
}

#[test]
fn passes_on_the_action() {
    let workflow = lint_workflow("uses: zizmorcore/zizmor-action@v0.6.2");
    let repo = repo_with(&[CI, (".github/workflows/lint.yml", &workflow)]);
    assert_eq!(status(repo.path(), "zizmor"), CheckStatus::Pass);
}
