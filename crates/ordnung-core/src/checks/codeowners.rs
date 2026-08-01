use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "codeowners",
    default_severity: Severity::Recommended,
    category: CheckCategory::RepositoryShape,
    scope: CheckScope::Repository,
    instructions: "Keep a valid CODEOWNERS file in .github/CODEOWNERS, CODEOWNERS, or docs/CODEOWNERS, in GitHub precedence order, with at least one rule that assigns an @account, @organization/team, or email owner.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let inventory = &context.inventory.github.codeowners;
    if !inventory.diagnostics.is_empty() {
        results.extend(inventory.diagnostics.iter().map(|diagnostic| {
            result(
                definition,
                CheckStatus::Fail,
                diagnostic.path.clone(),
                diagnostic.message.clone(),
            )
        }));
        return;
    }
    let Some(configuration) = &inventory.configuration else {
        results.push(result(
            definition,
            CheckStatus::Fail,
            PathBuf::from(".github/CODEOWNERS"),
            "no CODEOWNERS file found in .github, the repository root, or docs",
        ));
        return;
    };

    let assignments = configuration
        .rules
        .iter()
        .filter(|rule| !rule.owners.is_empty())
        .count();
    let shadowed = inventory.files.len().saturating_sub(1);
    let message = if assignments == 0 {
        "CODEOWNERS has no rules that assign an owner".to_owned()
    } else if shadowed == 0 {
        format!("CODEOWNERS has {assignments} ownership assignment(s)")
    } else {
        format!(
            "CODEOWNERS has {assignments} ownership assignment(s); {shadowed} lower-priority file(s) are shadowed"
        )
    };
    results.push(result(
        definition,
        if assignments == 0 {
            CheckStatus::Fail
        } else {
            CheckStatus::Pass
        },
        configuration.path.clone(),
        message,
    ));
}
