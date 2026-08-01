use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "ci-continue-on-error",
    default_severity: Severity::Required,
    category: CheckCategory::CiSafety,
    scope: CheckScope::Repository,
    instructions: "Do not let jobs or gating test, lint, format, typecheck, and build steps hide failures with continue-on-error.",
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

    let mut offenders = Vec::new();
    for workflow in &context.inventory.github.workflows {
        for job in &workflow.jobs {
            if job.continue_on_error {
                offenders.push(format!("{}: job '{}'", workflow.path.display(), job.name));
                continue;
            }
            for step in &job.steps {
                let gating = workflow
                    .tasks
                    .iter()
                    .any(|task| task.job == job.id && task.step == step.index);
                if step.continue_on_error && gating {
                    offenders.push(format!(
                        "{}: '{}'",
                        workflow.path.display(),
                        first_line(step.label())
                    ));
                }
            }
        }
    }

    results.push(result(
        definition,
        if offenders.is_empty() {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        PathBuf::from(".github/workflows"),
        if offenders.is_empty() {
            "no job or gating step masks failures with continue-on-error".to_owned()
        } else {
            format!(
                "continue-on-error hides gating failures: {}",
                offenders.join(", ")
            )
        },
    ));
}

fn first_line(value: &str) -> String {
    value
        .lines()
        .next()
        .unwrap_or(value)
        .chars()
        .take(60)
        .collect()
}
