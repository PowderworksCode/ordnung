use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use entl_codebase::TaskKind;
use entl_github::TaskInvocation;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "builds",
    default_severity: Severity::Required,
    category: CheckCategory::BuildToolchain,
    instructions: "Run every declared build, build:* or *:build package target on push or pull requests; Tauri projects also need a change-triggered compile check and a scheduled full build.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let mut found = false;
    let mut tauri_roots = BTreeSet::new();

    for package in &context.inventory.packages {
        let build_scripts = package
            .scripts
            .iter()
            .filter(|script| is_build_script(&script.name))
            .collect::<Vec<_>>();
        for script in build_scripts {
            found = true;
            let covered = context.inventory.github.workflows.iter().any(|workflow| {
                workflow.runs_on_changes()
                    && workflow.tasks.iter().any(|task| {
                        task.package_script.as_ref().is_some_and(|invocation| {
                            invocation.package_root == package.root
                                && invocation.name == script.name
                        })
                    })
            });
            results.push(result(
                definition,
                if covered {
                    CheckStatus::Pass
                } else {
                    CheckStatus::Fail
                },
                package.manifest.clone(),
                if covered {
                    format!("build target {:?} runs on changes", script.name)
                } else {
                    format!(
                        "build target {:?} does not run on push or pull_request",
                        script.name
                    )
                },
            ));
        }
    }

    for project in &context.inventory.projects {
        if project.has_capability(crate::inventory::ProjectCapability::Tauri) {
            tauri_roots.insert(project.root.clone());
        }
    }

    for root in tauri_roots {
        found = true;
        let scheduled = context.inventory.github.workflows.iter().any(|workflow| {
            workflow.triggers.contains("schedule")
                && workflow
                    .tasks
                    .iter()
                    .any(|task| is_tauri_build(task) && task_applies_to(task, &root))
        });
        let compile = context.inventory.github.workflows.iter().any(|workflow| {
            workflow.runs_on_changes()
                && workflow
                    .tasks
                    .iter()
                    .any(|task| is_tauri_compile(task, &root))
        });
        let status = if scheduled && compile {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        };
        let message = match (scheduled, compile) {
            (true, true) => {
                "Tauri has a change-triggered compile check and scheduled full build".to_owned()
            }
            (false, false) => {
                "Tauri has no change-triggered compile check or scheduled full build".to_owned()
            }
            (false, true) => "Tauri has no scheduled full build".to_owned(),
            (true, false) => "Tauri has no change-triggered compile check".to_owned(),
        };
        results.push(result(definition, status, root, message));
    }

    if !found {
        results.push(result(
            definition,
            CheckStatus::Skip,
            PathBuf::new(),
            "no package build targets or Tauri project detected",
        ));
    }
}

fn is_build_script(name: &str) -> bool {
    name == "build" || name.starts_with("build:") || name.ends_with(":build")
}

fn is_tauri_build(task: &TaskInvocation) -> bool {
    task.kind == TaskKind::Build && task.tool.as_str() == "tauri"
}

fn is_tauri_compile(task: &TaskInvocation, root: &Path) -> bool {
    if is_tauri_build(task) && task.arguments.iter().any(|argument| argument == "--debug") {
        return task_applies_to(task, root);
    }
    if task.tool.as_str() != "cargo" || !matches!(task.kind, TaskKind::Build | TaskKind::Lint) {
        return false;
    }
    let tauri_root = root.join("src-tauri");
    task.working_directory.starts_with(&tauri_root)
        || task.arguments.iter().any(|argument| {
            task.working_directory
                .join(argument)
                .starts_with(&tauri_root)
        })
}

fn task_applies_to(task: &TaskInvocation, root: &Path) -> bool {
    task.package_script
        .as_ref()
        .is_some_and(|script| script.package_root == root)
        || task.working_directory.starts_with(root)
}
