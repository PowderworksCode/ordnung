use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "project-inventory",
    default_severity: Severity::Required,
    category: CheckCategory::RepositoryShape,
    scope: CheckScope::Repository,
    instructions: "Keep supported project boundaries and manifests detectable by Ordnung.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    results.push(result(
        definition,
        CheckStatus::Pass,
        PathBuf::new(),
        if context.inventory.projects.is_empty() {
            "no supported language projects detected; repository checks still apply".into()
        } else {
            format!(
                "detected {} project boundary/boundaries",
                context.inventory.projects.len()
            )
        },
    ));

    for issue in &context.inventory.issues {
        results.push(result(
            definition,
            CheckStatus::Error,
            issue.path.clone(),
            issue.message.clone(),
        ));
    }
}
