// Tests for `src/checks/gitignore.rs`.
use crate::support::*;

#[test]
fn gitignore_requires_cargo_build_output_at_its_owner() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.0.0\"\n",
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let gitignore = report
        .results
        .iter()
        .find(|result| result.check == "gitignore")
        .unwrap();
    assert_eq!(gitignore.status, CheckStatus::Fail);
    assert!(gitignore.message.contains("target/"));

    fs::write(repo.path().join(".gitignore"), "/target\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(
        report
            .results
            .iter()
            .any(|result| { result.check == "gitignore" && result.status == CheckStatus::Pass })
    );
}

#[test]
fn gitignore_honors_nested_anchoring_inheritance_and_negation() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("apps/web")).unwrap();
    fs::write(
        repo.path().join("apps/web/package.json"),
        r#"{"name":"web","private":true}"#,
    )
    .unwrap();

    fs::write(repo.path().join(".gitignore"), "/node_modules/\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(report.results.iter().any(|result| {
        result.check == "gitignore"
            && result.status == CheckStatus::Fail
            && result.message.contains("apps/web")
    }));

    fs::write(repo.path().join(".gitignore"), "node_modules/\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(
        report
            .results
            .iter()
            .any(|result| { result.check == "gitignore" && result.status == CheckStatus::Pass })
    );

    fs::write(
        repo.path().join(".gitignore"),
        "node_modules/\n!apps/web/node_modules/\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(
        report
            .results
            .iter()
            .any(|result| { result.check == "gitignore" && result.status == CheckStatus::Fail })
    );

    fs::write(repo.path().join(".gitignore"), "*.log\n").unwrap();
    fs::write(repo.path().join("apps/web/.gitignore"), "/node_modules/\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(
        report
            .results
            .iter()
            .any(|result| { result.check == "gitignore" && result.status == CheckStatus::Pass })
    );
}
