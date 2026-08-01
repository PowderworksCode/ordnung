use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    GithubCheckContext, Severity, registry, result,
};
use crate::github::{GithubDefaultWorkflowPermissions, GithubValue};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "workflow-permissions",
    default_severity: Severity::Required,
    category: CheckCategory::CiSafety,
    scope: CheckScope::Repository,
    instructions: "Keep the repository's default GITHUB_TOKEN read-only and prevent workflows from approving pull requests; jobs that need write access must grant it explicitly with a permissions block.",
    repository_runner: None,
    github_runner: Some(run),
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    facts: &GithubCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    results.push(match &facts.actions_permissions {
        GithubValue::Known { value }
            if value.default_workflow_permissions == GithubDefaultWorkflowPermissions::Read
                && !value.can_approve_pull_request_reviews =>
        {
            result(
                definition,
                CheckStatus::Pass,
                PathBuf::new(),
                "default GITHUB_TOKEN is read-only and cannot approve pull requests",
            )
        }
        GithubValue::Known { value } => {
            let mut problems = Vec::new();
            if value.default_workflow_permissions == GithubDefaultWorkflowPermissions::Write {
                problems.push("default GITHUB_TOKEN is read-write");
            }
            if value.can_approve_pull_request_reviews {
                problems.push("workflows can approve pull requests");
            }
            result(
                definition,
                CheckStatus::Fail,
                PathBuf::new(),
                format!(
                    "{}; jobs needing write access must declare permissions explicitly",
                    problems.join("; ")
                ),
            )
        }
        GithubValue::Unavailable { reason } => result(
            definition,
            CheckStatus::Skip,
            PathBuf::new(),
            format!("workflow permissions are not visible to this token: {reason}"),
        ),
    });
}
