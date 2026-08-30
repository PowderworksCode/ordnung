// Tests for `src/checks/codespell.rs`.
use crate::support::*;

#[test]
fn documentation_tools_require_typed_change_workflows_and_vale_styles() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::create_dir_all(repo.path().join("styles")).unwrap();
    fs::write(repo.path().join("styles/.gitkeep"), "").unwrap();
    fs::write(
        repo.path().join(".vale.ini"),
        "StylesPath = styles\n[*.md]\nBasedOnStyles = Vale\n",
    )
    .unwrap();
    fs::write(repo.path().join(".github/workflows/docs.yml"), "on: pull_request\njobs:\n  docs:\n    steps:\n      - uses: codespell-project/actions-codespell@v2\n      - uses: errata-ai/vale-action@reviewdog\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    for check in ["codespell", "vale"] {
        assert_eq!(
            report
                .results
                .iter()
                .find(|result| result.check == check)
                .unwrap()
                .status,
            CheckStatus::Pass,
            "{check}"
        );
    }
    fs::write(repo.path().join(".vale.ini"), "StylesPath = missing\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert_eq!(
        report
            .results
            .iter()
            .find(|result| result.check == "vale")
            .unwrap()
            .status,
        CheckStatus::Fail
    );
}
