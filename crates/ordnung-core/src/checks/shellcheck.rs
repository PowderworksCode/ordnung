use std::path::PathBuf;

use entl_codebase::SHELLCHECK;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "shellcheck",
    default_severity: Severity::Off,
    category: CheckCategory::BuildToolchain,
    scope: CheckScope::Repository,
    instructions: "When the repository carries shell scripts, run ShellCheck over them from a push or pull-request workflow using its command or a registered GitHub Action.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    if context.inventory.shell_scripts.is_empty() {
        results.push(result(
            definition,
            CheckStatus::Skip,
            PathBuf::new(),
            "no shell scripts detected",
        ));
        return;
    }
    let invocations = context
        .inventory
        .github
        .tool_invocations
        .iter()
        .filter(|invocation| {
            invocation.runs_on_changes && invocation.tool.as_str() == SHELLCHECK.id
        })
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
            format!(
                "{} shell script(s) but no push or pull-request workflow runs ShellCheck",
                context.inventory.shell_scripts.len()
            )
        } else {
            format!(
                "ShellCheck runs in {}",
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
