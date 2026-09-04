// Tests for `src/checks/ci_exists.rs`.
use crate::support::*;

#[test]
fn ci_requires_rust_test_lint_and_format_tasks_on_changes() {
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
        "on: [push, pull_request]\njobs:\n  quality:\n    steps:\n      - run: cargo test && cargo clippy && cargo fmt --check\n",
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_config(repo.path(), &inventory, &TestLayoutConfig::default());
    assert!(
        report
            .results
            .iter()
            .any(|result| { result.check == "ci-exists" && result.status == CheckStatus::Pass })
    );

    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        "on: push\njobs:\n  quality:\n    steps:\n      - run: cargo test && cargo fmt --check\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_config(repo.path(), &inventory, &TestLayoutConfig::default());
    assert!(report.results.iter().any(|result| {
        result.check == "ci-exists"
            && result.status == CheckStatus::Fail
            && result.message.contains("lint")
    }));
}

#[test]
fn ci_rejects_invalid_workflow_files() {
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
        repo.path().join(".github/workflows/broken.yml"),
        "jobs: [\n",
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(report.results.iter().any(|result| {
        result.check == "ci-exists"
            && result.status == CheckStatus::Fail
            && result.scope == std::path::Path::new(".github/workflows/broken.yml")
            && result.message.contains("invalid YAML")
    }));
}

#[test]
fn ci_ignore_only_exempts_matching_project_instances() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::create_dir_all(repo.path().join("spikes/demo/src")).unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname='fixture'\nversion='0.0.0'\n",
    )
    .unwrap();
    fs::write(repo.path().join("src/lib.rs"), "pub fn fixture() {}\n").unwrap();
    fs::write(
        repo.path().join("spikes/demo/package.json"),
        r#"{"devDependencies":{"typescript":"1.0.0"}}"#,
    )
    .unwrap();
    fs::write(repo.path().join("spikes/demo/tsconfig.json"), "{}\n").unwrap();
    fs::write(repo.path().join("spikes/demo/src/index.ts"), "export {};\n").unwrap();
    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        "on: pull_request\njobs:\n  rust:\n    steps:\n      - run: cargo test && cargo clippy && cargo fmt --check\n",
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let without_ignore =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(without_ignore.results.iter().any(|result| {
        result.check == "ci-exists"
            && result.status == CheckStatus::Fail
            && result.message.starts_with("typescript CI is missing")
    }));

    let config = RepoConfig {
        ci_exists: CiExistsConfig {
            ignore: vec!["spikes".into()],
        },
        ..RepoConfig::default()
    };
    let report = run_repository_checks_with_repo_config(repo.path(), &inventory, &config);
    let ci = report
        .results
        .iter()
        .filter(|result| result.check == "ci-exists")
        .collect::<Vec<_>>();
    assert!(ci.iter().all(|result| result.status == CheckStatus::Pass));
    assert!(
        ci.iter()
            .all(|result| result.message.contains("typescript at spikes/demo"))
    );
}

/// The same rule the type layer follows: a language a project never declared
/// is not a language it owes CI tasks for. The grammar crate's grammar.js does
/// not put a JavaScript toolchain on the repository's bill.
#[test]
fn ci_exists_grades_only_languages_a_package_declares() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("crates/grammar/src")).unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/grammar\"]\nresolver = \"2\"\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("crates/grammar/Cargo.toml"),
        "[package]\nname = \"grammar\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("crates/grammar/src/lib.rs"),
        "pub fn g() {}\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("crates/grammar/grammar.js"),
        "module.exports = grammar({ name: 'g', rules: {} });\n",
    )
    .unwrap();
    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        "on: pull_request\njobs:\n  rust:\n    timeout-minutes: 20\n    steps:\n      - run: cargo test\n      - run: cargo clippy\n      - run: cargo fmt --check\n",
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let failures = report
        .results
        .iter()
        .filter(|result| result.check == "ci-exists" && result.status == CheckStatus::Fail)
        .map(|result| result.message.clone())
        .collect::<Vec<_>>();
    assert!(
        failures.is_empty(),
        "the Rust workspace satisfies its own tasks and owes no JavaScript ones: {failures:?}"
    );
}
