use std::path::PathBuf;

use entl::codebase::ZIZMOR;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "zizmor",
    default_severity: Severity::Off,
    category: CheckCategory::CiSafety,
    scope: CheckScope::Repository,
    instructions: "Run zizmor static analysis over the repository's GitHub Actions workflows from a push or pull-request workflow, using its command or the zizmor-action.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    if context.inventory.github.workflow_files.is_empty() {
        results.push(result(
            definition,
            CheckStatus::Skip,
            PathBuf::from(".github/workflows"),
            "no GitHub Actions workflows to analyze",
        ));
        return;
    }
    let invocations = context
        .inventory
        .github
        .tool_invocations
        .iter()
        .filter(|invocation| invocation.runs_on_changes && invocation.tool.as_str() == ZIZMOR.id)
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
            "no push or pull-request workflow runs zizmor".into()
        } else {
            format!(
                "zizmor runs in {}",
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
