use std::path::PathBuf;

use entl_github::ActionPinStatus;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

use super::examples;

/// Pinning Actions to a commit revision is externally endorsed practice rather
/// than a house preference: a mutable tag lets an upstream owner change what runs
/// in this repository's CI. Package version pinning is a separate, softer question
/// and lives in `pinned-dependencies`.
pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "pinned-actions",
    default_severity: Severity::Required,
    category: CheckCategory::Dependencies,
    scope: CheckScope::Repository,
    instructions: "Reference third-party GitHub Actions by commit SHA; local actions are exempt and first-party release channels are allowed.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let mut checked = 0usize;
    let mut channels = 0usize;
    let mut violations = Vec::new();
    for reference in &context.inventory.github.action_references {
        match reference.pin_status {
            ActionPinStatus::Pinned => checked += 1,
            ActionPinStatus::Channel => {
                checked += 1;
                channels += 1;
            }
            ActionPinStatus::Local => {}
            ActionPinStatus::Floating => {
                checked += 1;
                violations.push(format!(
                    "{}:{} {}@{}",
                    reference.workflow.display(),
                    reference.job,
                    reference.action,
                    reference.reference.as_deref().unwrap_or("<missing>")
                ));
            }
        }
    }

    if checked == 0 {
        results.push(result(
            definition,
            CheckStatus::Skip,
            PathBuf::new(),
            "no GitHub Action references",
        ));
        return;
    }
    if violations.is_empty() {
        let detail = if channels > 0 {
            format!(
                "all {checked} Action reference(s) are pinned, including {channels} allowed release channel(s)"
            )
        } else {
            format!("all {checked} Action reference(s) are pinned")
        };
        results.push(result(
            definition,
            CheckStatus::Pass,
            PathBuf::new(),
            detail,
        ));
        return;
    }
    results.push(result(
        definition,
        CheckStatus::Fail,
        PathBuf::new(),
        format!(
            "{} floating Action reference(s): {}",
            violations.len(),
            examples(&violations)
        ),
    ));
}
