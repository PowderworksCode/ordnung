use std::path::PathBuf;

use entl_codebase::{DependencyKind, DependencyPinStatus};

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

use super::examples;

/// Exact package versions are a preference rather than a consensus: the committed
/// lockfile already fixes what gets installed, and exact requirements work against
/// automated dependency updates. Action pinning is the security-relevant half and
/// lives in `pinned-actions`.
pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "pinned-dependencies",
    default_severity: Severity::Recommended,
    category: CheckCategory::Dependencies,
    scope: CheckScope::Project,
    instructions: "Use exact npm/Bun dependency versions; local dependencies are exempt and Cargo ranges stay advisory because Cargo.lock owns resolution.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let mut checked = 0usize;
    let mut violations = Vec::new();
    let mut advisory = Vec::new();
    for package in &context.inventory.packages {
        let Some(policy) = package.ecosystem_profile().dependency_pins else {
            continue;
        };
        for dependency in &package.dependencies {
            if dependency.kind == DependencyKind::Peer {
                continue;
            }
            match policy.classify(dependency) {
                DependencyPinStatus::Local => {}
                DependencyPinStatus::Pinned => checked += 1,
                DependencyPinStatus::Floating => {
                    checked += 1;
                    let description = format!(
                        "{}:{} {} {}",
                        display_root(&package.root),
                        dependency.name,
                        dependency.requirement.as_deref().unwrap_or("<unspecified>"),
                        package.ecosystem
                    );
                    if policy.advisory {
                        advisory.push(description);
                    } else {
                        violations.push(description);
                    }
                }
            }
        }
    }

    if checked == 0 {
        results.push(result(
            definition,
            CheckStatus::Skip,
            PathBuf::new(),
            "no pinnable dependency references",
        ));
        return;
    }
    let mut details = Vec::new();
    if !violations.is_empty() {
        details.push(format!(
            "{} floating reference(s): {}",
            violations.len(),
            examples(&violations)
        ));
    }
    if !advisory.is_empty() {
        details.push(format!(
            "Cargo advisory: {} floating reference(s): {}",
            advisory.len(),
            examples(&advisory)
        ));
    }
    results.push(result(
        definition,
        if violations.is_empty() {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        PathBuf::new(),
        if details.is_empty() {
            format!("all {checked} dependency reference(s) are pinned")
        } else {
            details.join("; ")
        },
    ));
}

fn display_root(root: &std::path::Path) -> String {
    if root.as_os_str().is_empty() {
        "/".to_owned()
    } else {
        root.display().to_string()
    }
}
