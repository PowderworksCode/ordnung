// Tests for `src/checks/stylelint.rs`.
use crate::support::*;

#[test]
fn stylelint_requires_typed_configuration_and_change_workflow() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::write(
        repo.path().join("package.json"),
        r#"{"devDependencies":{"typescript":"1.0.0"}}"#,
    )
    .unwrap();
    fs::write(repo.path().join("src/site.css"), "body { color: black; }\n").unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let missing = report
        .results
        .iter()
        .find(|result| result.check == "stylelint")
        .unwrap();
    assert_eq!(missing.status, CheckStatus::Fail);
    assert!(missing.message.contains("no Stylelint configuration"));

    fs::write(repo.path().join(".stylelintrc.json"), "{}\n").unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join(".github/workflows/style.yml"),
        "on: pull_request\njobs:\n  lint:\n    steps:\n      - run: npx stylelint '**/*.css'\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(
        report
            .results
            .iter()
            .any(|result| result.check == "stylelint" && result.status == CheckStatus::Pass)
    );
}
