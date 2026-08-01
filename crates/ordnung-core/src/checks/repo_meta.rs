use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    GithubCheckContext, Severity, registry, result,
};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "repo-meta",
    default_severity: Severity::Recommended,
    category: CheckCategory::RepositoryShape,
    scope: CheckScope::Repository,
    instructions: "Keep repository description and issue tracking configured.",
    repository_runner: None,
    github_runner: Some(run),
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    facts: &GithubCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let mut problems = Vec::new();
    if facts
        .description
        .as_deref()
        .is_none_or(|description| description.trim().is_empty())
    {
        problems.push("description is empty");
    }
    if !facts.has_issues {
        problems.push("issues are disabled");
    }

    results.push(result(
        definition,
        if problems.is_empty() {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        PathBuf::new(),
        if problems.is_empty() {
            "repository description and issue tracking are configured".into()
        } else {
            problems.join(", ")
        },
    ));
}
