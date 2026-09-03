// Tests for `src/checks/shellcheck.rs`.
use crate::support::*;
use crate::support_lint::*;

/// No shell in the repository, nothing to check.
#[test]
fn skips_a_repository_with_no_shell_scripts() {
    let repo = repo_with(&[("README.md", "# Fixture\n")]);
    assert_eq!(status(repo.path(), "shellcheck"), CheckStatus::Skip);
}

#[test]
fn fails_when_a_script_is_present_and_no_workflow_runs_it() {
    let repo = repo_with(&[("scripts/dev.sh", "#!/bin/sh\necho fixture\n"), CI]);
    assert_eq!(status(repo.path(), "shellcheck"), CheckStatus::Fail);
}

#[test]
fn passes_on_the_registered_action() {
    let workflow = lint_workflow("uses: ludeeus/action-shellcheck@2.0.0");
    let repo = repo_with(&[
        ("scripts/dev.sh", "#!/bin/sh\necho fixture\n"),
        CI,
        (".github/workflows/lint.yml", &workflow),
    ]);
    assert_eq!(status(repo.path(), "shellcheck"), CheckStatus::Pass);
}

/// The command counts as much as the action: what matters is that ShellCheck
/// runs, not which packaging of it a repository chose.
#[test]
fn passes_on_the_command() {
    let workflow = lint_workflow("run: shellcheck --severity=warning scripts/dev.sh");
    let repo = repo_with(&[
        ("scripts/dev.sh", "#!/bin/sh\necho fixture\n"),
        CI,
        (".github/workflows/lint.yml", &workflow),
    ]);
    assert_eq!(status(repo.path(), "shellcheck"), CheckStatus::Pass);
}
