// Tests for `src/checks/vale.rs`.
use crate::support::*;
use crate::support_lint::*;

/// Vale is configured by a file at the root, so its absence is the finding —
/// there is no subject to detect the way the other lint checks have one.
#[test]
fn fails_without_a_root_config() {
    let repo = repo_with(&[("README.md", "# Fixture\n"), CI]);
    assert_eq!(status(repo.path(), "vale"), CheckStatus::Fail);
}

/// A StylesPath naming a directory that is not there is a config that cannot
/// run, which is worth separating from one that is simply unenforced.
#[test]
fn fails_when_the_declared_styles_path_is_missing() {
    let repo = repo_with(&[
        (
            ".vale.ini",
            "StylesPath = .vale/styles\nMinAlertLevel = error\n",
        ),
        CI,
    ]);
    assert_eq!(status(repo.path(), "vale"), CheckStatus::Fail);
}

/// A path that climbs out of the repository is refused rather than followed.
#[test]
fn fails_when_the_styles_path_escapes_the_repository() {
    let repo = repo_with(&[(".vale.ini", "StylesPath = ../elsewhere\n"), CI]);
    assert_eq!(status(repo.path(), "vale"), CheckStatus::Fail);
}

#[test]
fn fails_when_configured_but_no_workflow_runs_it() {
    let repo = repo_with(&[
        (".vale.ini", "StylesPath = .vale/styles\n"),
        (".vale/styles/.keep", ""),
        CI,
    ]);
    assert_eq!(status(repo.path(), "vale"), CheckStatus::Fail);
}

#[test]
fn passes_when_configured_and_a_workflow_runs_it() {
    let workflow = lint_workflow("uses: errata-ai/vale-action@v3.0.0");
    let repo = repo_with(&[
        (".vale.ini", "StylesPath = .vale/styles\n"),
        (".vale/styles/.keep", ""),
        CI,
        (".github/workflows/lint.yml", &workflow),
    ]);
    assert_eq!(status(repo.path(), "vale"), CheckStatus::Pass);
}
