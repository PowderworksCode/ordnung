// Fixtures for the lint-tool checks. Each of them answers the same three
// questions — is the subject present, does a workflow cover it, and does the
// invocation count — so the fixtures say that once.
#![allow(dead_code)]

use crate::support::*;

pub const CARGO: (&str, &str) = (
    "Cargo.toml",
    "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
);
pub const LIB: (&str, &str) = ("src/lib.rs", "pub fn fixture() {}\n");
pub const CI: (&str, &str) = (
    ".github/workflows/ci.yml",
    "on: pull_request\njobs:\n  build:\n    steps:\n      - run: cargo test\n",
);

pub fn repo_with(files: &[(&str, &str)]) -> tempfile::TempDir {
    let repo = tempfile::tempdir().unwrap();
    for (path, body) in files {
        let full = repo.path().join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, body).unwrap();
    }
    repo
}

pub fn status(repo: &std::path::Path, check: &str) -> CheckStatus {
    let inventory = inspect_repository(repo, &InventoryOptions::default()).unwrap();
    let report = run_repository_checks_with_repo_config(repo, &inventory, &RepoConfig::default());
    report
        .results
        .iter()
        .find(|result| result.check == check)
        .unwrap_or_else(|| panic!("{check} reports a result"))
        .status
}

/// A lint workflow running exactly the given step.
pub fn lint_workflow(step: &str) -> String {
    format!("on: pull_request\njobs:\n  lint:\n    steps:\n      - {step}\n")
}
