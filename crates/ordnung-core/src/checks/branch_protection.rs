use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    GithubCheckContext, Severity, registry, result,
};
use crate::github::GithubValue;

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "branch-protection",
    default_severity: Severity::Required,
    category: CheckCategory::GithubSafeguards,
    scope: CheckScope::Repository,
    instructions: "Require pull requests and block force pushes and deletion on the default branch.",
    repository_runner: None,
    github_runner: Some(run),
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    facts: &GithubCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let scope = PathBuf::from(&facts.default_branch);
    results.push(match &facts.branch.protection {
        GithubValue::Known { value }
            if value.pull_requests_required
                && value.force_pushes_blocked
                && value.deletion_blocked =>
        {
            result(
                definition,
                CheckStatus::Pass,
                scope,
                "pull requests are required and force pushes and deletion are blocked",
            )
        }
        GithubValue::Known { value } => {
            let mut missing = Vec::new();
            if !value.pull_requests_required {
                missing.push("pull requests are not required");
            }
            if !value.force_pushes_blocked {
                missing.push("force pushes are allowed");
            }
            if !value.deletion_blocked {
                missing.push("branch deletion is allowed");
            }
            result(
                definition,
                CheckStatus::Fail,
                scope,
                format!("default branch safeguards missing: {}", missing.join(", ")),
            )
        }
        GithubValue::Unavailable { reason } if facts.visibility == "private" => result(
            definition,
            CheckStatus::Skip,
            scope,
            format!("not available for this private repository: {reason}"),
        ),
        GithubValue::Unavailable { reason } => result(
            definition,
            CheckStatus::Error,
            scope,
            format!("could not read default-branch protection: {reason}"),
        ),
    });
}
