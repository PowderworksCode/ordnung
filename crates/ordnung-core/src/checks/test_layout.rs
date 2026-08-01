//! Shared resolution for the two test-layout checks.
//!
//! Whether tests may live inside source files and whether every source file needs
//! a mirrored test file are independent positions, so they are separate checks
//! rather than two booleans on one. A severity travels through fleet policy; a
//! boolean on a repository-local config does not, so splitting them is what makes
//! each position configurable for a whole fleet.

use std::fs;
use std::path::{Path, PathBuf};

use crate::check::{CheckDefinition, CheckResult, CheckStatus, RepositoryCheckContext, result};
use crate::config::LanguageTestLayout;
use crate::inventory::Project;
use crate::profile::{LanguageProfile, language_profile};

/// One project/language pair with its configured layout and eligible source files.
pub(super) struct ProjectLayout<'a> {
    pub(super) project: &'a Project,
    pub(super) language: &'static LanguageProfile,
    pub(super) layout: LanguageTestLayout,
    pub(super) project_root: PathBuf,
    pub(super) source_files: Vec<PathBuf>,
}

impl ProjectLayout<'_> {
    /// The repository-relative path used to report a finding against a source file.
    pub(super) fn scope(&self, root: &Path, source_file: &Path) -> PathBuf {
        source_file
            .strip_prefix(root)
            .unwrap_or(source_file)
            .to_path_buf()
    }
}

/// Resolves every project/language pair, pushing the `Error` and `Skip` results
/// both checks share. `require_suffixes` is set by the mirror check, which cannot
/// work without them.
pub(super) fn resolve<'a>(
    definition: &'static CheckDefinition,
    context: &'a RepositoryCheckContext<'_>,
    require_suffixes: bool,
    results: &mut Vec<CheckResult>,
) -> Vec<ProjectLayout<'a>> {
    let mut resolved = Vec::new();
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
                continue;
            }
            if require_suffixes && layout.test_suffixes.is_empty() {
                results.push(result(
                    definition,
                    CheckStatus::Error,
                    project.root.clone(),
                    "test_suffixes cannot be empty",
                ));
                continue;
            }

            let project_root = context.root.join(&project.root);
            let mut source_files = Vec::new();
            let mut unreadable = false;
            for source_root in &layout.source_roots {
                let absolute = project_root.join(source_root);
                if !absolute.is_dir() {
                    continue;
                }
                if let Err(error) = collect_source_files(&absolute, profile, &mut source_files) {
                    results.push(result(
                        definition,
                        CheckStatus::Error,
                        project.root.join(source_root),
                        format!("could not read source tree: {error}"),
                    ));
                    unreadable = true;
                    break;
                }
            }
            if unreadable {
                continue;
            }
            source_files.sort();
            source_files.dedup();
            source_files.retain(|file| {
                let relative = file.strip_prefix(&project_root).unwrap_or(file);
                !ignored_test_path(relative, &context.test_layout.ignore)
            });

            if source_files.is_empty() {
                results.push(result(
                    definition,
                    CheckStatus::Skip,
                    project.root.clone(),
                    format!(
                        "no {} files found in configured source roots",
                        profile.display_name
                    ),
                ));
                continue;
            }

            resolved.push(ProjectLayout {
                project,
                language: profile,
                layout,
                project_root,
                source_files,
            });
        }
    }
    resolved
}

pub(super) fn has_mirrored_test(
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
