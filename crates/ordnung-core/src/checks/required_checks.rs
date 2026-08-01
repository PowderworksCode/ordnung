use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    GithubCheckContext, Severity, registry, result,
};
use crate::github::GithubValue;

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "required-checks",
    default_severity: Severity::Recommended,
    category: CheckCategory::GithubSafeguards,
    scope: CheckScope::Repository,
    instructions: "Require every check posted by pull-request workflows before default-branch changes merge.",
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
    let pr_checks = match &facts.pull_request_checks {
        GithubValue::Known { value } if value.is_empty() => {
            results.push(result(
                definition,
                CheckStatus::Skip,
                scope,
                "no pull-request workflow posts a status check",
            ));
            return;
        }
        GithubValue::Known { value } => value,
        GithubValue::Unavailable { reason } => {
            results.push(result(
                definition,
                CheckStatus::Error,
                scope,
                format!("could not inspect pull-request workflow jobs: {reason}"),
            ));
            return;
        }
    };

    results.push(match &facts.branch.required_checks {
        GithubValue::Known { value } => {
            let required = value.iter().map(String::as_str).collect::<BTreeSet<_>>();
            let missing = pr_checks
                .iter()
                .filter(|check| !required.contains(check.as_str()))
                .map(String::as_str)
                .collect::<Vec<_>>();
            if missing.is_empty() {
                result(
                    definition,
                    CheckStatus::Pass,
                    scope,
                    format!(
                        "default branch requires all {} pull-request checks: {}",
                        pr_checks.len(),
                        pr_checks.join(", ")
                    ),
                )
            } else if value.is_empty() {
                result(
                    definition,
                    CheckStatus::Fail,
                    scope,
                    format!(
                        "default branch requires no status checks; require: {}",
                        missing.join(", ")
                    ),
                )
            } else {
                result(
                    definition,
                    CheckStatus::Fail,
                    scope,
                    format!(
                        "pull-request checks not required on the default branch: {}",
                        missing.join(", ")
                    ),
                )
            }
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
            format!("could not read required status checks: {reason}"),
        ),
    });
}
