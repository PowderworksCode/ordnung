use std::path::{Path, PathBuf};

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

use super::examples;

/// The conventional location for hooks a repository commits rather than a
/// developer writes by hand.
const HOOKS_DIR: &str = ".githooks";

/// Client-side hooks worth committing. A file in the hooks directory that is not
/// one of these is documentation or a helper, not a hook Git will ever run.
const HOOK_NAMES: &[&str] = &[
    "applypatch-msg",
    "commit-msg",
    "post-checkout",
    "post-commit",
    "post-merge",
    "post-rewrite",
    "pre-applypatch",
    "pre-auto-gc",
    "pre-commit",
    "pre-merge-commit",
    "pre-push",
    "pre-rebase",
    "prepare-commit-msg",
];

/// Managers that install hooks through their own lifecycle, so the repository does
/// not wire up `core.hooksPath` itself.
const MANAGER_PACKAGES: &[&str] = &["husky", "lefthook", "simple-git-hooks", "cargo-husky"];
const MANAGER_FILES: &[&str] = &[
    "lefthook.yml",
    "lefthook.yaml",
    ".lefthook.yml",
    ".lefthook.yaml",
    ".pre-commit-config.yaml",
    ".pre-commit-config.yml",
];

/// Off by default: committing hooks is a real practice but far from consensus, and
/// a default that fires on a legitimate choice teaches its reader to ignore output.
///
/// This checks that a repository *provides* hooks and wires them up, never that they
/// are active. Whether `core.hooksPath` is set is local machine state, and a fleet
/// audit reads a fresh clone where it is always unset, so "are hooks installed?" is
/// not a question the repository can answer.
pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "git-hooks",
    default_severity: Severity::Off,
    category: CheckCategory::CiSafety,
    scope: CheckScope::Repository,
    instructions: "Commit the repository's Git hooks under .githooks, keep every hook file executable, and have the development script point core.hooksPath at it; a declared hook manager installs itself instead.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    if let Some(manager) = declared_manager(context) {
        // The manager owns installation through its own lifecycle, so requiring the
        // development script to repeat that would be wrong.
        results.push(result(
            definition,
            CheckStatus::Pass,
            PathBuf::new(),
            format!("hooks are managed by {manager}, which installs them itself"),
        ));
        return;
    }

    let hooks = committed_hooks(context);
    if hooks.is_empty() {
        results.push(result(
            definition,
            CheckStatus::Fail,
            PathBuf::from(HOOKS_DIR),
            format!(
                "no committed hooks in {HOOKS_DIR} and no hook manager declared; \
                 a gate that only runs in CI reports failures after a push"
            ),
        ));
        return;
    }

    let mut problems = Vec::new();
    let not_executable = hooks
        .iter()
        .filter(|hook| !is_executable(&context.root.join(hook)))
        .map(|hook| hook.display().to_string())
        .collect::<Vec<_>>();
    if !not_executable.is_empty() {
        // Git silently ignores a hook without the execute bit, so this fails closed
        // in the worst way: the gate looks present and never runs.
        problems.push(format!(
            "not executable, so Git will not run {}: {}",
            if not_executable.len() == 1 {
                "it"
            } else {
                "them"
            },
            examples(&not_executable)
        ));
    }

    let development = context.scripts.development_path();
    match activation(context, &development) {
        Activation::Wired => {}
        Activation::Missing => problems.push(format!(
            "{} does not set core.hooksPath, so a fresh clone runs no hooks",
            development.display()
        )),
        Activation::NoScript => problems.push(format!(
            "no {} to set core.hooksPath, so a fresh clone runs no hooks",
            development.display()
        )),
    }

    results.push(result(
        definition,
        if problems.is_empty() {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        PathBuf::from(HOOKS_DIR),
        if problems.is_empty() {
            format!(
                "{} committed hook(s) are executable and installed by {}",
                hooks.len(),
                development.display()
            )
        } else {
            problems.join("; ")
        },
    ));
}

/// Hook files under the conventional directory, ignoring documentation and helpers.
fn committed_hooks(context: &RepositoryCheckContext<'_>) -> Vec<PathBuf> {
    context
        .inventory
        .files
        .iter()
        .filter(|path| {
            path.parent()
                .is_some_and(|parent| parent == Path::new(HOOKS_DIR))
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| HOOK_NAMES.contains(&name))
        })
        .cloned()
        .collect()
}

fn declared_manager(context: &RepositoryCheckContext<'_>) -> Option<&'static str> {
    for file in MANAGER_FILES {
        if context.inventory.files.contains(&PathBuf::from(file)) {
            return Some(file);
        }
    }
    context.inventory.packages.iter().find_map(|package| {
        package.dependencies.iter().find_map(|dependency| {
            MANAGER_PACKAGES
                .iter()
                .find(|manager| **manager == dependency.package_name())
                .copied()
        })
    })
}

enum Activation {
    Wired,
    Missing,
    NoScript,
}

fn activation(context: &RepositoryCheckContext<'_>, development: &Path) -> Activation {
    let path = context.root.join(development);
    match std::fs::read_to_string(&path) {
        Ok(text) if text.contains("hooksPath") => Activation::Wired,
        Ok(_) => Activation::Missing,
        Err(_) => Activation::NoScript,
    }
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
}

/// Other platforms do not carry an execute bit, so the mode cannot be graded there.
#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    true
}
