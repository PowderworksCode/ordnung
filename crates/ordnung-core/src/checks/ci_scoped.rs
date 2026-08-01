use std::collections::BTreeSet;

use entl_codebase::{CiWorkload, tool_profile};

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "ci-scoped",
    default_severity: Severity::Recommended,
    category: CheckCategory::CiSafety,
    scope: CheckScope::Repository,
    instructions: "Gate heavy pull-request jobs with workflow path filters, a job condition, or a dependency on an output-producing fanout job.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let mut unscoped = BTreeSet::new();
    for workflow in context
        .inventory
        .github
        .workflows
        .iter()
        .filter(|workflow| workflow.triggers.contains("pull_request"))
    {
        if workflow.pull_request_path_filters {
            continue;
        }
        let fanout = workflow
            .jobs
            .iter()
            .filter(|job| job.has_outputs)
            .map(|job| job.id.as_str())
            .collect::<BTreeSet<_>>();
        for job in workflow.jobs.iter().filter(|job| !job.has_outputs) {
            let scoped = job.condition.is_some()
                || job
                    .needs
                    .iter()
                    .any(|dependency| fanout.contains(dependency.as_str()));
            if scoped {
                continue;
            }
            let heavy = workflow.tasks.iter().any(|task| {
                task.job == job.id
                    && tool_profile(task.tool.as_str())
                        .is_some_and(|tool| tool.ci_workload == CiWorkload::Heavy)
            });
            if heavy {
                unscoped.insert(format!("{}:{}", workflow.path.display(), job.name));
            }
        }
    }
    results.push(if unscoped.is_empty() {
        result(
            definition,
            CheckStatus::Pass,
            ".github/workflows".into(),
            "no heavy pull-request job runs without change scoping",
        )
    } else {
        result(
            definition,
            CheckStatus::Fail,
            ".github/workflows".into(),
            format!(
                "heavy jobs run on every pull request without change scoping: {}",
                unscoped.into_iter().collect::<Vec<_>>().join(", ")
            ),
        )
    });
}
