// Tests for `src/checks/reproducible_toolchain.rs`.
use crate::support::*;

#[test]
fn reproducible_toolchain_rejects_only_unbounded_setup_versions() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        r#"on: push
jobs:
  test:
    steps:
      - uses: actions/setup-node@v4
        with: {node-version: latest}
      - uses: actions/setup-python@v5
        with: {python-version: '3.*'}
      - uses: example/toolchain-action@v1
        with: {version: latest}
"#,
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let toolchain = report
        .results
        .iter()
        .find(|result| result.check == "reproducible-toolchain")
        .unwrap();
    assert_eq!(toolchain.status, CheckStatus::Fail);
    assert!(toolchain.message.contains("node-version: latest"));
    assert!(toolchain.message.contains("python-version: 3.*"));

    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        r#"on: push
jobs:
  test:
    steps:
      - uses: actions/setup-node@v4
        with: {node-version: 20.11.0}
      - uses: actions/setup-go@v5
        with: {go-version: stable}
      - uses: dtolnay/rust-toolchain@stable
"#,
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(report.results.iter().any(|result| {
        result.check == "reproducible-toolchain" && result.status == CheckStatus::Pass
    }));
}
