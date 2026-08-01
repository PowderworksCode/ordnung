use std::path::{Path, PathBuf};

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

const CONVENTIONAL: [&str; 24] = [
    "readme",
    "changelog",
    "changes",
    "history",
    "license",
    "licence",
    "copying",
    "notice",
    "contributing",
    "code_of_conduct",
    "security",
    "support",
    "governance",
    "authors",
    "maintainers",
    "citation",
    "codeowners",
    "agents",
    "claude",
    "field_guide",
    "conduct",
    "funding",
    "issue_template",
    "pull_request_template",
];

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "stray-files",
    default_severity: Severity::Off,
    category: CheckCategory::RepositoryShape,
    scope: CheckScope::Repository,
    instructions: "Keep root Markdown and text files conventional or explicitly listed in stray_files.allow; keep working notes under stray_files.notes.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let config = context.stray_files;
    let mut strays = Vec::new();
    for path in &context.inventory.files {
        if !root_text(path)
            || config.allow.iter().any(|allowed| eq_path(path, allowed))
            || path.file_stem().is_some_and(|stem| {
                CONVENTIONAL.contains(&stem.to_string_lossy().to_ascii_lowercase().as_str())
            })
        {
            continue;
        }
        strays.push(path.display().to_string());
    }
    let mut problems = Vec::new();
    if !strays.is_empty() {
        problems.push(format!("stray root text files: {}", strays.join(", ")));
    }
    results.push(result(
        definition,
        if problems.is_empty() {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        PathBuf::new(),
        if problems.is_empty() {
            "no stray root Markdown or text files".into()
        } else {
            format!(
                "{}; working notes belong under {}",
                problems.join("; "),
                config.notes.display()
            )
        },
    ));
}

fn root_text(path: &Path) -> bool {
    path.parent()
        .is_some_and(|parent| parent.as_os_str().is_empty())
        && matches!(
            path.extension()
                .and_then(|value| value.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("md" | "txt")
        )
}
fn eq_path(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}
