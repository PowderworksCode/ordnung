// Tests for `src/checks/readme.rs`.
use crate::support::*;

#[test]
fn readme_requires_existence_and_a_title_while_quality_judges_the_shape() {
    let repo = tempfile::tempdir().unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let missing = report
        .results
        .iter()
        .find(|result| result.check == "readme")
        .unwrap();
    assert_eq!(missing.status, CheckStatus::Fail);
    assert!(missing.message.contains("no root README"));

    fs::write(repo.path().join("README.md"), "# Demo\n\nIt works.\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let floor = report
        .results
        .iter()
        .find(|result| result.check == "readme")
        .unwrap();
    assert_eq!(floor.status, CheckStatus::Pass, "{}", floor.message);
    let thin = report
        .results
        .iter()
        .find(|result| result.check == "readme-quality")
        .unwrap();
    assert_eq!(thin.status, CheckStatus::Fail);
    for problem in [
        "under 150 words",
        "install/getting-started",
        "usage/docs",
        "License section",
        "Contributing section",
    ] {
        assert!(thin.message.contains(problem), "missing {problem:?}");
    }

    fs::create_dir_all(repo.path().join("docs")).unwrap();
    fs::write(repo.path().join("docs/guide.md"), "# Guide\n").unwrap();
    fs::write(
        repo.path().join("README.md"),
        complete_readme("docs/guide.md?view=full#usage"),
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let complete = report
        .results
        .iter()
        .find(|result| result.check == "readme")
        .unwrap();
    assert_eq!(complete.status, CheckStatus::Pass, "{}", complete.message);
}
