use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckStatus,
    GithubCheckContext, Severity, registry, result,
};
use crate::github::GithubValue;

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "strict-status-checks",
    default_severity: Severity::Recommended,
    category: CheckCategory::GithubSafeguards,
    instructions: "Require status checks to run against the latest default-branch state.",
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
    results.push(match &facts.branch.strict_status_checks {
        GithubValue::Known { value: true } => {
            let mut message =
                "required checks must run against the latest default branch".to_owned();
            if !facts.allow_update_branch {
                message.push_str("; enable update-branch suggestions to help contributors stay current");
            }
            result(definition, CheckStatus::Pass, scope, message)
        }
        GithubValue::Known { value: false } => {
            let mut message = match &facts.branch.required_checks {
                GithubValue::Known { value } if value.is_empty() =>
                    "no required status checks are configured; configure them before enabling strict mode"
                        .to_owned(),
                _ => "required checks do not require the latest default branch".to_owned(),
            };
            if !facts.allow_update_branch {
                message.push_str("; update-branch suggestions are also disabled");
            }
            result(definition, CheckStatus::Fail, scope, message)
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
            format!("could not read strict status-check policy: {reason}"),
        ),
    });
}
