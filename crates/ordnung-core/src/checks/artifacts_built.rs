use std::path::PathBuf;

use entl::codebase::{Artifact, TAURI_ARTIFACT, artifact_profile};
use entl::github::{TaskInvocation, Workflow};

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "artifacts-built",
    default_severity: Severity::Recommended,
    category: CheckCategory::BuildToolchain,
    scope: CheckScope::Project,
    instructions: "Build every detected binary, site bundle, napi-rs addon, and Tauri application in GitHub Actions; run full Tauri builds on a scheduled workflow.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    if context.inventory.artifacts.is_empty() {
        results.push(result(
            definition,
            CheckStatus::Skip,
            PathBuf::new(),
            "no distributable artifacts detected",
        ));
        return;
    }

    for artifact in &context.inventory.artifacts {
        let profile = artifact_profile(artifact.profile.as_str())
            .expect("inventory artifacts reference registered profiles");
        let scheduled = profile.id == TAURI_ARTIFACT.id;
        let covered = context
            .inventory
            .github
            .workflows
            .iter()
            .filter(|workflow| !scheduled || workflow.triggers.contains("schedule"))
            .any(|workflow| workflow_covers(workflow, artifact));
        results.push(result(
            definition,
            if covered {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            artifact.root.clone(),
            match (covered, scheduled) {
                (true, true) => {
                    format!("{} is built by a scheduled workflow", profile.display_name)
                }
                (true, false) => format!("{} is built in GitHub Actions", profile.display_name),
                (false, true) => format!(
                    "{} is not built by a scheduled workflow",
                    profile.display_name
                ),
                (false, false) => {
                    format!("{} is not built by any workflow", profile.display_name)
                }
            },
        ));
    }
}

fn workflow_covers(workflow: &Workflow, artifact: &Artifact) -> bool {
    workflow
        .tasks
        .iter()
        .any(|task| task_covers(task, artifact))
}

fn task_covers(task: &TaskInvocation, artifact: &Artifact) -> bool {
    task.produces_artifact(artifact.profile.as_str()) && task.package_roots.contains(&artifact.root)
}
