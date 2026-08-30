// Tests for `src/checks/stray_files.rs`.
use crate::support::*;

#[test]
fn stray_files_use_configured_corrals() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(repo.path().join("ROADMAP.md"), "# Roadmap\n").unwrap();
    fs::write(repo.path().join("TODOS.md"), "# Todos\n").unwrap();
    fs::write(repo.path().join("todo.txt"), "TODO: centralized\n").unwrap();
    let config =
        RepoConfig::parse("ordnung.toml", "[stray_files]\nallow = ['ROADMAP.md']\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report = run_repository_checks_with_repo_config(repo.path(), &inventory, &config);
    let files = report
        .results
        .iter()
        .find(|result| result.check == "stray-files")
        .unwrap();
    assert_eq!(files.status, CheckStatus::Fail);
    assert!(files.message.contains("TODOS.md"));
    assert!(!files.message.contains("ROADMAP.md"));
}
