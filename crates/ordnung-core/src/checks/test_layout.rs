use std::fs;
use std::path::{Path, PathBuf};

use entl_codebase::language_conventions;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};
use crate::config::{LanguageTestLayout, TestLayoutConfig};
use crate::inventory::Project;
use crate::profile::{LanguageProfile, language_profile};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "test-layout",
    default_severity: Severity::Off,
    category: CheckCategory::BuildToolchain,
    instructions: "Keep tests outside source files and mirror configured source paths under the test root.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    for project in &context.inventory.projects {
        for language in &project.languages {
            let Some(profile) = language_profile(language.as_str()) else {
                results.push(result(
                    definition,
                    CheckStatus::Error,
                    project.root.clone(),
                    format!("unknown language profile {language:?}"),
                ));
                continue;
            };
            if project.languages.iter().any(|candidate| {
                language_profile(candidate.as_str())
                    .is_some_and(|candidate_profile| candidate_profile.supersedes(profile))
            }) {
                continue;
            }
            let layout = context.test_layout.layout_for(profile);
            check_project(
                definition,
                context.root,
                project,
                profile,
                &layout,
                context.test_layout,
                results,
            );
        }
    }
}

fn check_project(
    definition: &'static CheckDefinition,
    root: &Path,
    project: &Project,
    language: &LanguageProfile,
    layout: &LanguageTestLayout,
    config: &TestLayoutConfig,
    results: &mut Vec<CheckResult>,
) {
    if let Some(invalid) = layout
        .source_roots
        .iter()
        .chain(std::iter::once(&layout.test_root))
        .find(|path| !safe_relative_path(path))
    {
        results.push(result(
            definition,
            CheckStatus::Error,
            project.root.clone(),
            format!(
                "test layout path {} is not safely relative",
                invalid.display()
            ),
        ));
        return;
    }
    if config.require_mirror && layout.test_suffixes.is_empty() {
        results.push(result(
            definition,
            CheckStatus::Error,
            project.root.clone(),
            "test_suffixes cannot be empty when require_mirror is enabled",
        ));
        return;
    }

    let project_root = root.join(&project.root);
    let mut source_files = Vec::new();
    for source_root in &layout.source_roots {
        let absolute = project_root.join(source_root);
        if !absolute.is_dir() {
            continue;
        }
        if let Err(error) = collect_source_files(&absolute, language, &mut source_files) {
            results.push(result(
                definition,
                CheckStatus::Error,
                project.root.join(source_root),
                format!("could not read source tree: {error}"),
            ));
            return;
        }
    }
    source_files.sort();
    source_files.dedup();
    source_files.retain(|file| {
        let relative = file.strip_prefix(&project_root).unwrap_or(file);
        !ignored_test_path(relative, &config.ignore)
    });

    if source_files.is_empty() {
        results.push(result(
            definition,
            CheckStatus::Skip,
            project.root.clone(),
            format!(
                "no {} files found in configured source roots",
                language.display_name
            ),
        ));
        return;
    }

    let mut violations = 0;
    for source_file in &source_files {
        let scope = source_file
            .strip_prefix(root)
            .unwrap_or(source_file)
            .to_path_buf();
        if config.scan_inline {
            match fs::read_to_string(source_file) {
                Ok(content) => {
                    if let Some(indicator) = language_conventions(language)
                        .and_then(|conventions| conventions.inline_test_indicator(&content))
                    {
                        violations += 1;
                        results.push(result(
                            definition,
                            CheckStatus::Fail,
                            scope.clone(),
                            format!(
                                "inline test indicator {indicator:?} belongs under the external test root"
                            ),
                        ));
                    }
                }
                Err(error) => {
                    violations += 1;
                    results.push(result(
                        definition,
                        CheckStatus::Error,
                        scope.clone(),
                        format!("could not read source file: {error}"),
                    ));
                }
            }
        }

        if config.require_mirror && !has_mirrored_test(&project_root, source_file, language, layout)
        {
            violations += 1;
            results.push(result(
                definition,
                CheckStatus::Fail,
                scope,
                format!(
                    "no mirrored test file under {}",
                    project.root.join(&layout.test_root).display()
                ),
            ));
        }
    }

    if violations == 0 {
        results.push(result(
            definition,
            CheckStatus::Pass,
            project.root.clone(),
            format!(
                "{} {} source file(s) use external mirrored tests",
                source_files.len(),
                language.display_name
            ),
        ));
    }
}

fn collect_source_files(
    directory: &Path,
    language: &LanguageProfile,
    files: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            collect_source_files(&path, language, files)?;
        } else if file_type.is_file() && language.accepts_source(&path) {
            files.push(path);
        }
    }
    Ok(())
}

fn has_mirrored_test(
    project_root: &Path,
    source_file: &Path,
    language: &LanguageProfile,
    layout: &LanguageTestLayout,
) -> bool {
    let Some(relative) = layout.source_roots.iter().find_map(|source_root| {
        source_file
            .strip_prefix(project_root.join(source_root))
            .ok()
    }) else {
        return false;
    };
    let Some(stem) = relative.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    let parent = relative.parent().unwrap_or_else(|| Path::new(""));
    layout.test_suffixes.iter().any(|suffix| {
        language.source_extensions.iter().any(|extension| {
            project_root
                .join(&layout.test_root)
                .join(parent)
                .join(format!("{stem}{suffix}.{extension}"))
                .is_file()
        })
    })
}

fn safe_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path.components().all(|component| {
            matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
}

fn ignored_test_path(path: &Path, patterns: &[String]) -> bool {
    patterns.iter().any(|pattern| {
        let prefix = pattern
            .strip_suffix("/**")
            .or_else(|| pattern.strip_suffix('/'))
            .unwrap_or(pattern);
        let prefix = Path::new(prefix);
        path == prefix || path.starts_with(prefix)
    })
}
