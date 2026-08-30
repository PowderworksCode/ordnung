// Tests for `src/checks/ci_scoped.rs`.
use crate::support::*;

#[test]
fn heavy_pull_request_jobs_require_structured_scoping() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = 'fixture'\nversion = '0.0.0'\n",
    )
    .unwrap();
    fs::write(repo.path().join("src/lib.rs"), "pub fn value() {}\n").unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        "on: pull_request\njobs:\n  test:\n    steps:\n      - run: cargo test\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(
        report
            .results
            .iter()
            .any(|result| result.check == "ci-scoped" && result.status == CheckStatus::Fail)
    );

    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        "on:\n  pull_request:\n    paths: ['src/**', 'Cargo.toml']\njobs:\n  test:\n    steps:\n      - run: cargo test\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(
        report
            .results
            .iter()
            .any(|result| result.check == "ci-scoped" && result.status == CheckStatus::Pass)
    );
}
