// Tests for `src/checks/builds.rs`.
use crate::support::*;

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
