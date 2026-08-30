// Tests for `src/check.rs`: the registry of checks and the policy surface
// over it. A test belonging to one check lives beside that check, under
// tests/checks/.
use ordnung_core::{
    CheckStatus, InventoryOptions, RepoConfig, Severity, TestLayoutConfig, check_definition,
    check_definitions, check_ids, default_policy, inspect_repository,
    run_repository_checks_with_config, run_repository_checks_with_repo_config,
};
use std::collections::BTreeSet;
use std::fs;

#[test]
fn advisory_checks_keep_recommended_defaults() {
    let policy = default_policy();
    for check in [
        "artifacts-built",
        "auto-update-pr-branches",
        "ci-job-timeout",
        "ci-scheduled-run",
        "repo-meta",
    ] {
        assert_eq!(policy[check], Severity::Recommended, "{check}");
    }
}

/// A specific linter or an Ordnung-specific convention is a house preference, so it
/// ships off and a fleet raises it deliberately.
#[test]
fn house_preference_checks_ship_off() {
    let policy = default_policy();
    for check in [
        "action-badge",
        "field-guide",
        "hawk",
        "shellcheck",
        "stray-files",
        "stylelint",
        "test-inline",
        "test-mirror",
        "vale",
        "website",
        "zizmor",
    ] {
        assert_eq!(policy[check], Severity::Off, "{check}");
    }
}

#[test]
fn typescript_package_scripts_supply_ci_and_typecheck_tasks() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join("package.json"),
        r#"{
  "scripts": {
    "check": "biome check .",
    "test": "vitest run",
    "typecheck": "tsc --noEmit"
  },
  "devDependencies": { "typescript": "latest" }
}"#,
    )
    .unwrap();
    fs::write(repo.path().join("tsconfig.json"), "{}\n").unwrap();
    fs::write(repo.path().join("src/index.ts"), "export {};\n").unwrap();
    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        "on: pull_request\njobs:\n  quality:\n    steps:\n      - run: |\n          bun run check\n          bun run test\n          bun run typecheck\n",
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
    assert!(
        report
            .results
            .iter()
            .any(|result| { result.check == "typecheck" && result.status == CheckStatus::Pass })
    );
}

/// The three tool checks skip when their subject is absent, fail when the
/// subject exists with no workflow coverage, and pass when a change-triggered
/// workflow runs the tool by command, runner, cargo subcommand, or action.
#[test]
fn lint_tool_checks_skip_fail_and_pass_on_workflow_coverage() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(repo.path().join("README.md"), "# Fixture\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    for check in ["hawk", "shellcheck", "zizmor"] {
        assert_eq!(
            report
                .results
                .iter()
                .find(|result| result.check == check)
                .unwrap()
                .status,
            CheckStatus::Skip,
            "{check} without its subject"
        );
    }

    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::write(repo.path().join("src/lib.rs"), "pub fn fixture() {}\n").unwrap();
    fs::write(repo.path().join("scripts.sh"), "#!/bin/sh\necho fixture\n").unwrap();
    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        "on: pull_request\njobs:\n  build:\n    steps:\n      - run: cargo test\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    for check in ["hawk", "shellcheck", "zizmor"] {
        assert_eq!(
            report
                .results
                .iter()
                .find(|result| result.check == check)
                .unwrap()
                .status,
            CheckStatus::Fail,
            "{check} without workflow coverage"
        );
    }

    fs::write(
        repo.path().join(".github/workflows/lint.yml"),
        "on: pull_request\njobs:\n  lint:\n    steps:\n      - run: cargo +1.98.0 hawk check -D warnings\n      - run: uvx zizmor@1.14.2 .\n      - uses: ludeeus/action-shellcheck@2.0.0\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    for check in ["hawk", "shellcheck", "zizmor"] {
        assert_eq!(
            report
                .results
                .iter()
                .find(|result| result.check == check)
                .unwrap()
                .status,
            CheckStatus::Pass,
            "{check} with workflow coverage"
        );
    }
}

#[test]
fn registered_checks_are_complete_unique_and_sorted() {
    let definitions = check_definitions();
    assert_eq!(definitions.len(), 51);
    assert!(definitions.windows(2).all(|pair| pair[0].id < pair[1].id));
    assert!(
        definitions
            .iter()
            .all(|definition| !definition.instructions.trim().is_empty())
    );

    let ids = check_ids();
    assert_eq!(ids.len(), ids.iter().collect::<BTreeSet<_>>().len());
    for definition in definitions {
        assert!(std::ptr::eq(
            check_definition(definition.id).unwrap(),
            *definition
        ));
    }
}

#[test]
fn default_policy_comes_from_registered_definitions() {
    let policy = default_policy();
    assert_eq!(policy.len(), check_definitions().len());
    for definition in check_definitions() {
        assert_eq!(policy[definition.id], definition.default_severity);
    }
    assert_eq!(policy["test-inline"], Severity::Off);
    assert_eq!(policy["test-mirror"], Severity::Off);
}

/// Policy that selects directories can only apply to project-scoped checks, so the
/// scope must be declared rather than inferred.
#[test]
fn every_check_declares_a_scope_and_github_checks_are_repository_scoped() {
    use ordnung_core::CheckScope;
    for definition in check_definitions() {
        if definition.github_runner.is_some() {
            assert_eq!(
                definition.scope,
                CheckScope::Repository,
                "{} reads GitHub facts, which describe one repository",
                definition.id
            );
        }
    }
    let project = check_definitions()
        .iter()
        .filter(|definition| definition.scope == CheckScope::Project)
        .count();
    assert_eq!(project, 12, "project-scoped checks");
}
