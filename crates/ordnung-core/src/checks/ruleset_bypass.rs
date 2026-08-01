use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckStatus,
    GithubCheckContext, Severity, registry, result,
};
use crate::github::GithubValue;

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "ruleset-bypass",
    default_severity: Severity::Recommended,
    category: CheckCategory::GithubSafeguards,
    instructions: "Give every active branch ruleset that gates merging at least one explicit bypass actor for emergency administration.",
    repository_runner: None,
    github_runner: Some(run),
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &GithubCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let rulesets = match &context.facts.rulesets {
        GithubValue::Known { value } => value,
        GithubValue::Unavailable { reason } => {
            results.push(result(
                definition,
                CheckStatus::Skip,
                PathBuf::new(),
                format!("rulesets are not visible to this token: {reason}"),
            ));
            return;
        }
    };
    let gating = rulesets
        .iter()
        .filter(|ruleset| ruleset.is_active_gating_branch_ruleset())
        .collect::<Vec<_>>();
    if gating.is_empty() {
        results.push(result(
            definition,
            CheckStatus::Skip,
            PathBuf::new(),
            "no active branch ruleset gates merging",
        ));
        return;
    }
    let missing = gating
        .iter()
        .filter(|ruleset| ruleset.bypass_actors.is_empty())
        .map(|ruleset| ruleset.name.as_str())
        .collect::<Vec<_>>();
    results.push(if missing.is_empty() {
        result(
            definition,
            CheckStatus::Pass,
            PathBuf::new(),
            format!(
                "all {} active gating branch ruleset(s) have bypass actors",
                gating.len()
            ),
        )
    } else {
        result(
            definition,
            CheckStatus::Fail,
            PathBuf::new(),
            format!(
                "active gating ruleset(s) have no bypass actor: {}",
                missing.join(", ")
            ),
        )
    });
}
