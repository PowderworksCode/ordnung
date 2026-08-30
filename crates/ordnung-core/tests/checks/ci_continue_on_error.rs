// Tests for `src/checks/ci_continue_on_error.rs`.
use crate::support::*;

#[test]
fn ci_continue_on_error_only_flags_jobs_and_gating_steps() {
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
    steps:
      - name: Run tests
        run: cargo test
        continue-on-error: true
      - name: Upload coverage
        uses: codecov/codecov-action@v4
        continue-on-error: true
  lint:
    name: Lint
    continue-on-error: true
    steps:
      - run: cargo clippy
"#,
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let masking = report
        .results
        .iter()
        .find(|result| result.check == "ci-continue-on-error")
        .unwrap();
    assert_eq!(masking.status, CheckStatus::Fail);
    assert!(masking.message.contains("Run tests"));
    assert!(masking.message.contains("job 'Lint'"));
    assert!(!masking.message.contains("Upload coverage"));
}
