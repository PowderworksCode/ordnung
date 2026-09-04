// Tests for `src/checks/lockfiles.rs`.
use crate::support::*;

#[test]
fn lockfile_check_uses_discovered_ownership_once_per_workspace() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[workspace]\nmembers=['crates/member']\n",
    )
    .unwrap();
    fs::write(repo.path().join("Cargo.lock"), "").unwrap();
    fs::create_dir_all(repo.path().join("crates/member")).unwrap();
    fs::write(
        repo.path().join("crates/member/Cargo.toml"),
        "[package]\nname='member'\nversion='0.0.0'\n",
    )
    .unwrap();
    fs::create_dir_all(repo.path().join("tools/standalone")).unwrap();
    fs::write(
        repo.path().join("tools/standalone/Cargo.toml"),
        "[package]\nname='standalone'\nversion='0.0.0'\n",
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_config(repo.path(), &inventory, &TestLayoutConfig::default());
    let lockfiles = report
        .results
        .iter()
        .filter(|result| result.check == "lockfiles")
        .collect::<Vec<_>>();
    assert_eq!(lockfiles.len(), 2);
    assert!(lockfiles.iter().any(|result| {
        result.scope.as_os_str().is_empty() && result.status == CheckStatus::Pass
    }));
    assert!(lockfiles.iter().any(|result| {
        result.scope == std::path::Path::new("tools/standalone")
            && result.status == CheckStatus::Fail
    }));
}
