use std::fs;
use std::path::{Path, PathBuf};

use ordnung_core::{InventoryOptions, ProjectCapability, inspect_repository};

fn write(root: &Path, path: &str, content: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

#[test]
fn cargo_workspace_ownership_uses_members_and_excludes() {
    let repo = tempfile::tempdir().unwrap();
    write(
        repo.path(),
        "Cargo.toml",
        "[workspace]\nmembers = ['crates/*']\nexclude = ['crates/standalone']\n",
    );
    write(repo.path(), "Cargo.lock", "");
    write(
        repo.path(),
        "crates/member/Cargo.toml",
        "[package]\nname = 'member'\nversion = '0.0.0'\n",
    );
    write(
        repo.path(),
        "crates/standalone/Cargo.toml",
        "[package]\nname = 'standalone'\nversion = '0.0.0'\n",
    );
    write(repo.path(), "crates/standalone/Cargo.lock", "");

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let workspace = inventory
        .packages
        .iter()
        .find(|package| package.root.as_os_str().is_empty())
        .unwrap();
    assert_eq!(workspace.workspace_root, Some(PathBuf::new()));
    assert!(!workspace.is_workspace_member());
    let member = inventory
        .packages
        .iter()
        .find(|package| package.root == Path::new("crates/member"))
        .unwrap();
    assert_eq!(member.workspace_root, Some(PathBuf::new()));
    assert_eq!(member.lockfile_owner, Path::new(""));
    assert_eq!(member.lockfile, Some(PathBuf::from("Cargo.lock")));

    let standalone = inventory
        .packages
        .iter()
        .find(|package| package.root == Path::new("crates/standalone"))
        .unwrap();
    assert_eq!(standalone.workspace_root, None);
    assert_eq!(standalone.lockfile_owner, Path::new("crates/standalone"));
    assert_eq!(
        standalone.lockfile,
        Some(PathBuf::from("crates/standalone/Cargo.lock"))
    );
}

#[test]
fn explicit_cargo_workspace_link_is_resolved() {
    let repo = tempfile::tempdir().unwrap();
    write(repo.path(), "Cargo.toml", "[workspace]\n");
    write(repo.path(), "Cargo.lock", "");
    write(
        repo.path(),
        "tools/member/Cargo.toml",
        "[package]\nname = 'member'\nversion = '0.0.0'\nworkspace = '../..'\n",
    );

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let member = inventory
        .packages
        .iter()
        .find(|package| package.root == Path::new("tools/member"))
        .unwrap();
    assert_eq!(member.workspace_root, Some(PathBuf::new()));
    assert_eq!(member.lockfile, Some(PathBuf::from("Cargo.lock")));
}

#[test]
fn node_workspace_members_inherit_manager_and_lockfile_owner() {
    let repo = tempfile::tempdir().unwrap();
    write(
        repo.path(),
        "package.json",
        r#"{"packageManager":"pnpm@10.0.0","workspaces":["packages/*"]}"#,
    );
    write(repo.path(), "pnpm-lock.yaml", "");
    write(
        repo.path(),
        "pnpm-workspace.yaml",
        "packages:\n  - packages/*\n",
    );
    write(
        repo.path(),
        "packages/site/package.json",
        r#"{"devDependencies":{"typescript":"1.0.0","vite":"1.0.0"}}"#,
    );
    write(repo.path(), "packages/site/tsconfig.json", "{}");

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let workspace = inventory
        .packages
        .iter()
        .find(|package| package.root.as_os_str().is_empty())
        .unwrap();
    assert_eq!(workspace.workspace_root, Some(PathBuf::new()));
    let package = inventory
        .packages
        .iter()
        .find(|package| package.root == Path::new("packages/site"))
        .unwrap();
    assert_eq!(package.ecosystem.as_str(), "pnpm");
    assert_eq!(package.workspace_root, Some(PathBuf::new()));
    assert_eq!(package.lockfile_owner, Path::new(""));
    assert_eq!(package.lockfile, Some(PathBuf::from("pnpm-lock.yaml")));

    let project = inventory
        .projects
        .iter()
        .find(|project| project.root == Path::new("packages/site"))
        .unwrap();
    assert!(project.has_language("typescript"));
    assert!(project.has_capability(ProjectCapability::StaticSite));
}

#[test]
fn pnpm_workspace_exclusions_leave_packages_independent() {
    let repo = tempfile::tempdir().unwrap();
    write(repo.path(), "package.json", "{}");
    write(repo.path(), "pnpm-lock.yaml", "");
    write(
        repo.path(),
        "pnpm-workspace.yaml",
        "packages:\n  - packages/*\n  - '!packages/private'\n",
    );
    write(repo.path(), "packages/public/package.json", "{}");
    write(repo.path(), "packages/private/package.json", "{}");

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let public = inventory
        .packages
        .iter()
        .find(|package| package.root == Path::new("packages/public"))
        .unwrap();
    let private = inventory
        .packages
        .iter()
        .find(|package| package.root == Path::new("packages/private"))
        .unwrap();
    assert_eq!(public.workspace_root, Some(PathBuf::new()));
    assert_eq!(private.workspace_root, None);
    assert_eq!(private.ecosystem.as_str(), "npm");
}

#[test]
fn package_manager_field_selects_manager_without_a_lockfile() {
    let repo = tempfile::tempdir().unwrap();
    write(
        repo.path(),
        "package.json",
        r#"{"packageManager":"yarn@4.6.0"}"#,
    );

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    assert_eq!(inventory.packages[0].ecosystem.as_str(), "yarn");
    assert_eq!(inventory.packages[0].lockfile, None);
}

#[test]
fn conflicting_manager_evidence_is_reported() {
    let repo = tempfile::tempdir().unwrap();
    write(
        repo.path(),
        "package.json",
        r#"{"packageManager":"pnpm@10.0.0"}"#,
    );
    write(repo.path(), "bun.lock", "");
    write(repo.path(), "pnpm-lock.yaml", "");

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    assert!(inventory.issues.iter().any(|issue| {
        issue.path == Path::new("package.json")
            && issue
                .message
                .contains("conflicting package-manager lockfiles")
    }));
}

#[test]
fn invalid_workspace_and_manager_declarations_are_reported() {
    let repo = tempfile::tempdir().unwrap();
    write(
        repo.path(),
        "package.json",
        r#"{"packageManager":"mystery@1.0.0","workspaces":["packages/["]}"#,
    );
    write(
        repo.path(),
        "cargo/Cargo.toml",
        "[package]\nname='member'\nversion='0.0.0'\nworkspace='../missing'\n",
    );

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    assert!(
        inventory
            .issues
            .iter()
            .any(|issue| issue.message.contains("unsupported packageManager"))
    );
    assert!(
        inventory
            .issues
            .iter()
            .any(|issue| issue.message.contains("invalid workspace pattern"))
    );
    assert!(inventory.issues.iter().any(|issue| {
        issue
            .message
            .contains("no workspace manifest was found there")
    }));
}

#[test]
fn invalid_manifests_are_diagnostics_without_inventing_packages() {
    let repo = tempfile::tempdir().unwrap();
    write(repo.path(), "Cargo.toml", "not = [valid");
    write(repo.path(), "web/package.json", "{");

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    assert!(inventory.packages.is_empty());
    assert_eq!(inventory.issues.len(), 2);
}

#[test]
fn walker_honors_hidden_generated_gitignore_and_explicit_patterns() {
    let repo = tempfile::tempdir().unwrap();
    write(repo.path(), ".gitignore", "ignored-by-git/\n");
    write(repo.path(), ".hidden/package.json", "{}");
    write(repo.path(), "node_modules/pkg/package.json", "{}");
    write(repo.path(), "ignored-by-git/package.json", "{}");
    write(repo.path(), "explicit/package.json", "{}");
    write(repo.path(), "kept/package.json", "{}");

    let inventory = inspect_repository(
        repo.path(),
        &InventoryOptions {
            ignore: vec!["explicit/**".into()],
        },
    )
    .unwrap();
    assert!(
        inventory
            .projects
            .iter()
            .any(|project| project.root == Path::new("kept"))
    );
    assert!(inventory.projects.iter().all(|project| {
        !matches!(
            project.root.to_str(),
            Some(".hidden" | "node_modules/pkg" | "ignored-by-git" | "explicit")
        )
    }));
}

#[cfg(unix)]
#[test]
fn walker_does_not_follow_directory_symlinks() {
    use std::os::unix::fs::symlink;

    let repo = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write(outside.path(), "package.json", "{}");
    symlink(outside.path(), repo.path().join("linked")).unwrap();

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    assert!(inventory.projects.is_empty());
}

#[test]
fn static_site_configuration_requires_a_package_boundary() {
    let repo = tempfile::tempdir().unwrap();
    write(repo.path(), "orphan/vite.config.ts", "");
    write(repo.path(), "site/package.json", "{}");
    write(repo.path(), "site/vite.config.ts", "");

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    assert!(
        inventory
            .projects
            .iter()
            .all(|project| project.root != Path::new("orphan"))
    );
    assert!(
        inventory
            .projects
            .iter()
            .find(|project| project.root == Path::new("site"))
            .unwrap()
            .has_capability(ProjectCapability::StaticSite)
    );
}

#[test]
fn source_files_detect_languages_without_creating_nested_project_boundaries() {
    let repo = tempfile::tempdir().unwrap();
    write(repo.path(), "scripts/release.ts", "export {};\n");
    write(repo.path(), "tools/helper.rs", "fn helper() {}\n");

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    assert_eq!(inventory.projects.len(), 1);
    assert_eq!(inventory.projects[0].root, Path::new(""));
    assert!(inventory.projects[0].has_language("typescript"));
    assert!(inventory.projects[0].has_language("rust"));
}

#[test]
fn nested_package_owns_its_source_language_evidence() {
    let repo = tempfile::tempdir().unwrap();
    write(repo.path(), "package.json", "{}");
    write(
        repo.path(),
        "packages/tool/Cargo.toml",
        "[package]\nname='tool'\nversion='0.0.0'\n",
    );
    write(
        repo.path(),
        "packages/tool/src/lib.rs",
        "pub fn tool() {}\n",
    );

    let inventory = inspect_repository(repo.path(), &InventoryOptions::default()).unwrap();
    let root = inventory
        .projects
        .iter()
        .find(|project| project.root == Path::new(""))
        .unwrap();
    let tool = inventory
        .projects
        .iter()
        .find(|project| project.root == Path::new("packages/tool"))
        .unwrap();
    assert!(!root.has_language("rust"));
    assert!(tool.has_language("rust"));
}
