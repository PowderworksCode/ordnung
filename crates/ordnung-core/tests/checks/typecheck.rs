// Tests for `src/checks/typecheck.rs`.
use crate::support::*;

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

/// A type layer belongs to a package, not to a file extension. A tree-sitter
/// grammar crate carries a grammar.js and owes nobody a tsconfig; the site
/// beside it carries a package.json and does.
#[test]
fn typecheck_asks_only_where_a_package_declares_itself() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("crates/grammar/src")).unwrap();
    fs::create_dir_all(repo.path().join("site")).unwrap();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/grammar\"]\nresolver = \"2\"\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("crates/grammar/Cargo.toml"),
        "[package]\nname = \"grammar\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("crates/grammar/src/lib.rs"),
        "pub fn g() {}\n",
    )
    .unwrap();
    // The whole reason the crate looked like a JavaScript project.
    fs::write(
        repo.path().join("crates/grammar/grammar.js"),
        "module.exports = grammar({ name: 'g', rules: {} });\n",
    )
    .unwrap();
    fs::write(repo.path().join("site/package.json"), "{}").unwrap();
    fs::write(repo.path().join("site/worker.ts"), "export default {};\n").unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let report =
        run_repository_checks_with_repo_config(repo.path(), &inventory, &RepoConfig::default());
    let typecheck = report
        .results
        .iter()
        .filter(|result| result.check == "typecheck" && result.status == CheckStatus::Fail)
        .collect::<Vec<_>>();
    assert_eq!(
        typecheck.len(),
        1,
        "expected only the site to owe a type layer, got {:?}",
        typecheck
            .iter()
            .map(|result| result.scope.display().to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(typecheck[0].scope, std::path::Path::new("site"));
}
