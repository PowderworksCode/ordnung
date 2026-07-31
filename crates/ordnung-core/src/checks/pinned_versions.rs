use std::path::PathBuf;

use entl_codebase::{DependencyKind, DependencyPinStatus};
use entl_github::ActionPinStatus;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

const EXAMPLE_LIMIT: usize = 8;

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "pinned-versions",
    default_severity: Severity::Required,
    category: CheckCategory::Dependencies,
    instructions: "Use exact npm/Bun dependency versions and commit-SHA GitHub Action references; local dependencies are exempt, stable Action channels are allowed, and Cargo ranges remain advisory when Cargo.lock owns resolution.",
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
    let mut channels = 0usize;
    for reference in &context.inventory.github.action_references {
        match reference.pin_status {
            ActionPinStatus::Pinned => checked += 1,
            ActionPinStatus::Channel => {
                checked += 1;
                channels += 1;
            }
            ActionPinStatus::Local => {}
            ActionPinStatus::Floating => {
                checked += 1;
                violations.push(format!(
                    "{}:{} {}@{}",
                    reference.workflow.display(),
                    reference.job,
                    reference.action,
                    reference.reference.as_deref().unwrap_or("<missing>")
                ));
            }
        }
    }
    if checked == 0 {
        results.push(result(
            definition,
            CheckStatus::Skip,
            PathBuf::new(),
            "no pinnable dependency or GitHub Action references",
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
    if channels > 0 {
        details.push(format!(
            "{channels} allowed GitHub Action release channel(s)"
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
            format!("all {checked} dependency and Action references are pinned")
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

fn examples(values: &[String]) -> String {
    let shown = values
        .iter()
        .take(EXAMPLE_LIMIT)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if values.len() > EXAMPLE_LIMIT {
        format!("{shown} (+{} more)", values.len() - EXAMPLE_LIMIT)
    } else {
        shown
    }
}
