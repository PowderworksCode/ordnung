use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckStatus,
    GithubCheckContext, Severity, registry, result,
};
use crate::github::GithubValue;

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "dependabot-automerge",
    default_severity: Severity::Recommended,
    category: CheckCategory::Dependencies,
    instructions: "When github.allow_auto_merge is explicitly enabled, use a Dependabot-only pull-request workflow that fetches update metadata, excludes major updates, and enables auto-merge behind required checks.",
    repository_runner: None,
    github_runner: Some(run),
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &GithubCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    if context.settings.allow_auto_merge != Some(true) {
        results.push(result(
            definition,
            CheckStatus::Skip,
            PathBuf::new(),
            "Dependabot auto-merge is not opted in through github.allow_auto_merge",
        ));
        return;
    }
    let mut problems = Vec::new();
    if !context.allow_auto_merge {
        problems.push("GitHub auto-merge is disabled".to_owned());
    }
    match &context.branch.required_checks {
        GithubValue::Known { value } if value.is_empty() => {
            problems.push("the default branch has no required status checks".to_owned())
        }
        GithubValue::Known { .. } => {}
        GithubValue::Unavailable { reason } => {
            problems.push(format!("required status checks are unavailable: {reason}"))
        }
    }
    let candidates = context
        .workflows
        .iter()
        .filter(|workflow| {
            workflow.state == "active"
                && (workflow.dependabot_automerge.dependabot_only
                    || workflow.dependabot_automerge.enables_auto_merge)
        })
        .collect::<Vec<_>>();
    let valid = candidates.iter().find(|workflow| {
        let facts = &workflow.dependabot_automerge;
        facts.pull_request_trigger
            && facts.dependabot_only
            && facts.fetches_metadata
            && facts.excludes_major_updates
            && facts.enables_auto_merge
    });
    if valid.is_none() {
        if let Some(workflow) = candidates.first() {
            let facts = &workflow.dependabot_automerge;
            let missing = [
                (!facts.pull_request_trigger).then_some("pull-request trigger"),
                (!facts.dependabot_only).then_some("Dependabot actor gate"),
                (!facts.fetches_metadata).then_some("dependabot/fetch-metadata"),
                (!facts.excludes_major_updates).then_some("major-update exclusion"),
                (!facts.enables_auto_merge).then_some("auto-merge command"),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
            problems.push(format!(
                "{} is missing {}",
                workflow.path,
                missing.join(", ")
            ));
        } else {
            problems.push("no active Dependabot auto-merge workflow was found".to_owned());
        }
    }
    let scope = valid
        .map(|workflow| PathBuf::from(&workflow.path))
        .or_else(|| {
            candidates
                .first()
                .map(|workflow| PathBuf::from(&workflow.path))
        })
        .unwrap_or_else(|| PathBuf::from(".github/workflows"));
    results.push(result(
        definition,
        if problems.is_empty() {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        scope,
        if problems.is_empty() {
            "Dependabot non-major updates enable auto-merge behind required checks".to_owned()
        } else {
            problems.join("; ")
        },
    ));
}
