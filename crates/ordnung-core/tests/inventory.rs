use std::fs;
use std::path::Path;

use ordnung_core::{InventoryOptions, ProjectCapability, inspect_repository};

#[test]
fn detects_nested_rust_and_typescript_site() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/core\"]\n",
    )
    .unwrap();
    fs::write(temp.path().join("Cargo.lock"), "").unwrap();
    fs::create_dir_all(temp.path().join("crates/core/src")).unwrap();
    fs::write(
        temp.path().join("crates/core/Cargo.toml"),
        "[package]\nname = \"core\"\nversion = \"0.0.0\"\n",
    )
    .unwrap();
    fs::create_dir_all(temp.path().join("site")).unwrap();
    fs::write(
        temp.path().join("site/package.json"),
        r#"{"devDependencies":{"typescript":"1.0.0","vite":"1.0.0"}}"#,
    )
    .unwrap();
    fs::write(temp.path().join("site/bun.lock"), "").unwrap();
    fs::write(temp.path().join("site/tsconfig.json"), "{}").unwrap();

    let inventory = inspect_repository(temp.path(), &InventoryOptions::default()).unwrap();
    assert_eq!(inventory.projects.len(), 3);
    assert!(inventory.projects[0].has_capability(ProjectCapability::CargoWorkspace));
    assert!(inventory.projects[0].has_language("rust"));
    assert!(inventory.projects[0].uses_ecosystem("cargo"));
    let site = inventory
        .projects
        .iter()
        .find(|project| project.root == Path::new("site"))
        .unwrap();
    assert!(!site.has_language("javascript"));
    assert!(site.has_language("typescript"));
    assert!(site.has_capability(ProjectCapability::StaticSite));
    assert!(site.uses_ecosystem("bun"));
}

#[test]
fn broader_entl_languages_do_not_expand_ordnung_policy_support() {
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join("scripts")).unwrap();
    fs::write(repo.path().join("scripts/release.py"), "print('release')\n").unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    assert!(inventory.projects.is_empty());
}

#[test]
fn superseded_languages_are_not_graded_twice() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(
        repo.path().join("package.json"),
        r#"{"devDependencies":{"typescript":"1.0.0"}}"#,
    )
    .unwrap();
    fs::write(repo.path().join("tsconfig.json"), "{}\n").unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::write(repo.path().join("src/index.js"), "export {};\n").unwrap();
    fs::write(repo.path().join("src/index.ts"), "export {};\n").unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let root = inventory
        .projects
        .iter()
        .find(|project| project.root.as_os_str().is_empty())
        .unwrap();
    assert!(root.has_language("typescript"));
    assert!(!root.has_language("javascript"));
}

#[test]
fn skips_dependencies_and_explicit_ignores() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join("node_modules/fake")).unwrap();
    fs::write(temp.path().join("node_modules/fake/package.json"), "{}").unwrap();
    fs::create_dir_all(temp.path().join("experiments/demo")).unwrap();
    fs::write(temp.path().join("experiments/demo/package.json"), "{}").unwrap();

    let inventory = inspect_repository(
        temp.path(),
        &InventoryOptions {
            ignore: vec!["experiments/**".into()],
        },
    )
    .unwrap();
    assert!(inventory.projects.is_empty());
}
