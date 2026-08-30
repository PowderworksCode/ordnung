// Tests for `src/checks/license.rs`.
use crate::support::*;

#[test]
fn license_requires_an_approved_root_filename() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("nested")).unwrap();
    fs::write(repo.path().join("nested/LICENSE"), "nested terms\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let missing = report
        .results
        .iter()
        .find(|result| result.check == "license")
        .unwrap();
    assert_eq!(missing.status, CheckStatus::Fail);
    assert_eq!(missing.scope, std::path::Path::new("LICENSE"));

    fs::write(repo.path().join("COPYING"), "custom terms\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let present = report
        .results
        .iter()
        .find(|result| result.check == "license")
        .unwrap();
    assert_eq!(present.status, CheckStatus::Pass);
    assert_eq!(present.scope, std::path::Path::new("COPYING"));

    fs::write(repo.path().join("LICENSE.md"), "preferred terms\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let preferred = report
        .results
        .iter()
        .find(|result| result.check == "license")
        .unwrap();
    assert_eq!(preferred.scope, std::path::Path::new("LICENSE.md"));
}
