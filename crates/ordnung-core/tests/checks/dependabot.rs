// Tests for `src/checks/dependabot.rs`.
use crate::support::*;

#[test]
fn dependabot_covers_each_nested_package_directory() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(repo.path().join("package.json"), r#"{"private":true}"#).unwrap();
    fs::create_dir_all(repo.path().join("apps/web")).unwrap();
    fs::write(
        repo.path().join("apps/web/package.json"),
        r#"{"name":"web","private":true}"#,
    )
    .unwrap();
    fs::create_dir_all(repo.path().join(".github")).unwrap();
    fs::write(
        repo.path().join(".github/dependabot.yml"),
        "version: 2\nupdates:\n  - package-ecosystem: npm\n    directory: /\n    schedule: { interval: weekly }\n",
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(report.results.iter().any(|result| {
        result.check == "dependabot"
            && result.status == CheckStatus::Fail
            && result.message.contains("/apps/web")
    }));

    fs::write(
        repo.path().join(".github/dependabot.yml"),
        "version: 2\nupdates:\n  - package-ecosystem: npm\n    directories: [\"/\", \"/apps/*\"]\n    schedule: { interval: weekly }\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(
        report
            .results
            .iter()
            .filter(|result| result.check == "dependabot")
            .all(|result| result.status == CheckStatus::Pass)
    );
}

#[test]
fn dependabot_anchors_cargo_at_the_lockfile_owner() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/member\"]\n",
    )
    .unwrap();
    fs::write(repo.path().join("Cargo.lock"), "").unwrap();
    fs::create_dir_all(repo.path().join("crates/member/src")).unwrap();
    fs::write(
        repo.path().join("crates/member/Cargo.toml"),
        "[package]\nname = \"member\"\nversion = \"0.0.0\"\n",
    )
    .unwrap();
    fs::write(repo.path().join("crates/member/src/lib.rs"), "").unwrap();
    fs::create_dir_all(repo.path().join(".github")).unwrap();
    fs::write(
        repo.path().join(".github/dependabot.yml"),
        "version: 2\nupdates:\n  - package-ecosystem: cargo\n    directory: /\n    schedule: { interval: weekly }\n",
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let cargo = report
        .results
        .iter()
        .filter(|result| result.check == "dependabot")
        .collect::<Vec<_>>();
    assert_eq!(cargo.len(), 1);
    assert_eq!(cargo[0].status, CheckStatus::Pass);
    assert!(cargo[0].message.contains("cargo at `/`"));
}

#[test]
fn dependabot_requires_github_actions_at_the_root() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        "on: push\njobs: {}\n",
    )
    .unwrap();
    fs::write(
        repo.path().join(".github/dependabot.yml"),
        "version: 2\nupdates:\n  - package-ecosystem: github-actions\n    directory: /.github/workflows\n    schedule: { interval: weekly }\n",
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let actions = report
        .results
        .iter()
        .find(|result| result.check == "dependabot")
        .unwrap();
    assert_eq!(actions.status, CheckStatus::Fail);
    assert!(actions.message.contains("github-actions"));

    fs::write(
        repo.path().join(".github/dependabot.yml"),
        "version: 2\nupdates:\n  - package-ecosystem: github-actions\n    directory: /\n    schedule: { interval: weekly }\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(
        report
            .results
            .iter()
            .any(|result| { result.check == "dependabot" && result.status == CheckStatus::Pass })
    );
}

#[test]
fn dependabot_reports_disabled_and_unavailable_security_settings() {
    let mut repository = facts();
    repository.vulnerability_alerts = GithubValue::known(false);
    repository.automated_security_fixes = GithubValue::unavailable("HTTP 403");

    let report = run_github_checks(&repository);
    let dependabot = report
        .results
        .iter()
        .find(|result| result.check == "dependabot")
        .unwrap();
    assert_eq!(dependabot.status, CheckStatus::Fail);
    assert!(dependabot.message.contains("vulnerability alerts"));
    assert!(dependabot.message.contains("HTTP 403"));

    repository.vulnerability_alerts = GithubValue::known(true);
    let report = run_github_checks(&repository);
    let dependabot = report
        .results
        .iter()
        .find(|result| result.check == "dependabot")
        .unwrap();
    assert_eq!(dependabot.status, CheckStatus::Skip);
}
