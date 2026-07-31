use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckStatus,
    GithubCheckContext, Severity, registry, result,
};
use crate::github::GithubWorkflowFacts;

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "ci-green",
    default_severity: Severity::Required,
    category: CheckCategory::CiSafety,
    instructions: "Keep latest default-branch runs green for active repository-owned workflows.",
    repository_runner: None,
    github_runner: Some(run),
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    facts: &GithubCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let hosting = std::env::var("GITHUB_WORKFLOW").ok();
    let family = ["housekeeping", "housecaptain"];
    let workflows: Vec<&GithubWorkflowFacts> = facts
        .workflows
        .iter()
        .filter(|workflow| {
            workflow.state == "active"
                && !workflow.is_github_managed()
                && !family.contains(&workflow.name.as_str())
                && hosting.as_deref() != Some(workflow.name.as_str())
        })
        .collect();
    let excluded = facts
        .workflows
        .iter()
        .filter(|workflow| {
            family.contains(&workflow.name.as_str())
                || hosting.as_deref() == Some(workflow.name.as_str())
        })
        .map(|workflow| workflow.name.as_str())
        .collect::<Vec<_>>();
    if workflows.is_empty() {
        results.push(result(
            definition,
            CheckStatus::Skip,
            PathBuf::from(".github/workflows"),
            if excluded.is_empty() {
                "no active repository-owned workflows found".to_owned()
            } else {
                format!(
                    "no workflows to grade beyond self-audit or hosting workflows: {}",
                    excluded.join(", ")
                )
            },
        ));
        return;
    }

    let mut red = Vec::new();
    let mut green = Vec::new();
    let mut quiet = Vec::new();
    for workflow in &workflows {
        match &workflow.latest_run {
            Some(run) if run.conclusion.as_deref() == Some("success") => {
                green.push(workflow.name.as_str())
            }
            Some(run) => red.push(format!(
                "{} ({}: {})",
                workflow.name,
                run.conclusion.as_deref().unwrap_or("no conclusion"),
                run.html_url
            )),
            None => quiet.push(workflow.name.as_str()),
        }
    }

    let mut notes = Vec::new();
    if !quiet.is_empty() {
        notes.push(format!(
            "no completed {} runs yet for: {}",
            facts.default_branch,
            quiet.join(", ")
        ));
    }
    if !excluded.is_empty() {
        notes.push(format!(
            "not grading self-audit or hosting workflows: {}",
            excluded.join(", ")
        ));
    }
    let notes = notes.join("; ");

    results.push(result(
        definition,
        if !red.is_empty() {
            CheckStatus::Fail
        } else if green.is_empty() {
            CheckStatus::Skip
        } else {
            CheckStatus::Pass
        },
        PathBuf::from(".github/workflows"),
        if !red.is_empty() {
            with_notes(
                format!("red on {}: {}", facts.default_branch, red.join("; ")),
                &notes,
            )
        } else if green.is_empty() {
            with_notes(
                format!(
                    "no workflow has a completed {} run yet",
                    facts.default_branch
                ),
                &notes,
            )
        } else {
            with_notes(
                format!(
                    "latest {} runs green: {}",
                    facts.default_branch,
                    green.join(", ")
                ),
                &notes,
            )
        },
    ));
}

fn with_notes(message: String, notes: &str) -> String {
    if notes.is_empty() {
        message
    } else {
        format!("{message}; {notes}")
    }
}
