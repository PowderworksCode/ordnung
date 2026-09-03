// Tests for `src/checks/scripts.rs`.
use crate::support::*;

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
