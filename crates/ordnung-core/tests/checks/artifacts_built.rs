// Tests for `src/checks/artifacts_built.rs`.
use crate::support::*;

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
