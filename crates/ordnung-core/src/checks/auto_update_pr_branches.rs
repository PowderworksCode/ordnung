use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckStatus,
    GithubCheckContext, Severity, registry, result,
};
use crate::github::GithubValue;

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "auto-update-pr-branches",
    default_severity: Severity::Recommended,
    category: CheckCategory::GithubSafeguards,
    instructions: "Allow and automate pull-request branch updates when strict checks require freshness.",
    repository_runner: None,
    github_runner: Some(run),
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    facts: &GithubCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let strict = match &facts.branch.strict_status_checks {
        GithubValue::Known { value } => *value,
        GithubValue::Unavailable { reason } => {
            results.push(result(
                definition,
                CheckStatus::Error,
                PathBuf::new(),
                format!("could not determine whether strict checks apply: {reason}"),
            ));
            return;
        }
    };
    if !strict {
        results.push(result(
            definition,
            CheckStatus::Skip,
            PathBuf::new(),
            "strict status checks are disabled",
        ));
        return;
    }

    let workflow_present = facts.workflows.iter().any(|workflow| {
        workflow.state == "active"
            && workflow.path == ".github/workflows/auto-update-pr-branches.yml"
    });
    results.push(result(
        definition,
        if facts.allow_update_branch && workflow_present {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        PathBuf::from(".github/workflows/auto-update-pr-branches.yml"),
        if !facts.allow_update_branch {
            String::from("GitHub does not allow pull request branches to be updated")
        } else if !workflow_present {
            String::from("automatic pull request branch update workflow is missing")
        } else {
            String::from("pull request branches can be and are updated automatically")
        },
    ));
}
