// Tests for `src/checks/conventional_commits.rs`.
use crate::support::*;

#[test]
fn conventional_commits_requires_real_ci_enforcement_and_documentation() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join(".github/workflows/conventional.yml"),
        "name: conventional\non: pull_request\njobs: {}\n",
    )
    .unwrap();
    fs::write(repo.path().join("README.md"), "# Demo\n").unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let conventional = report
        .results
        .iter()
        .find(|result| result.check == "conventional-commits")
        .unwrap();
    assert_eq!(conventional.status, CheckStatus::Fail);
    assert!(conventional.message.contains("no CI enforcement"));
    assert!(conventional.message.contains("not mentioned"));

    fs::write(
        repo.path().join(".github/workflows/conventional.yml"),
        "on: pull_request_target\njobs:\n  title:\n    steps:\n      - uses: amannn/action-semantic-pull-request@v6\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("README.md"),
        "# Demo\n\nPull request titles follow Conventional Commits.\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let conventional = report
        .results
        .iter()
        .find(|result| result.check == "conventional-commits")
        .unwrap();
    assert_eq!(conventional.status, CheckStatus::Pass);
    assert_eq!(
        conventional.scope,
        std::path::Path::new(".github/workflows/conventional.yml")
    );
}
