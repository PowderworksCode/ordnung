// Tests for `src/checks/required_dependencies.rs`.
use crate::support::*;

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
