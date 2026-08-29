use std::fs;

use ordnung_core::fleet::ManagedState;
use ordnung_core::{
    CheckStatus, CiExistsConfig, DependencyRequirement, InventoryOptions, LanguageTestLayout,
    RepoConfig, Severity, TestLayoutConfig, default_policy, inspect_repository,
    run_repository_checks_with_config, run_repository_checks_with_repo_config,
    run_repository_checks_with_requirements,
};

#[test]
fn rust_test_layout_rejects_inline_tests_and_requires_a_mirror() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("src/lib.rs"),
        "pub fn value() -> bool { true }\n#[cfg(test)]\nmod tests {}\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();

    let report =
        run_repository_checks_with_config(repo.path(), &inventory, &TestLayoutConfig::default());
    let failures: Vec<_> = report
        .results
        .iter()
        .filter(|result| {
            matches!(result.check.as_str(), "test-inline" | "test-mirror")
                && result.status == CheckStatus::Fail
        })
        .collect();
    assert_eq!(failures.len(), 2);
    assert!(
        failures
            .iter()
            .all(|result| result.severity == Severity::Off)
    );
    // Each position is now reported by its own check, so a fleet can require one
    // without the other.
    let inline = failures
        .iter()
        .find(|result| result.check == "test-inline")
        .expect("test-inline reports the inline module");
    assert!(inline.message.contains("inline"), "{}", inline.message);
    let mirror = failures
        .iter()
        .find(|result| result.check == "test-mirror")
        .expect("test-mirror reports the missing mirror");
    assert!(mirror.message.contains("mirrored"), "{}", mirror.message);

    fs::write(
        repo.path().join("src/lib.rs"),
        "pub fn value() -> bool { true }\npub const MARKER: &str = \"#[cfg(test)]\";\n",
    )
    .unwrap();
    fs::create_dir_all(repo.path().join("tests")).unwrap();
    fs::write(repo.path().join("tests/lib.rs"), "#[test]\nfn value() {}\n").unwrap();
    let clean =
        run_repository_checks_with_config(repo.path(), &inventory, &TestLayoutConfig::default());
    for check in ["test-inline", "test-mirror"] {
        assert!(
            clean
                .results
                .iter()
                .any(|result| result.check == check && result.status == CheckStatus::Pass),
            "{check} should pass"
        );
    }
}

#[test]
fn typescript_layout_accepts_configured_external_suffix() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::create_dir_all(repo.path().join("checks")).unwrap();
    fs::write(
        repo.path().join("package.json"),
        r#"{"devDependencies":{"typescript":"1.0.0"}}"#,
    )
    .unwrap();
    fs::write(repo.path().join("tsconfig.json"), "{}").unwrap();
    fs::write(
        repo.path().join("src/widget.ts"),
        "export const widget = 1;\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("checks/widget.spec.ts"),
        "test('widget', () => {});\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let mut config = TestLayoutConfig::default();
    config.languages.insert(
        "typescript".into(),
        LanguageTestLayout {
            source_roots: vec!["src".into()],
            test_root: "checks".into(),
            test_suffixes: vec![".spec".into()],
        },
    );

    let report = run_repository_checks_with_config(repo.path(), &inventory, &config);
    assert!(
        report
            .results
            .iter()
            .any(|result| { result.check == "test-mirror" && result.status == CheckStatus::Pass })
    );
}

#[test]
fn test_layout_checks_are_optional_by_default() {
    assert_eq!(default_policy()["test-inline"], Severity::Off);
    assert_eq!(default_policy()["test-mirror"], Severity::Off);
}

#[test]
fn lockfile_check_uses_discovered_ownership_once_per_workspace() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[workspace]\nmembers=['crates/member']\n",
    )
    .unwrap();
    fs::write(repo.path().join("Cargo.lock"), "").unwrap();
    fs::create_dir_all(repo.path().join("crates/member")).unwrap();
    fs::write(
        repo.path().join("crates/member/Cargo.toml"),
        "[package]\nname='member'\nversion='0.0.0'\n",
    )
    .unwrap();
    fs::create_dir_all(repo.path().join("tools/standalone")).unwrap();
    fs::write(
        repo.path().join("tools/standalone/Cargo.toml"),
        "[package]\nname='standalone'\nversion='0.0.0'\n",
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_config(repo.path(), &inventory, &TestLayoutConfig::default());
    let lockfiles = report
        .results
        .iter()
        .filter(|result| result.check == "lockfiles")
        .collect::<Vec<_>>();
    assert_eq!(lockfiles.len(), 2);
    assert!(lockfiles.iter().any(|result| {
        result.scope.as_os_str().is_empty() && result.status == CheckStatus::Pass
    }));
    assert!(lockfiles.iter().any(|result| {
        result.scope == std::path::Path::new("tools/standalone")
            && result.status == CheckStatus::Fail
    }));
}

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
fn field_guide_can_live_anywhere_in_the_repository() {
    let repo = tempfile::tempdir().unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let missing = report
        .results
        .iter()
        .find(|result| result.check == "field-guide")
        .unwrap();
    assert_eq!(missing.status, CheckStatus::Fail);
    assert_eq!(missing.severity, Severity::Off);
    assert_eq!(missing.scope, std::path::Path::new("notes/field_guide.md"));

    fs::create_dir_all(repo.path().join("knowledge")).unwrap();
    fs::write(
        repo.path().join("knowledge/field_guide.md"),
        "# Agent Field Guide\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let present = report
        .results
        .iter()
        .find(|result| result.check == "field-guide")
        .unwrap();
    assert_eq!(present.status, CheckStatus::Pass);
    assert_eq!(
        present.scope,
        std::path::Path::new("knowledge/field_guide.md")
    );
}

#[test]
fn license_requires_an_approved_root_filename() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("nested")).unwrap();
    fs::write(repo.path().join("nested/LICENSE"), "nested terms\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let missing = report
        .results
        .iter()
        .find(|result| result.check == "license")
        .unwrap();
    assert_eq!(missing.status, CheckStatus::Fail);
    assert_eq!(missing.scope, std::path::Path::new("LICENSE"));

    fs::write(repo.path().join("COPYING"), "custom terms\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let present = report
        .results
        .iter()
        .find(|result| result.check == "license")
        .unwrap();
    assert_eq!(present.status, CheckStatus::Pass);
    assert_eq!(present.scope, std::path::Path::new("COPYING"));

    fs::write(repo.path().join("LICENSE.md"), "preferred terms\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let preferred = report
        .results
        .iter()
        .find(|result| result.check == "license")
        .unwrap();
    assert_eq!(preferred.scope, std::path::Path::new("LICENSE.md"));
}

#[test]
fn changelog_requires_an_approved_root_filename() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("docs")).unwrap();
    fs::write(repo.path().join("docs/CHANGELOG.md"), "# Changes\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert_eq!(
        report
            .results
            .iter()
            .find(|result| result.check == "changelog")
            .unwrap()
            .status,
        CheckStatus::Fail
    );
    fs::write(repo.path().join("HISTORY.md"), "# History\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert_eq!(
        report
            .results
            .iter()
            .find(|result| result.check == "changelog")
            .unwrap()
            .scope,
        std::path::Path::new("HISTORY.md")
    );
}

#[test]
fn stray_files_use_configured_corrals() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(repo.path().join("ROADMAP.md"), "# Roadmap\n").unwrap();
    fs::write(repo.path().join("TODOS.md"), "# Todos\n").unwrap();
    fs::write(repo.path().join("todo.txt"), "TODO: centralized\n").unwrap();
    let config =
        RepoConfig::parse("ordnung.toml", "[stray_files]\nallow = ['ROADMAP.md']\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report = run_repository_checks_with_repo_config(repo.path(), &inventory, &config);
    let files = report
        .results
        .iter()
        .find(|result| result.check == "stray-files")
        .unwrap();
    assert_eq!(files.status, CheckStatus::Fail);
    assert!(files.message.contains("TODOS.md"));
    assert!(!files.message.contains("ROADMAP.md"));
}

#[test]
fn documentation_tools_require_typed_change_workflows_and_vale_styles() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::create_dir_all(repo.path().join("styles")).unwrap();
    fs::write(repo.path().join("styles/.gitkeep"), "").unwrap();
    fs::write(
        repo.path().join(".vale.ini"),
        "StylesPath = styles\n[*.md]\nBasedOnStyles = Vale\n",
    )
    .unwrap();
    fs::write(repo.path().join(".github/workflows/docs.yml"), "on: pull_request\njobs:\n  docs:\n    steps:\n      - uses: codespell-project/actions-codespell@v2\n      - uses: errata-ai/vale-action@reviewdog\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    for check in ["codespell", "vale"] {
        assert_eq!(
            report
                .results
                .iter()
                .find(|result| result.check == check)
                .unwrap()
                .status,
            CheckStatus::Pass,
            "{check}"
        );
    }
    fs::write(repo.path().join(".vale.ini"), "StylesPath = missing\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert_eq!(
        report
            .results
            .iter()
            .find(|result| result.check == "vale")
            .unwrap()
            .status,
        CheckStatus::Fail
    );
}

#[test]
fn reproducible_toolchain_rejects_only_unbounded_setup_versions() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        r#"on: push
jobs:
  test:
    steps:
      - uses: actions/setup-node@v4
        with: {node-version: latest}
      - uses: actions/setup-python@v5
        with: {python-version: '3.*'}
      - uses: example/toolchain-action@v1
        with: {version: latest}
"#,
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let toolchain = report
        .results
        .iter()
        .find(|result| result.check == "reproducible-toolchain")
        .unwrap();
    assert_eq!(toolchain.status, CheckStatus::Fail);
    assert!(toolchain.message.contains("node-version: latest"));
    assert!(toolchain.message.contains("python-version: 3.*"));

    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        r#"on: push
jobs:
  test:
    steps:
      - uses: actions/setup-node@v4
        with: {node-version: 20.11.0}
      - uses: actions/setup-go@v5
        with: {go-version: stable}
      - uses: dtolnay/rust-toolchain@stable
"#,
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(report.results.iter().any(|result| {
        result.check == "reproducible-toolchain" && result.status == CheckStatus::Pass
    }));
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

#[test]
fn javascript_requires_an_explicit_type_layer_and_ci_typecheck() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::write(repo.path().join("package.json"), "{}\n").unwrap();
    fs::write(
        repo.path().join("src/index.js"),
        "export const value = 1;\n",
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let typecheck = report
        .results
        .iter()
        .find(|result| result.check == "typecheck")
        .unwrap();
    assert_eq!(typecheck.status, CheckStatus::Fail);
    assert!(typecheck.message.contains("no type layer"));
    assert!(typecheck.message.contains("jsconfig.json or tsconfig.json"));

    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join("jsconfig.json"),
        "{\"compilerOptions\":{\"checkJs\":true}}\n",
    )
    .unwrap();
    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        "on: push\njobs:\n  types:\n    steps:\n      - run: npx tsc --noEmit\n",
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(report.results.iter().any(|result| {
        result.check == "typecheck"
            && result.status == CheckStatus::Pass
            && result.message.contains("jsconfig.json")
    }));
}

#[test]
fn compiled_only_projects_skip_the_separate_typecheck_check() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname='fixture'\nversion='0.0.0'\n",
    )
    .unwrap();
    fs::write(repo.path().join("src/lib.rs"), "pub fn fixture() {}\n").unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(
        report
            .results
            .iter()
            .any(|result| { result.check == "typecheck" && result.status == CheckStatus::Skip })
    );
}

#[test]
fn every_package_build_target_must_run_on_changes() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join("package.json"),
        r#"{
  "scripts": {
    "build:web": "vite build",
    "release:build": "bun run build:web"
  },
  "devDependencies": {"typescript": "latest"}
}"#,
    )
    .unwrap();
    fs::write(repo.path().join("src/index.ts"), "export {};\n").unwrap();
    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        "on: pull_request\njobs:\n  build:\n    steps:\n      - run: bun run release:build\n",
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let builds = report
        .results
        .iter()
        .filter(|result| result.check == "builds")
        .collect::<Vec<_>>();
    assert_eq!(builds.len(), 2);
    assert!(
        builds
            .iter()
            .all(|result| result.status == CheckStatus::Pass)
    );

    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        "on: pull_request\njobs:\n  build:\n    steps:\n      - run: vite build\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(report.results.iter().any(|result| {
        result.check == "builds"
            && result.status == CheckStatus::Fail
            && result.message.contains("build:web")
    }));
}

#[test]
fn tauri_needs_change_compile_and_scheduled_full_build() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::create_dir_all(repo.path().join("src-tauri/src")).unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join("package.json"),
        r#"{"scripts":{"tauri":"tauri"},"devDependencies":{"typescript":"latest"}}"#,
    )
    .unwrap();
    fs::write(repo.path().join("src/index.ts"), "export {};\n").unwrap();
    fs::write(
        repo.path().join("src-tauri/Cargo.toml"),
        "[package]\nname='desktop'\nversion='0.0.0'\n",
    )
    .unwrap();
    fs::write(repo.path().join("src-tauri/src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(repo.path().join("src-tauri/tauri.conf.json"), "{}\n").unwrap();
    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        "on: pull_request\njobs:\n  desktop:\n    steps:\n      - run: cargo check --manifest-path src-tauri/Cargo.toml\n",
    )
    .unwrap();
    fs::write(
        repo.path().join(".github/workflows/nightly.yml"),
        "on:\n  schedule:\n    - cron: '0 3 * * *'\njobs:\n  desktop:\n    steps:\n      - run: bun run tauri build\n",
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(report.results.iter().any(|result| {
        result.check == "builds"
            && result.status == CheckStatus::Pass
            && result.message.contains("scheduled full build")
    }));
}

#[test]
fn artifact_builds_follow_transitive_package_scripts() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join("package.json"),
        r#"{
  "scripts": {
    "build:web": "vite build",
    "build:addon": "napi build --release",
    "build:all": "bun run build:web && bun run build:addon"
  },
  "devDependencies": {
    "@napi-rs/cli": "1",
    "typescript": "1",
    "vite": "1"
  }
}"#,
    )
    .unwrap();
    fs::write(repo.path().join("src/index.ts"), "export {};\n").unwrap();
    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        "on: push\njobs:\n  build:\n    steps:\n      - run: bun run build:all\n",
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let artifacts = report
        .results
        .iter()
        .filter(|result| result.check == "artifacts-built")
        .collect::<Vec<_>>();
    assert_eq!(artifacts.len(), 2);
    assert!(
        artifacts
            .iter()
            .all(|result| result.status == CheckStatus::Pass)
    );
}

#[test]
fn artifact_builds_do_not_cross_package_boundaries() {
    let repo = tempfile::tempdir().unwrap();
    for package in ["apps/a", "apps/b"] {
        fs::create_dir_all(repo.path().join(package).join("src")).unwrap();
        fs::write(
            repo.path().join(package).join("package.json"),
            r#"{"devDependencies":{"typescript":"1","vite":"1"}}"#,
        )
        .unwrap();
        fs::write(
            repo.path().join(package).join("src/index.ts"),
            "export {};\n",
        )
        .unwrap();
    }
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        r#"on: push
jobs:
  build:
    defaults:
      run:
        working-directory: apps/a
    steps:
      - run: vite build
"#,
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(report.results.iter().any(|result| {
        result.check == "artifacts-built"
            && result.scope == std::path::Path::new("apps/a")
            && result.status == CheckStatus::Pass
    }));
    assert!(report.results.iter().any(|result| {
        result.check == "artifacts-built"
            && result.scope == std::path::Path::new("apps/b")
            && result.status == CheckStatus::Fail
    }));
}

#[test]
fn cargo_workspace_builds_cover_member_binaries() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[workspace]\nmembers=['crates/a', 'crates/b']\n",
    )
    .unwrap();
    for package in ["a", "b"] {
        let root = repo.path().join("crates").join(package);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            format!("[package]\nname='{package}'\nversion='0.0.0'\n"),
        )
        .unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
    }
    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        "on: push\njobs:\n  test:\n    steps:\n      - run: cargo test --workspace\n",
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let artifacts = report
        .results
        .iter()
        .filter(|result| result.check == "artifacts-built")
        .collect::<Vec<_>>();
    assert_eq!(artifacts.len(), 2);
    assert!(
        artifacts
            .iter()
            .all(|result| result.status == CheckStatus::Pass)
    );
}

#[test]
fn codegen_requires_a_subsequent_guard_in_the_same_job() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("generated")).unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(repo.path().join("generated/bindings.rs"), "// generated\n").unwrap();
    let config = RepoConfig::parse(
        "ordnung.toml",
        r#"[[codegen]]
name = "bindings"
command = "make bindgen"
outputs = ["generated/**"]
"#,
    )
    .unwrap();

    fs::write(
        repo.path().join(".github/workflows/codegen.yml"),
        r#"on: pull_request
jobs:
  generated:
    steps:
      - run: git diff --exit-code
      - run: make bindgen
  unrelated:
    steps:
      - run: git status --porcelain
"#,
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report = run_repository_checks_with_repo_config(repo.path(), &inventory, &config);
    assert!(report.results.iter().any(|result| {
        result.check == "codegen-drift"
            && result.status == CheckStatus::Fail
            && result.message.contains("subsequent")
    }));

    fs::write(
        repo.path().join(".github/workflows/codegen.yml"),
        r#"on: pull_request
jobs:
  generated:
    steps:
      - run: |
          make bindgen
          git diff --exit-code
"#,
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report = run_repository_checks_with_repo_config(repo.path(), &inventory, &config);
    assert!(
        report.results.iter().any(|result| {
            result.check == "codegen-drift" && result.status == CheckStatus::Pass
        })
    );
}

#[test]
fn codegen_commands_are_scoped_to_the_declared_project() {
    let repo = tempfile::tempdir().unwrap();
    for package in ["packages/a", "packages/b"] {
        fs::create_dir_all(repo.path().join(package)).unwrap();
        fs::write(repo.path().join(package).join("package.json"), "{}\n").unwrap();
    }
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join(".github/workflows/codegen.yml"),
        r#"on: pull_request
jobs:
  generated:
    defaults:
      run:
        working-directory: packages/a
    steps:
      - run: bun run generate
      - run: git diff --exit-code
"#,
    )
    .unwrap();
    let config = RepoConfig::parse(
        "ordnung.toml",
        r#"[[codegen]]
name = "package b"
root = "packages/b"
command = "bun run generate"
outputs = ["src/generated/**"]
"#,
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report = run_repository_checks_with_repo_config(repo.path(), &inventory, &config);
    assert!(report.results.iter().any(|result| {
        result.check == "codegen-drift"
            && result.status == CheckStatus::Fail
            && result.message.contains("never runs")
    }));
}

#[test]
fn codegen_commands_resolve_through_package_scripts() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join("package.json"),
        r#"{"scripts":{"generate":"napi build --platform"}}"#,
    )
    .unwrap();
    fs::write(
        repo.path().join(".github/workflows/codegen.yml"),
        r#"on: pull_request
jobs:
  generated:
    steps:
      - run: bun run generate
      - run: git diff --quiet
"#,
    )
    .unwrap();
    let config = RepoConfig::parse(
        "ordnung.toml",
        r#"[[codegen]]
name = "native bindings"
command = "napi build --platform"
outputs = ["index.node"]
"#,
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report = run_repository_checks_with_repo_config(repo.path(), &inventory, &config);
    assert!(
        report.results.iter().any(|result| {
            result.check == "codegen-drift" && result.status == CheckStatus::Pass
        })
    );
}

#[test]
fn tauri_artifact_requires_a_scheduled_full_build() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("src-tauri")).unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join("package.json"),
        r#"{"scripts":{"tauri":"tauri"},"devDependencies":{"vite":"1"}}"#,
    )
    .unwrap();
    fs::write(repo.path().join("src-tauri/tauri.conf.json"), "{}\n").unwrap();
    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        "on: push\njobs:\n  desktop:\n    steps:\n      - run: bun run tauri build\n",
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(report.results.iter().any(|result| {
        result.check == "artifacts-built"
            && result.message.contains("Tauri")
            && result.status == CheckStatus::Fail
    }));
    assert!(report.results.iter().any(|result| {
        result.check == "artifacts-built"
            && result.message.contains("site")
            && result.status == CheckStatus::Pass
    }));

    fs::write(
        repo.path().join(".github/workflows/ci.yml"),
        "on:\n  schedule:\n    - cron: '0 3 * * *'\njobs:\n  desktop:\n    steps:\n      - run: bun run tauri build\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    assert!(
        report
            .results
            .iter()
            .filter(|result| { result.check == "artifacts-built" })
            .all(|result| result.status == CheckStatus::Pass)
    );
}

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

#[test]
fn codeowners_requires_a_rule_that_assigns_an_owner() {
    let repo = tempfile::tempdir().unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let missing = report
        .results
        .iter()
        .find(|result| result.check == "codeowners")
        .unwrap();
    assert_eq!(missing.status, CheckStatus::Fail);
    assert!(missing.message.contains("no CODEOWNERS file"));

    fs::write(repo.path().join("CODEOWNERS"), "# defaults\n/apps/github\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let unowned = report
        .results
        .iter()
        .find(|result| result.check == "codeowners")
        .unwrap();
    assert_eq!(unowned.status, CheckStatus::Fail);
    assert!(unowned.message.contains("no rules that assign an owner"));

    fs::write(repo.path().join("CODEOWNERS"), "* @org/maintainers\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let owned = report
        .results
        .iter()
        .find(|result| result.check == "codeowners")
        .unwrap();
    assert_eq!(owned.status, CheckStatus::Pass);
    assert_eq!(owned.scope, std::path::Path::new("CODEOWNERS"));
}

#[test]
fn codeowners_rejects_invalid_github_syntax() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(repo.path().join("CODEOWNERS"), "!generated/ @owner\n").unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let invalid = report
        .results
        .iter()
        .find(|result| result.check == "codeowners")
        .unwrap();
    assert_eq!(invalid.status, CheckStatus::Fail);
    assert!(invalid.message.contains("negated patterns"));
}

#[test]
fn scripts_requires_a_documented_development_entry_and_corrals_shell_files() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("scripts")).unwrap();
    fs::write(repo.path().join("scripts/dev.sh"), "#!/bin/sh\n").unwrap();
    fs::write(
        repo.path().join("README.md"),
        "# Demo\n\nRun `scripts/dev.sh` to set up.\n",
    )
    .unwrap();
    fs::write(repo.path().join("deploy.sh"), "#!/bin/sh\n").unwrap();
    fs::write(repo.path().join("release"), "#!/usr/bin/env bash\n").unwrap();
    fs::create_dir_all(repo.path().join(".githooks")).unwrap();
    fs::write(repo.path().join(".githooks/pre-commit.sh"), "#!/bin/sh\n").unwrap();
    fs::create_dir_all(repo.path().join("vendor/package")).unwrap();
    fs::write(repo.path().join("vendor/package/build.sh"), "#!/bin/sh\n").unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    assert!(
        inventory
            .shell_scripts
            .contains(std::path::Path::new("release"))
    );
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let scripts = report
        .results
        .iter()
        .find(|result| result.check == "scripts")
        .unwrap();
    assert_eq!(scripts.status, CheckStatus::Fail);
    assert!(scripts.message.contains("deploy.sh"));
    assert!(scripts.message.contains("release"));
    assert!(!scripts.message.contains("pre-commit"));
    assert!(!scripts.message.contains("vendor"));

    fs::remove_file(repo.path().join("deploy.sh")).unwrap();
    fs::remove_file(repo.path().join("release")).unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let scripts = report
        .results
        .iter()
        .find(|result| result.check == "scripts")
        .unwrap();
    assert_eq!(scripts.status, CheckStatus::Pass);
}

#[test]
fn scripts_reports_missing_and_undocumented_development_entries() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(repo.path().join("README.md"), "# Demo\n").unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let scripts = report
        .results
        .iter()
        .find(|result| result.check == "scripts")
        .unwrap();
    assert_eq!(scripts.status, CheckStatus::Fail);
    assert!(scripts.message.contains("no scripts/dev.sh"));

    fs::create_dir_all(repo.path().join("scripts")).unwrap();
    fs::write(repo.path().join("scripts/dev.sh"), "#!/bin/sh\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let scripts = report
        .results
        .iter()
        .find(|result| result.check == "scripts")
        .unwrap();
    assert_eq!(scripts.status, CheckStatus::Fail);
    assert!(scripts.message.contains("not mentioned in README.md"));
}

#[test]
fn scripts_configuration_uses_exact_allow_paths_and_custom_entry_points() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("bin")).unwrap();
    fs::create_dir_all(repo.path().join("tools")).unwrap();
    fs::write(repo.path().join("bin/setup"), "#!/usr/bin/env bash\n").unwrap();
    fs::write(repo.path().join("install.sh"), "#!/bin/sh\n").unwrap();
    fs::write(repo.path().join("tools/install.sh"), "#!/bin/sh\n").unwrap();
    fs::write(repo.path().join("README"), "Run `bin/setup`.\n").unwrap();
    let config = RepoConfig::parse(
        "ordnung.toml",
        "[scripts]\ndirectory = 'bin'\ndevelopment = 'setup'\nallow = ['install.sh']\n",
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report = run_repository_checks_with_repo_config(repo.path(), &inventory, &config);
    let scripts = report
        .results
        .iter()
        .find(|result| result.check == "scripts")
        .unwrap();
    assert_eq!(scripts.status, CheckStatus::Fail);
    assert!(!scripts.message.contains("outside bin/: install.sh,"));
    assert!(scripts.message.contains("tools/install.sh"));
}

#[test]
fn conventional_commits_requires_real_ci_enforcement_and_documentation() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join(".github/workflows/conventional.yml"),
        "name: conventional\non: pull_request\njobs: {}\n",
    )
    .unwrap();
    fs::write(repo.path().join("README.md"), "# Demo\n").unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let conventional = report
        .results
        .iter()
        .find(|result| result.check == "conventional-commits")
        .unwrap();
    assert_eq!(conventional.status, CheckStatus::Fail);
    assert!(conventional.message.contains("no CI enforcement"));
    assert!(conventional.message.contains("not mentioned"));

    fs::write(
        repo.path().join(".github/workflows/conventional.yml"),
        "on: pull_request_target\njobs:\n  title:\n    steps:\n      - uses: amannn/action-semantic-pull-request@v6\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("README.md"),
        "# Demo\n\nPull request titles follow Conventional Commits.\n",
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let conventional = report
        .results
        .iter()
        .find(|result| result.check == "conventional-commits")
        .unwrap();
    assert_eq!(conventional.status, CheckStatus::Pass);
    assert_eq!(
        conventional.scope,
        std::path::Path::new(".github/workflows/conventional.yml")
    );
}

fn complete_readme(link: &str) -> String {
    format!(
        "# Demo\n\nA repository that demonstrates the README quality floor.\n\n\
         ## Getting Started\n\nRun the development setup command.\n\n\
         ## Usage\n\nUse the command and see [the guide]({link}).\n\n\
         ### Contributions\n\nChanges are welcome through pull requests.\n\n\
         ## Licensing\n\nReleased under the MIT license.\n\n{}",
        "Additional documentation explains the project purpose, behavior, maintenance, and supported workflows clearly. ".repeat(20)
    )
}

#[test]
fn readme_requires_existence_and_a_title_while_quality_judges_the_shape() {
    let repo = tempfile::tempdir().unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let missing = report
        .results
        .iter()
        .find(|result| result.check == "readme")
        .unwrap();
    assert_eq!(missing.status, CheckStatus::Fail);
    assert!(missing.message.contains("no root README"));

    fs::write(repo.path().join("README.md"), "# Demo\n\nIt works.\n").unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let floor = report
        .results
        .iter()
        .find(|result| result.check == "readme")
        .unwrap();
    assert_eq!(floor.status, CheckStatus::Pass, "{}", floor.message);
    let thin = report
        .results
        .iter()
        .find(|result| result.check == "readme-quality")
        .unwrap();
    assert_eq!(thin.status, CheckStatus::Fail);
    for problem in [
        "under 150 words",
        "install/getting-started",
        "usage/docs",
        "License section",
        "Contributing section",
    ] {
        assert!(thin.message.contains(problem), "missing {problem:?}");
    }

    fs::create_dir_all(repo.path().join("docs")).unwrap();
    fs::write(repo.path().join("docs/guide.md"), "# Guide\n").unwrap();
    fs::write(
        repo.path().join("README.md"),
        complete_readme("docs/guide.md?view=full#usage"),
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let complete = report
        .results
        .iter()
        .find(|result| result.check == "readme")
        .unwrap();
    assert_eq!(complete.status, CheckStatus::Pass, "{}", complete.message);
}

#[test]
fn readme_rejects_broken_and_escaping_relative_links() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(
        repo.path().join("README.md"),
        complete_readme("docs/missing.md"),
    )
    .unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let broken = report
        .results
        .iter()
        .find(|result| result.check == "readme-quality")
        .unwrap();
    assert_eq!(broken.status, CheckStatus::Fail);
    assert!(broken.message.contains("docs/missing.md"));

    fs::write(
        repo.path().join("README.md"),
        complete_readme("../outside.md"),
    )
    .unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let escaping = report
        .results
        .iter()
        .find(|result| result.check == "readme-quality")
        .unwrap();
    assert_eq!(escaping.status, CheckStatus::Fail);
    assert!(escaping.message.contains("../outside.md"));
}

#[test]
fn readme_accepts_its_word_limit_and_rejects_one_word_over() {
    let repo = tempfile::tempdir().unwrap();
    let mut readme = complete_readme("#usage");
    let existing_words = readme.split_whitespace().count();
    readme.push_str(&" filler".repeat(1_500 - existing_words));
    assert_eq!(readme.split_whitespace().count(), 1_500);
    fs::write(repo.path().join("README.md"), &readme).unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let at_limit = report
        .results
        .iter()
        .find(|result| result.check == "readme-quality")
        .unwrap();
    assert_eq!(at_limit.status, CheckStatus::Pass, "{}", at_limit.message);

    readme.push_str(" overflow");
    fs::write(repo.path().join("README.md"), readme).unwrap();
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let over_limit = report
        .results
        .iter()
        .find(|result| result.check == "readme-quality")
        .unwrap();
    assert_eq!(over_limit.status, CheckStatus::Fail);
    assert!(over_limit.message.contains("over 1500 words (1501)"));
}

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

fn requirement(name: &str, language: &str, require: &[&str]) -> DependencyRequirement {
    DependencyRequirement {
        name: name.into(),
        language: Some(language.into()),
        ecosystem: None,
        require: require
            .iter()
            .map(|package| (*package).to_owned())
            .collect(),
        kind: None,
        state: ManagedState::Present,
    }
}

fn dependency_result(
    repo: &std::path::Path,
    manifest: &str,
    requirements: &[DependencyRequirement],
) -> ordnung_core::CheckResult {
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("Cargo.toml"), manifest).unwrap();
    fs::write(repo.join("src/lib.rs"), "pub fn value() {}\n").unwrap();
    let inventory = inspect_repository(repo, &InventoryOptions::default()).unwrap();
    let report = run_repository_checks_with_requirements(
        repo,
        &inventory,
        &RepoConfig::default(),
        requirements,
    );
    report
        .results
        .into_iter()
        .find(|result| result.check == "required-dependencies")
        .expect("required-dependencies runs")
}

#[test]
fn required_dependencies_skips_when_no_requirement_is_configured() {
    let repo = tempfile::tempdir().unwrap();
    let result = dependency_result(
        repo.path(),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\n",
        &[],
    );
    assert_eq!(result.status, CheckStatus::Skip);
}

#[test]
fn required_dependencies_reports_a_missing_package() {
    let repo = tempfile::tempdir().unwrap();
    let result = dependency_result(
        repo.path(),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\n",
        &[requirement("rust-refactoring", "rust", &["itertools"])],
    );
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(result.message.contains("itertools"), "{}", result.message);
}

#[test]
fn required_dependencies_accepts_a_declared_package() {
    let repo = tempfile::tempdir().unwrap();
    let result = dependency_result(
        repo.path(),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\n\
         [dependencies]\nitertools = \"0.14\"\n",
        &[requirement("rust-refactoring", "rust", &["itertools"])],
    );
    assert_eq!(result.status, CheckStatus::Pass, "{}", result.message);
}

#[test]
fn required_dependencies_ignore_packages_of_another_language() {
    let repo = tempfile::tempdir().unwrap();
    let result = dependency_result(
        repo.path(),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\n",
        &[requirement("ts-refactoring", "typescript", &["remeda"])],
    );
    assert_eq!(result.status, CheckStatus::Pass, "{}", result.message);
}

#[test]
fn a_workspace_member_resolves_an_inherited_declaration_and_the_root_is_not_required() {
    let repo = tempfile::tempdir().unwrap();
    let root = repo.path();
    fs::create_dir_all(root.join("crates/member/src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/member\"]\nresolver = \"3\"\n\
         [workspace.dependencies]\nitertools = \"0.14\"\n",
    )
    .unwrap();
    // Cargo requires the member to opt in; the root declaration alone grants nothing.
    fs::write(
        root.join("crates/member/Cargo.toml"),
        "[package]\nname = \"member\"\nversion = \"0.0.0\"\n\
         [dependencies]\nitertools.workspace = true\n",
    )
    .unwrap();
    fs::write(root.join("crates/member/src/lib.rs"), "pub fn value() {}\n").unwrap();
    let inventory = inspect_repository(root, &InventoryOptions::default()).unwrap();

    let report = run_repository_checks_with_requirements(
        root,
        &inventory,
        &RepoConfig::default(),
        &[requirement("rust-refactoring", "rust", &["itertools"])],
    );
    let result = report
        .results
        .iter()
        .find(|result| result.check == "required-dependencies")
        .expect("required-dependencies runs");
    assert_eq!(result.status, CheckStatus::Pass, "{}", result.message);
}

/// A Bun or npm package reports `javascript` from its manifest while the project
/// around it is TypeScript. Selecting `typescript` must still match it.
#[test]
fn a_typescript_project_matches_though_its_package_language_is_javascript() {
    let repo = tempfile::tempdir().unwrap();
    let root = repo.path();
    fs::create_dir_all(root.join("site/src")).unwrap();
    fs::write(root.join("site/package.json"), "{\"name\":\"site\"}").unwrap();
    fs::write(root.join("site/tsconfig.json"), "{}").unwrap();
    fs::write(root.join("site/src/index.ts"), "export const value = 1;\n").unwrap();
    let inventory = inspect_repository(root, &InventoryOptions::default()).unwrap();

    let package = inventory
        .packages
        .iter()
        .find(|package| package.root == std::path::Path::new("site"))
        .expect("the site package is discovered");
    assert_eq!(
        package.language.as_str(),
        "javascript",
        "manifest language is what makes this case worth testing"
    );

    let requirement = DependencyRequirement {
        name: "ts-refactoring".into(),
        language: Some("typescript".into()),
        ecosystem: None,
        require: vec!["lodash".into()],
        kind: None,
        state: ManagedState::Present,
    };
    let report = run_repository_checks_with_requirements(
        root,
        &inventory,
        &RepoConfig::default(),
        &[requirement],
    );
    let result = report
        .results
        .iter()
        .find(|result| result.check == "required-dependencies")
        .expect("required-dependencies runs");
    assert_eq!(result.status, CheckStatus::Fail, "{}", result.message);
    assert!(result.message.contains("lodash"), "{}", result.message);
}

fn hooks_result(repo: &std::path::Path) -> ordnung_core::CheckResult {
    let inventory = inspect_repository(repo, &InventoryOptions::default()).unwrap();
    run_repository_checks_with_repo_config(repo, &inventory, &RepoConfig::default())
        .results
        .into_iter()
        .find(|result| result.check == "git-hooks")
        .expect("git-hooks runs")
}

#[cfg(unix)]
fn write_hook(repo: &std::path::Path, name: &str, executable: bool) {
    use std::os::unix::fs::PermissionsExt;
    let dir = repo.join(".githooks");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
    let mode = if executable { 0o755 } else { 0o644 };
    fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
}

fn write_dev_script(repo: &std::path::Path, body: &str) {
    fs::create_dir_all(repo.join("scripts")).unwrap();
    fs::write(repo.join("scripts/dev.sh"), body).unwrap();
}

#[test]
fn git_hooks_requires_committed_hooks_or_a_manager() {
    let repo = tempfile::tempdir().unwrap();
    let result = hooks_result(repo.path());
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(
        result.message.contains("no committed hooks"),
        "{}",
        result.message
    );
}

#[cfg(unix)]
#[test]
fn git_hooks_accepts_executable_hooks_installed_by_the_development_script() {
    let repo = tempfile::tempdir().unwrap();
    write_hook(repo.path(), "pre-commit", true);
    write_hook(repo.path(), "commit-msg", true);
    write_dev_script(
        repo.path(),
        "#!/usr/bin/env bash\ngit config core.hooksPath .githooks\n",
    );
    let result = hooks_result(repo.path());
    assert_eq!(result.status, CheckStatus::Pass, "{}", result.message);
    assert!(result.message.contains('2'), "{}", result.message);
}

/// Git ignores a hook without the execute bit, so the gate looks present and never
/// runs. That is the worst way for this to fail, which is why it is graded.
#[cfg(unix)]
#[test]
fn git_hooks_rejects_a_hook_that_git_would_silently_ignore() {
    let repo = tempfile::tempdir().unwrap();
    write_hook(repo.path(), "pre-commit", false);
    write_dev_script(repo.path(), "git config core.hooksPath .githooks\n");
    let result = hooks_result(repo.path());
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(
        result.message.contains("not executable"),
        "{}",
        result.message
    );
    assert!(result.message.contains("pre-commit"), "{}", result.message);
}

#[cfg(unix)]
#[test]
fn git_hooks_rejects_committed_hooks_that_nothing_installs() {
    let repo = tempfile::tempdir().unwrap();
    write_hook(repo.path(), "pre-commit", true);
    write_dev_script(repo.path(), "#!/usr/bin/env bash\ncargo build\n");
    let result = hooks_result(repo.path());
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(
        result.message.contains("core.hooksPath"),
        "{}",
        result.message
    );
}

/// A manager installs through its own lifecycle, so requiring the development
/// script to repeat that would be wrong.
#[test]
fn git_hooks_accepts_a_declared_manager_without_a_development_script() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(
        repo.path().join("package.json"),
        r#"{"name":"fixture","devDependencies":{"lefthook":"1.7.0"}}"#,
    )
    .unwrap();
    let result = hooks_result(repo.path());
    assert_eq!(result.status, CheckStatus::Pass, "{}", result.message);
    assert!(result.message.contains("lefthook"), "{}", result.message);
}

/// A README beside the hooks is documentation, not something Git runs.
#[cfg(unix)]
#[test]
fn git_hooks_ignores_files_that_are_not_hook_names() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".githooks")).unwrap();
    fs::write(repo.path().join(".githooks/README.md"), "# hooks\n").unwrap();
    fs::write(
        repo.path().join(".githooks/run-straitjacket"),
        "#!/bin/sh\n",
    )
    .unwrap();
    let result = hooks_result(repo.path());
    assert_eq!(result.status, CheckStatus::Fail);
    assert!(
        result.message.contains("no committed hooks"),
        "{}",
        result.message
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

/// Modeled on a real repository whose fanout job enumerates the repository
/// rather than the change: the matrix expands identically on every pull
/// request, so depending on the fanout is not scoping. A fanout that reads
/// the diff, a job condition, or workflow path filters each short-circuit.
#[test]
fn matrix_jobs_must_short_circuit_on_pull_requests() {
    let status = |workflow: &str| {
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
        fs::write(repo.path().join(".github/workflows/ci.yml"), workflow).unwrap();
        let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
        let report =
            run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
        report
            .results
            .iter()
            .find(|result| result.check == "ci-matrix-scoped")
            .unwrap()
            .clone()
    };

    let enumerating = status(
        "on: pull_request\njobs:\n  discover:\n    outputs:\n      langs: ${{ steps.list.outputs.langs }}\n    steps:\n      - id: list\n        run: find crates -name grammar.js\n  sweep:\n    needs: discover\n    strategy:\n      matrix:\n        lang: ${{ fromJSON(needs.discover.outputs.langs) }}\n    steps:\n      - run: ./sweep.sh ${{ matrix.lang }}\n",
    );
    assert_eq!(enumerating.status, CheckStatus::Fail);
    assert!(enumerating.message.contains("ci.yml:sweep"));

    let diff_aware = status(
        "on: pull_request\njobs:\n  discover:\n    outputs:\n      langs: ${{ steps.list.outputs.langs }}\n    steps:\n      - id: list\n        run: git diff --name-only origin/main | cut -d/ -f2 | sort -u\n  sweep:\n    needs: discover\n    strategy:\n      matrix:\n        lang: ${{ fromJSON(needs.discover.outputs.langs) }}\n    steps:\n      - run: ./sweep.sh ${{ matrix.lang }}\n",
    );
    assert_eq!(diff_aware.status, CheckStatus::Pass);

    let conditioned = status(
        "on: pull_request\njobs:\n  changes:\n    outputs:\n      rust: ${{ steps.filter.outputs.rust }}\n    steps:\n      - id: filter\n        uses: dorny/paths-filter@v3\n  build:\n    needs: changes\n    if: needs.changes.outputs.rust == 'true'\n    strategy:\n      matrix:\n        os: [ubuntu-latest, macos-latest]\n    steps:\n      - run: cargo test\n",
    );
    assert_eq!(conditioned.status, CheckStatus::Pass);

    let path_filtered = status(
        "on:\n  pull_request:\n    paths: [\"crates/**\"]\njobs:\n  build:\n    strategy:\n      matrix:\n        os: [ubuntu-latest, macos-latest]\n    steps:\n      - run: cargo test\n",
    );
    assert_eq!(path_filtered.status, CheckStatus::Pass);
}
