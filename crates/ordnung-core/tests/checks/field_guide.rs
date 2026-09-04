// Tests for `src/checks/field_guide.rs`.
use crate::support::*;

#[test]
fn field_guide_can_live_anywhere_in_the_repository() {
    let repo = tempfile::tempdir().unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let missing = report
        .results
        .iter()
        .find(|result| result.check == "field-guide")
        .unwrap();
    assert_eq!(missing.status, CheckStatus::Fail);
    assert_eq!(missing.severity, Severity::Off);
    assert_eq!(missing.scope, std::path::Path::new("notes/field_guide.md"));

    fs::create_dir_all(repo.path().join("knowledge")).unwrap();
    fs::write(
        repo.path().join("knowledge/field_guide.md"),
        "# Agent Field Guide\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let present = report
        .results
        .iter()
        .find(|result| result.check == "field-guide")
        .unwrap();
    assert_eq!(present.status, CheckStatus::Pass);
    assert_eq!(
        present.scope,
        std::path::Path::new("knowledge/field_guide.md")
    );
}
