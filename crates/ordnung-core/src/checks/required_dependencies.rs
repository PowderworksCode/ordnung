use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

const EXAMPLE_LIMIT: usize = 8;

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "required-dependencies",
    default_severity: Severity::Required,
    category: CheckCategory::Dependencies,
    scope: CheckScope::Project,
    instructions: "Declare every package the effective policy requires for a project's language or ecosystem; a workspace member may inherit the declaration from its workspace root.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    if context.dependencies.is_empty() {
        results.push(result(
            definition,
            CheckStatus::Skip,
            PathBuf::new(),
            "no dependency requirements are configured",
        ));
        return;
    }

    let mut satisfied = 0usize;
    let mut missing = Vec::new();
    for package in &context.inventory.packages {
        if is_workspace_aggregate(package) {
            continue;
        }
        let languages = package_languages(context, package);
        let ecosystem = package.ecosystem.as_str();
        for requirement in context.dependencies {
            if !requirement.matches(languages.iter().copied(), ecosystem) {
                continue;
            }
            for wanted in &requirement.require {
                if declares(&package.dependencies, wanted, requirement.kind) {
                    satisfied += 1;
                } else {
                    missing.push(format!(
                        "{}: {wanted} ({})",
                        display_root(&package.root),
                        requirement.name
                    ));
                }
            }
        }
    }

    if missing.is_empty() {
        let detail = if satisfied == 0 {
            "no package matches a dependency requirement".to_owned()
        } else {
            format!("all {satisfied} required dependency declaration(s) are present")
        };
        results.push(result(
            definition,
            CheckStatus::Pass,
            PathBuf::new(),
            detail,
        ));
        return;
    }

    let shown = missing
        .iter()
        .take(EXAMPLE_LIMIT)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let detail = if missing.len() > EXAMPLE_LIMIT {
        format!(
            "{} required dependency declaration(s) missing: {shown}, and {} more",
            missing.len(),
            missing.len() - EXAMPLE_LIMIT
        )
    } else {
        format!("required dependency declaration(s) missing: {shown}")
    };
    results.push(result(
        definition,
        CheckStatus::Fail,
        PathBuf::new(),
        detail,
    ));
}

/// The package's manifest language plus the languages of the project rooted at the
/// same path. A Bun or npm package reports `javascript` from its manifest while the
/// project around it is `typescript`, and a requirement should match either name.
fn package_languages<'a>(
    context: &'a RepositoryCheckContext<'_>,
    package: &'a crate::inventory::PackageInstance,
) -> Vec<&'a str> {
    let mut languages = vec![package.language.as_str()];
    for project in &context.inventory.projects {
        if project.root == package.root {
            languages.extend(project.languages.iter().map(|language| language.as_str()));
        }
    }
    languages.sort_unstable();
    languages.dedup();
    languages
}

/// A workspace root is a synthesized aggregation entry that never carries manifest
/// dependencies of its own, so requiring packages of it would always fail. A
/// standalone package has no workspace root and is always checked. Members resolve
/// their own inherited declarations, so nothing has to be borrowed from the root.
fn is_workspace_aggregate(package: &crate::inventory::PackageInstance) -> bool {
    package.dependencies.is_empty()
        && package
            .workspace_root
            .as_ref()
            .is_some_and(|root| root == &package.root)
}

fn declares(
    dependencies: &[entl_codebase::Dependency],
    wanted: &str,
    kind: Option<entl_codebase::DependencyKind>,
) -> bool {
    dependencies.iter().any(|dependency| {
        dependency.package_name() == wanted && kind.is_none_or(|kind| dependency.kind == kind)
    })
}

fn display_root(root: &std::path::Path) -> String {
    if root.as_os_str().is_empty() {
        ".".to_owned()
    } else {
        root.display().to_string()
    }
}
