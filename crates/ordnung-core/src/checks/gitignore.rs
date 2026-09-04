use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use entl::codebase::CARGO_ECOSYSTEM;
use ignore::gitignore::GitignoreBuilder;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "gitignore",
    default_severity: Severity::Required,
    category: CheckCategory::RepositoryShape,
    scope: CheckScope::Project,
    instructions: "Ignore each ecosystem's build junk at every package scope: Cargo requires target/ and Bun, npm, pnpm, and Yarn require node_modules/; applicable ancestor .gitignore files may provide the rule.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

struct Requirement {
    ecosystem: &'static str,
    scope: PathBuf,
    pattern: &'static str,
}

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let requirements = requirements(context);
    if requirements.is_empty() {
        results.push(result(
            definition,
            CheckStatus::Skip,
            PathBuf::from(".gitignore"),
            "no ecosystems with known build junk detected",
        ));
        return;
    }

    for requirement in requirements {
        let target = requirement
            .scope
            .join(requirement.pattern.trim_end_matches('/'));
        let (status, message) = match ignored(context.root, &requirement.scope, requirement.pattern)
        {
            Ok(true) => (
                CheckStatus::Pass,
                format!(
                    "{} ignores {} at {}",
                    requirement.ecosystem,
                    requirement.pattern,
                    display_root(&requirement.scope)
                ),
            ),
            Ok(false) => (
                CheckStatus::Fail,
                format!(
                    "{} is missing {} coverage at {}",
                    requirement.ecosystem,
                    requirement.pattern,
                    display_root(&requirement.scope)
                ),
            ),
            Err(message) => (CheckStatus::Error, message),
        };
        results.push(result(definition, status, target, message));
    }
}

fn requirements(context: &RepositoryCheckContext<'_>) -> Vec<Requirement> {
    let mut seen = BTreeSet::new();
    let mut requirements = Vec::new();
    for package in &context.inventory.packages {
        let profile = package.ecosystem_profile();
        let scope = if std::ptr::eq(profile, &CARGO_ECOSYSTEM) {
            package.lockfile_owner.clone()
        } else {
            package.root.clone()
        };
        for pattern in profile.gitignore_patterns {
            if seen.insert((scope.clone(), *pattern)) {
                requirements.push(Requirement {
                    ecosystem: profile.display_name,
                    scope: scope.clone(),
                    pattern,
                });
            }
        }
    }
    requirements.sort_by(|left, right| {
        left.scope
            .cmp(&right.scope)
            .then(left.pattern.cmp(right.pattern))
    });
    requirements
}

fn ignored(root: &Path, scope: &Path, pattern: &str) -> Result<bool, String> {
    let target = root.join(scope).join(pattern.trim_end_matches('/'));
    let mut matched = match_gitignore(root, &target)?;
    let mut relative = PathBuf::new();
    for component in scope.components() {
        relative.push(component);
        let directory = root.join(&relative);
        if let Some(value) = match_gitignore(&directory, &target)? {
            matched = Some(value);
        }
    }
    Ok(matched.unwrap_or(false))
}

fn match_gitignore(directory: &Path, target: &Path) -> Result<Option<bool>, String> {
    let path = directory.join(".gitignore");
    if !path.is_file() {
        return Ok(None);
    }
    let mut builder = GitignoreBuilder::new(directory);
    if let Some(error) = builder.add(&path) {
        return Err(format!("could not parse {}: {error}", path.display()));
    }
    let matcher = builder
        .build()
        .map_err(|error| format!("could not build .gitignore matcher: {error}"))?;
    let matched = matcher.matched(target, true);
    if matched.is_ignore() {
        Ok(Some(true))
    } else if matched.is_whitelist() {
        Ok(Some(false))
    } else {
        Ok(None)
    }
}

fn display_root(path: &Path) -> String {
    if path.as_os_str().is_empty() {
        ".".to_owned()
    } else {
        path.to_string_lossy().replace('\\', "/")
    }
}
