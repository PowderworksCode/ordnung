use std::fs;
use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

const DOCUMENTATION_PATHS: [&str; 3] = ["CONTRIBUTING.md", "README.md", "README"];

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "conventional-commits",
    default_severity: Severity::Recommended,
    category: CheckCategory::RepositoryShape,
    scope: CheckScope::Repository,
    instructions: "Enforce Conventional Commits in a pull-request or push workflow with a recognized semantic-title action, commitlint, cocogitto, convco, or an explicit failing PR-title validator; mention Conventional Commits in the root README or CONTRIBUTING.md.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let enforcements = &context.inventory.github.conventional_commits.enforcements;
    let documented = match documented(context) {
        Ok(documented) => documented,
        Err((path, message)) => {
            results.push(result(definition, CheckStatus::Error, path, message));
            return;
        }
    };
    let mut problems = Vec::new();
    if enforcements.is_empty() {
        problems.push("no CI enforcement for PR titles or commit messages");
    }
    if documented.is_none() {
        problems.push("Conventional Commits are not mentioned in README or CONTRIBUTING.md");
    }

    if problems.is_empty() {
        let enforcement = &enforcements[0];
        results.push(result(
            definition,
            CheckStatus::Pass,
            enforcement.workflow.clone(),
            format!(
                "{} enforces {} in job `{}` and {} documents the convention",
                enforcement.enforcer,
                enforcement.target,
                enforcement.job,
                documented.expect("documentation is present").display()
            ),
        ));
    } else {
        results.push(result(
            definition,
            CheckStatus::Fail,
            PathBuf::from(".github/workflows"),
            problems.join("; "),
        ));
    }
}

fn documented(context: &RepositoryCheckContext<'_>) -> Result<Option<PathBuf>, (PathBuf, String)> {
    for path in DOCUMENTATION_PATHS.map(PathBuf::from) {
        if !context.inventory.files.contains(&path) {
            continue;
        }
        let text = fs::read_to_string(context.root.join(&path)).map_err(|error| {
            (
                path.clone(),
                format!("could not read {}: {error}", path.display()),
            )
        })?;
        if text.to_ascii_lowercase().contains("conventional commit") {
            return Ok(Some(path));
        }
    }
    Ok(None)
}
