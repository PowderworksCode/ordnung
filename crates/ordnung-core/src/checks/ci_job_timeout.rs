use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "ci-job-timeout",
    default_severity: Severity::Recommended,
    category: CheckCategory::CiSafety,
    instructions: "Give every push and pull-request CI job an explicit finite timeout; reusable-workflow jobs are exempt because GitHub does not allow the setting there.",
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

    let unbounded = context
        .inventory
        .github
        .workflows
        .iter()
        .filter(|workflow| workflow.runs_on_changes())
        .flat_map(|workflow| {
            workflow
                .jobs
                .iter()
                .filter(|job| job.uses.is_none() && job.timeout_minutes.is_none())
                .map(|job| format!("{}: {}", workflow.path.display(), job.name))
        })
        .collect::<Vec<_>>();
    results.push(result(
        definition,
        if unbounded.is_empty() {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        PathBuf::from(".github/workflows"),
        if unbounded.is_empty() {
            "every push and pull-request CI job has a finite timeout".to_owned()
        } else {
            format!("CI jobs without timeout-minutes: {}", unbounded.join(", "))
        },
    ));
}
