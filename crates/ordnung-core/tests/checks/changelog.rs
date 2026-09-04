// Tests for `src/checks/changelog.rs`.
use crate::support::*;

#[test]
fn changelog_requires_an_approved_root_filename() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("docs")).unwrap();
    fs::write(repo.path().join("docs/CHANGELOG.md"), "# Changes\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert_eq!(
        report
            .results
            .iter()
            .find(|result| result.check == "changelog")
            .unwrap()
            .status,
        CheckStatus::Fail
    );
    fs::write(repo.path().join("HISTORY.md"), "# History\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert_eq!(
        report
            .results
            .iter()
            .find(|result| result.check == "changelog")
            .unwrap()
            .scope,
        std::path::Path::new("HISTORY.md")
    );
}
