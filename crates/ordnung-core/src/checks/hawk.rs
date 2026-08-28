use std::path::PathBuf;

use entl_codebase::HAWK;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "hawk",
    default_severity: Severity::Off,
    category: CheckCategory::BuildToolchain,
    scope: CheckScope::Repository,
    instructions: "In Rust repositories, run Astral's hawk (cargo hawk) from a push or pull-request workflow to flag unnecessarily public APIs.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let rust = context.inventory.projects.iter().any(|project| {
        project
            .languages
            .iter()
            .any(|language| language.as_str() == "rust")
    });
    if !rust {
        results.push(result(
            definition,
            CheckStatus::Skip,
            PathBuf::new(),
            "no Rust project detected",
        ));
        return;
    }
    let invocations = context
        .inventory
        .github
        .tool_invocations
        .iter()
        .filter(|invocation| invocation.runs_on_changes && invocation.tool.as_str() == HAWK.id)
        .collect::<Vec<_>>();
    results.push(result(
        definition,
        if invocations.is_empty() {
            CheckStatus::Fail
        } else {
            CheckStatus::Pass
        },
        invocations.first().map_or_else(
            || PathBuf::from(".github/workflows"),
            |invocation| invocation.workflow.clone(),
        ),
        if invocations.is_empty() {
            "no push or pull-request workflow runs cargo hawk".into()
        } else {
            format!(
                "hawk runs in {}",
                invocations
                    .iter()
                    .map(|invocation| invocation.workflow.display().to_string())
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        },
    ));
}
