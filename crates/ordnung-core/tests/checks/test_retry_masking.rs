// Tests for `src/checks/test_retry_masking.rs`.
use crate::support::*;

#[test]
fn retry_masking_uses_tool_profiles_for_commands_and_configs() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(
        repo.path().join("playwright.config.ts"),
        "export default { retries: 2 };\n",
    )
    .unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join(".github/workflows/test.yml"),
        "on: pull_request\njobs:\n  test:\n    steps:\n      - run: npx playwright test --retries=2\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let retry = report
        .results
        .iter()
        .find(|result| result.check == "test-retry-masking")
        .unwrap();
    assert_eq!(retry.status, CheckStatus::Fail);
    assert!(retry.message.contains("playwright.config.ts"));
    assert!(retry.message.contains("test.yml:test"));

    fs::write(
        repo.path().join("playwright.config.ts"),
        "export default { retries: 0 };\n",
    )
    .unwrap();
    fs::write(
        repo.path().join(".github/workflows/test.yml"),
        "on: pull_request\njobs:\n  test:\n    steps:\n      - run: npx playwright test\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(report.results.iter().any(|result| {
        result.check == "test-retry-masking" && result.status == CheckStatus::Pass
    }));
}
