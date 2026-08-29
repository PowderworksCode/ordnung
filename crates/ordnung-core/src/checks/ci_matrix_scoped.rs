use std::collections::BTreeSet;

use entl_github::WorkflowJob;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "ci-matrix-scoped",
    default_severity: Severity::Recommended,
    category: CheckCategory::CiSafety,
    scope: CheckScope::Repository,
    instructions: "Let every pull-request matrix job short-circuit: scope the workflow with path filters, condition the job, or expand the matrix from a fanout job that inspects the change.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

/// Actions whose whole purpose is answering "what did this change touch".
const CHANGE_ACTIONS: [&str; 2] = ["dorny/paths-filter", "tj-actions/changed-files"];

/// Whether a job's own steps read the change, making its outputs a real
/// scoping signal rather than an enumeration of the repository. A fanout that
/// lists everything (`find`, a hardcoded list) expands the matrix identically
/// on every pull request, which is exactly what this check exists to catch.
fn inspects_change(job: &WorkflowJob) -> bool {
    job.steps.iter().any(|step| {
        if let Some(uses) = &step.uses {
            let action = uses.split('@').next().unwrap_or(uses).to_ascii_lowercase();
            if CHANGE_ACTIONS.contains(&action.as_str()) {
                return true;
            }
        }
        step.run
            .as_deref()
            .is_some_and(|run| run.contains("git diff") || run.contains("git merge-base"))
    })
}

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
        let change_aware = workflow
            .jobs
            .iter()
            .filter(|job| inspects_change(job))
            .map(|job| job.id.as_str())
            .collect::<BTreeSet<_>>();
        for job in &workflow.jobs {
            let Some(matrix) = &job.matrix else {
                continue;
            };
            let scoped = job.condition.is_some()
                || matrix
                    .from_needs
                    .iter()
                    .any(|source| change_aware.contains(source.as_str()));
            if !scoped {
                unscoped.insert(format!("{}:{}", workflow.path.display(), job.id));
            }
        }
    }
    results.push(if unscoped.is_empty() {
        result(
            definition,
            CheckStatus::Pass,
            ".github/workflows".into(),
            "every pull-request matrix job can short-circuit",
        )
    } else {
        result(
            definition,
            CheckStatus::Fail,
            ".github/workflows".into(),
            format!(
                "matrix jobs expand fully on every pull request: {}",
                unscoped.into_iter().collect::<Vec<_>>().join(", ")
            ),
        )
    });
}
