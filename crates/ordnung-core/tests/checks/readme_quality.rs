// Tests for `src/checks/readme_quality.rs`.
use crate::support::*;

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
