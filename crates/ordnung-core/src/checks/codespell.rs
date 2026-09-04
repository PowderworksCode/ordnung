use std::path::PathBuf;

use entl::codebase::CODESPELL;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "codespell",
    default_severity: Severity::Recommended,
    category: CheckCategory::Documentation,
    scope: CheckScope::Repository,
    instructions: "Run Codespell from a push or pull-request workflow using its command or registered GitHub Action.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let invocations = context
        .inventory
        .github
        .tool_invocations
        .iter()
        .filter(|invocation| invocation.runs_on_changes && invocation.tool.as_str() == CODESPELL.id)
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
            "no push or pull-request workflow runs Codespell".into()
        } else {
            format!(
                "Codespell runs in {}",
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
