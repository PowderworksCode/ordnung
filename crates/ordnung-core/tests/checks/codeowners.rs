// Tests for `src/checks/codeowners.rs`.
use crate::support::*;

#[test]
fn codeowners_requires_a_rule_that_assigns_an_owner() {
    let repo = tempfile::tempdir().unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let missing = report
        .results
        .iter()
        .find(|result| result.check == "codeowners")
        .unwrap();
    assert_eq!(missing.status, CheckStatus::Fail);
    assert!(missing.message.contains("no CODEOWNERS file"));

    fs::write(repo.path().join("CODEOWNERS"), "# defaults\n/apps/github\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let unowned = report
        .results
        .iter()
        .find(|result| result.check == "codeowners")
        .unwrap();
    assert_eq!(unowned.status, CheckStatus::Fail);
    assert!(unowned.message.contains("no rules that assign an owner"));

    fs::write(repo.path().join("CODEOWNERS"), "* @org/maintainers\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let owned = report
        .results
        .iter()
        .find(|result| result.check == "codeowners")
        .unwrap();
    assert_eq!(owned.status, CheckStatus::Pass);
    assert_eq!(owned.scope, std::path::Path::new("CODEOWNERS"));
}

#[test]
fn codeowners_rejects_invalid_github_syntax() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(repo.path().join("CODEOWNERS"), "!generated/ @owner\n").unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let invalid = report
        .results
        .iter()
        .find(|result| result.check == "codeowners")
        .unwrap();
    assert_eq!(invalid.status, CheckStatus::Fail);
    assert!(invalid.message.contains("negated patterns"));
}
