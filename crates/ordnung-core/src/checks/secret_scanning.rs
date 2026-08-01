use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    GithubCheckContext, Severity, registry, result,
};
use crate::github::GithubValue;

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "secret-scanning",
    default_severity: Severity::Required,
    category: CheckCategory::GithubSafeguards,
    scope: CheckScope::Repository,
    instructions: "Keep secret scanning and push protection enabled where available.",
    repository_runner: None,
    github_runner: Some(run),
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    facts: &GithubCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    results.push(match &facts.security {
        GithubValue::Known { value } if value.secret_scanning && value.push_protection => result(
            definition,
            CheckStatus::Pass,
            PathBuf::new(),
            "secret scanning and push protection are enabled",
        ),
        GithubValue::Known { value } => {
            let mut missing = Vec::new();
            if !value.secret_scanning {
                missing.push("secret scanning");
            }
            if !value.push_protection {
                missing.push("push protection");
            }
            result(
                definition,
                CheckStatus::Fail,
                PathBuf::new(),
                format!("disabled: {}", missing.join(", ")),
            )
        }
        GithubValue::Unavailable { reason } if facts.visibility == "private" => result(
            definition,
            CheckStatus::Skip,
            PathBuf::new(),
            format!("not available for this private repository: {reason}"),
        ),
        GithubValue::Unavailable { reason } => result(
            definition,
            CheckStatus::Error,
            PathBuf::new(),
            format!("could not read secret-scanning settings: {reason}"),
        ),
    });
}
