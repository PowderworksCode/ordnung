use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use entl_codebase::{TaskKind, language_conventions};

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};
use crate::profile::language_profile;

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "ci-exists",
    default_severity: Severity::Required,
    category: CheckCategory::CiSafety,
    scope: CheckScope::Project,
    instructions: "Keep a push or pull-request workflow with test, lint, and format tasks for every detected language; exempt scratch project paths explicitly with ci_exists.ignore.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    if !context.inventory.github.has_workflows() {
        results.push(result(
            definition,
            CheckStatus::Fail,
            PathBuf::from(".github/workflows"),
            "no GitHub Actions workflows found",
        ));
        return;
    }
    if !context.inventory.github.diagnostics.is_empty() {
        for diagnostic in &context.inventory.github.diagnostics {
            results.push(result(
                definition,
                CheckStatus::Fail,
                diagnostic.path.clone(),
                diagnostic.message.clone(),
            ));
        }
        return;
    }
    if !context
        .inventory
        .github
        .workflows
        .iter()
        .any(entl_github::Workflow::runs_on_changes)
    {
        results.push(result(
            definition,
            CheckStatus::Fail,
            PathBuf::from(".github/workflows"),
            "no valid GitHub Actions workflow runs on push or pull_request",
        ));
        return;
    }

    let mut language_roots = BTreeMap::<String, BTreeSet<PathBuf>>::new();
    for project in &context.inventory.projects {
        // A project that declares no package.json is not owed JavaScript CI
        // tasks because a file of that language sits in it: a Rust crate with a
        // grammar.js or a build script has not taken on a JavaScript toolchain,
        // and asking it for one is advice nobody should follow.
        let declares_package =
            crate::inventory::declares_optionally_typed_package(context.inventory, project);
        for language in &project.languages {
            let optionally_typed = language_profile(language.as_str())
                .and_then(language_conventions)
                .is_some_and(|conventions| conventions.typecheck.is_some());
            if optionally_typed && !declares_package {
                continue;
            }
            language_roots
                .entry(language.as_str().to_owned())
                .or_default()
                .insert(project.root.clone());
        }
    }

    let mut exempt = Vec::new();
    for (language, roots) in &mut language_roots {
        roots.retain(|root| {
            if is_ignored(root, &context.ci_exists.ignore) {
                exempt.push(format!("{language} at {}", display_root(root)));
                false
            } else {
                true
            }
        });
    }
    let exemption = (!exempt.is_empty()).then(|| {
        exempt.sort();
        format!("; not graded: {} (ci_exists.ignore)", exempt.join(", "))
    });
    let languages = language_roots
        .into_iter()
        .filter(|(_, roots)| !roots.is_empty())
        .map(|(language, _)| language)
        .collect::<Vec<_>>();
    if languages.is_empty() {
        results.push(result(
            definition,
            CheckStatus::Pass,
            PathBuf::from(".github/workflows"),
            format!(
                "a GitHub Actions workflow runs on changes{}",
                exemption.as_deref().unwrap_or_default()
            ),
        ));
    }
    for language in languages {
        let missing = [TaskKind::Test, TaskKind::Lint, TaskKind::Format]
            .into_iter()
            .filter(|kind| !context.inventory.has_task(&language, *kind))
            .map(TaskKind::as_str)
            .collect::<Vec<_>>();
        results.push(result(
            definition,
            if missing.is_empty() {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            PathBuf::from(".github/workflows"),
            if missing.is_empty() {
                format!(
                    "{} CI runs test, lint, and format tasks on changes",
                    language
                )
            } else {
                format!(
                    "{} CI is missing {} tasks on push or pull_request",
                    language,
                    missing.join(", ")
                )
            } + exemption.as_deref().unwrap_or_default(),
        ));
    }
}

fn is_ignored(path: &Path, patterns: &[String]) -> bool {
    let path = display_root(path);
    patterns.iter().any(|pattern| {
        let pattern = pattern.trim_matches('/');
        [pattern.to_owned(), format!("{pattern}/**")]
            .into_iter()
            .filter_map(|pattern| globset::Glob::new(&pattern).ok())
            .any(|pattern| pattern.compile_matcher().is_match(&path))
    })
}

fn display_root(path: &Path) -> String {
    if path.as_os_str().is_empty() {
        ".".to_owned()
    } else {
        path.to_string_lossy().replace('\\', "/")
    }
}
