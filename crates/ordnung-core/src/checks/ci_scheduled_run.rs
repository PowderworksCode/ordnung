use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "ci-scheduled-run",
    default_severity: Severity::Recommended,
    category: CheckCategory::CiSafety,
    instructions: "Run validation on a schedule when periodic coverage should expose repository bitrot between changes.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    if !context.inventory.github.has_workflows() {
        results.push(result(
            definition,
            CheckStatus::Skip,
            PathBuf::from(".github/workflows"),
            "no workflows found; ci-exists covers their absence",
        ));
        return;
    }

    let scheduled = context
        .inventory
        .github
        .workflows
        .iter()
        .filter(|workflow| workflow.triggers.contains("schedule"))
        .map(|workflow| workflow.path.display().to_string())
        .collect::<Vec<_>>();
    results.push(result(
        definition,
        if scheduled.is_empty() {
            CheckStatus::Fail
        } else {
            CheckStatus::Pass
        },
        PathBuf::from(".github/workflows"),
        if scheduled.is_empty() {
            "no workflow runs on a schedule".to_owned()
        } else {
            format!(
                "scheduled validation configured in: {}",
                scheduled.join(", ")
            )
        },
    ));
}
