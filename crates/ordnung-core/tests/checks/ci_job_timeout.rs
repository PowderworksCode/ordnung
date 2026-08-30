// Tests for `src/checks/ci_job_timeout.rs`.
use crate::support::*;

#[test]
fn scheduled_and_timeout_checks_use_typed_workflow_jobs() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname='fixture'\nversion='0.0.0'\n",
    )
    .unwrap();
    fs::write(repo.path().join("src/lib.rs"), "pub fn fixture() {}\n").unwrap();
    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        r#"on: pull_request
jobs:
  test:
    steps: [{run: cargo test}]
"#,
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(report.results.iter().any(|result| {
        result.check == "ci-scheduled-run" && result.status == CheckStatus::Fail
    }));
    assert!(report.results.iter().any(|result| {
        result.check == "ci-job-timeout"
            && result.status == CheckStatus::Fail
            && result.message.contains("test")
    }));

    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        r#"on: {pull_request: {}, schedule: [{cron: '0 7 * * 1'}]}
jobs:
  test:
    timeout-minutes: 10
    steps: [{run: cargo test}]
  reusable:
    uses: ./.github/workflows/reusable.yml
"#,
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(report.results.iter().any(|result| {
        result.check == "ci-scheduled-run" && result.status == CheckStatus::Pass
    }));
    assert!(
        report.results.iter().any(|result| {
            result.check == "ci-job-timeout" && result.status == CheckStatus::Pass
        })
    );
}
