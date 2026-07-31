use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckStatus,
    GithubCheckContext, Severity, registry, result,
};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "allow-auto-merge",
    default_severity: Severity::Required,
    category: CheckCategory::GithubSafeguards,
    instructions: "Keep GitHub auto-merge equal to the effective github.allow_auto_merge policy; an unmanaged setting is left alone.",
    repository_runner: None,
    github_runner: Some(run),
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &GithubCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let Some(desired) = context.settings.allow_auto_merge else {
        results.push(result(
            definition,
            CheckStatus::Skip,
            PathBuf::new(),
            "auto-merge is not managed by the effective GitHub settings policy",
        ));
        return;
    };
    let current = context.facts.allow_auto_merge;
    let state = |value| if value { "enabled" } else { "disabled" };
    results.push(if current == desired {
        result(
            definition,
            CheckStatus::Pass,
            PathBuf::new(),
            format!("auto-merge is {}, as configured", state(desired)),
        )
    } else {
        result(
            definition,
            CheckStatus::Fail,
            PathBuf::new(),
            format!(
                "auto-merge is {} but effective policy requires it {}",
                state(current),
                state(desired)
            ),
        )
    });
}
