// Tests for `src/checks/pinned_actions.rs`.
use crate::support::*;

#[test]
fn pinned_actions_and_dependencies_report_separately() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(
        repo.path().join("package.json"),
        r#"{"dependencies":{"exact":"1.2.3","floating":"^2.0.0","local":"workspace:*"},"peerDependencies":{"peer":"^3.0.0"}}"#,
    )
    .unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        "on: push\njobs:\n  ci:\n    steps:\n      - uses: actions/checkout@v4\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let find = |id: &str| {
        report
            .results
            .iter()
            .find(|result| result.check == id)
            .unwrap_or_else(|| panic!("{id} runs"))
    };
    // The security-relevant half is required; the package half is advisory.
    let actions = find("pinned-actions");
    assert_eq!(actions.status, CheckStatus::Fail);
    assert_eq!(actions.severity, Severity::Required);
    assert!(actions.message.contains("actions/checkout@v4"));
    assert!(!actions.message.contains("floating ^2.0.0"));

    let dependencies = find("pinned-dependencies");
    assert_eq!(dependencies.status, CheckStatus::Fail);
    assert_eq!(dependencies.severity, Severity::Recommended);
    assert!(dependencies.message.contains("floating ^2.0.0"));
    assert!(!dependencies.message.contains("actions/checkout"));
    assert!(!dependencies.message.contains("local"));
    assert!(!dependencies.message.contains("peer"));
}

#[test]
fn cargo_ranges_are_advisory_and_action_channels_are_allowed() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = 'fixture'\nversion = '0.0.0'\n[dependencies]\nserde = '1'\n",
    )
    .unwrap();
    fs::write(repo.path().join("Cargo.lock"), "").unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::write(repo.path().join("src/lib.rs"), "").unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join(".github/workflows/docs.yml"),
        "on: push\njobs:\n  docs:\n    steps:\n      - uses: errata-ai/vale-action@stable\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let dependencies = report
        .results
        .iter()
        .find(|result| result.check == "pinned-dependencies")
        .unwrap();
    assert_eq!(dependencies.status, CheckStatus::Pass);
    assert!(dependencies.message.contains("Cargo advisory"));

    let actions = report
        .results
        .iter()
        .find(|result| result.check == "pinned-actions")
        .unwrap();
    assert_eq!(actions.status, CheckStatus::Pass);
    assert!(actions.message.contains("allowed release channel"));
}
