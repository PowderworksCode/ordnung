// Fixtures shared by the mirrored check tests. A check's file imports this
// one and nothing else, so it reads as the check it belongs to.
//
// No single file needs every fixture, and one target compiles them all.
#![allow(dead_code)]

pub use std::fs;

pub use ordnung_core::fleet::ManagedState;
pub use ordnung_core::{
    CheckStatus, CiExistsConfig, DependencyRequirement, InventoryOptions, LanguageTestLayout,
    RepoConfig, Severity, TestLayoutConfig, default_policy, inspect_repository,
    run_repository_checks_with_config, run_repository_checks_with_repo_config,
    run_repository_checks_with_requirements,
};

pub fn complete_readme(link: &str) -> String {
    format!(
        "# Demo\n\nA repository that demonstrates the README quality floor.\n\n\
         ## Getting Started\n\nRun the development setup command.\n\n\
         ## Usage\n\nUse the command and see [the guide]({link}).\n\n\
         ### Contributions\n\nChanges are welcome through pull requests.\n\n\
         ## Licensing\n\nReleased under the MIT license.\n\n{}",
        "Additional documentation explains the project purpose, behavior, maintenance, and supported workflows clearly. ".repeat(20)
    )
}

pub fn requirement(name: &str, language: &str, require: &[&str]) -> DependencyRequirement {
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

pub fn dependency_result(
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

pub fn hooks_result(repo: &std::path::Path) -> ordnung_core::CheckResult {
    let inventory = inspect_repository(repo, &InventoryOptions::default()).unwrap();
    run_repository_checks_with_repo_config(repo, &inventory, &RepoConfig::default())
        .results
        .into_iter()
        .find(|result| result.check == "git-hooks")
        .expect("git-hooks runs")
}

#[cfg(unix)]
pub fn write_hook(repo: &std::path::Path, name: &str, executable: bool) {
    use std::os::unix::fs::PermissionsExt;
    let dir = repo.join(".githooks");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
    let mode = if executable { 0o755 } else { 0o644 };
    fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
}

pub fn write_dev_script(repo: &std::path::Path, body: &str) {
    fs::create_dir_all(repo.join("scripts")).unwrap();
    fs::write(repo.join("scripts/dev.sh"), body).unwrap();
}
