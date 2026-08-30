// Tests for `src/checks/project_inventory.rs`.
//
// This check never fails. It reports what Ordnung managed to see, so that a
// repository graded against nothing says so out loud rather than looking clean.
use crate::support::*;
use crate::support_lint::*;

#[test]
fn a_repository_with_no_recognised_project_still_passes_and_says_so() {
    let repo = repo_with(&[("README.md", "# Fixture\n")]);
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let result = report
        .results
        .iter()
        .find(|result| result.check == "project-inventory")
        .expect("project-inventory reports a result");
    assert_eq!(result.status, CheckStatus::Pass);
    assert!(
        result.message.contains("no supported"),
        "{}",
        result.message
    );
}

#[test]
fn a_cargo_package_is_counted_as_a_boundary() {
    let repo = repo_with(&[CARGO, LIB]);
    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let result = report
        .results
        .iter()
        .find(|result| result.check == "project-inventory")
        .expect("project-inventory reports a result");
    assert_eq!(result.status, CheckStatus::Pass);
    assert!(result.message.contains("detected 1"), "{}", result.message);
}
