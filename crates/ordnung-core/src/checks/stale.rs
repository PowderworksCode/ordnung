use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckStatus,
    GithubCheckContext, Severity, registry, result,
};
use crate::github::GithubValue;

const IDLE_DAYS: u64 = 30;

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "stale",
    default_severity: Severity::Recommended,
    category: CheckCategory::MaintenanceAutomation,
    instructions: "Keep open pull requests active within 30 days, remove branches already merged into the default branch, and enable automatic branch deletion after merge.",
    repository_runner: None,
    github_runner: Some(run),
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &GithubCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    if context.archived {
        results.push(result(
            definition,
            CheckStatus::Skip,
            PathBuf::new(),
            "repository is archived",
        ));
        return;
    }
    let stale = match &context.stale {
        GithubValue::Known { value } => value,
        GithubValue::Unavailable { reason } => {
            results.push(result(
                definition,
                CheckStatus::Error,
                PathBuf::new(),
                format!("could not inspect pull requests and branches: {reason}"),
            ));
            return;
        }
    };
    let mut problems = Vec::new();
    let idle = stale
        .open_pull_requests
        .iter()
        .filter(|pull| pull.idle_days > IDLE_DAYS)
        .collect::<Vec<_>>();
    if !idle.is_empty() {
        problems.push(format!(
            "{} pull request(s) idle over {IDLE_DAYS} days: {}",
            idle.len(),
            idle.iter()
                .take(5)
                .map(|pull| format!("#{} ({}d)", pull.number, pull.idle_days))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !stale.merged_branches.is_empty() {
        problems.push(format!(
            "{} merged branch(es) remain: {}",
            stale.merged_branches.len(),
            stale
                .merged_branches
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !context.delete_branch_on_merge {
        problems.push("automatic branch deletion after merge is disabled".to_owned());
    }
    let mut notes = Vec::new();
    if stale.pull_requests_truncated {
        notes.push("only the first 100 open pull requests were inspected".to_owned());
    }
    if stale.non_default_branches > stale.examined_branches {
        notes.push(format!(
            "only {} of {} non-default branches were examined",
            stale.examined_branches, stale.non_default_branches
        ));
    }
    if stale.branches_truncated {
        notes.push("only the first 100 branches were listed".to_owned());
    }
    let mut message = if problems.is_empty() {
        format!(
            "{} open pull request(s) are active, no examined branches are already merged, and merged branches auto-delete",
            stale.open_pull_requests.len()
        )
    } else {
        problems.join("; ")
    };
    if !notes.is_empty() {
        message.push_str("; ");
        message.push_str(&notes.join("; "));
    }
    results.push(result(
        definition,
        if problems.is_empty() {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        PathBuf::new(),
        message,
    ));
}
