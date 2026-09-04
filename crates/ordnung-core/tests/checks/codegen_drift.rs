// Tests for `src/checks/codegen_drift.rs`.
use crate::support::*;

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
