use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "reproducible-toolchain",
    default_severity: Severity::Required,
    category: CheckCategory::BuildToolchain,
    scope: CheckScope::Repository,
    instructions: "Keep GitHub setup-action toolchain inputs off unbounded latest and wildcard versions; explicit versions and bounded stable channels are allowed.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let floating = context
        .inventory
        .github
        .workflows
        .iter()
        .flat_map(|workflow| {
            workflow.jobs.iter().flat_map(|job| {
                job.steps.iter().flat_map(|step| {
                    let setup_action = step
                        .uses
                        .as_deref()
                        .is_some_and(|action| action.contains("setup-"));
                    step.inputs
                        .iter()
                        .filter(move |(key, value)| {
                            setup_action
                                && (key.as_str() == "version" || key.ends_with("-version"))
                                && is_floating(value)
                        })
                        .map(|(key, value)| format!("{}: {key}: {value}", workflow.path.display()))
                })
            })
        })
        .collect::<Vec<_>>();

    results.push(result(
        definition,
        if floating.is_empty() {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        PathBuf::from(".github/workflows"),
        if floating.is_empty() {
            "no GitHub setup action uses an unbounded toolchain version".to_owned()
        } else {
            format!(
                "CI builds on floating toolchain versions: {}",
                floating.join(", ")
            )
        },
    ));
}

fn is_floating(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    if matches!(value.as_str(), "latest" | "*" | "x") {
        return true;
    }
    let mut components = value.split('.').collect::<Vec<_>>();
    let Some(last) = components.pop() else {
        return false;
    };
    matches!(last, "x" | "*")
        && !components.is_empty()
        && components.iter().all(|component| {
            component
                .chars()
                .all(|character| character.is_ascii_digit())
        })
}
