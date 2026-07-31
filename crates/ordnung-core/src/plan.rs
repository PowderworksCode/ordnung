use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::check::{CheckResult, CheckStatus, Report, Severity};
use crate::error::{Error, Result};
use crate::fleet::{ChangeKind, ManagedChange};
use crate::github::GithubSettingChange;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileOperation {
    Create,
    Update,
    Delete,
}

impl From<ChangeKind> for FileOperation {
    fn from(value: ChangeKind) -> Self {
        match value {
            ChangeKind::Create => Self::Create,
            ChangeKind::Update => Self::Update,
            ChangeKind::Delete => Self::Delete,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum FileChangeSource {
    Check { check: String },
    Managed { entry: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedFileChange {
    pub path: PathBuf,
    pub operation: FileOperation,
    pub sources: Vec<FileChangeSource>,
    #[serde(skip)]
    content: Option<Vec<u8>>,
}

impl PlannedFileChange {
    pub fn content(&self) -> Option<&[u8]> {
        self.content.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemediationPlan {
    pub repository: String,
    pub findings: Vec<CheckResult>,
    pub file_changes: Vec<PlannedFileChange>,
    pub github_setting_changes: Vec<GithubSettingChange>,
}

impl RemediationPlan {
    pub fn has_changes(&self) -> bool {
        !self.file_changes.is_empty() || !self.github_setting_changes.is_empty()
    }

    pub fn is_clean(&self) -> bool {
        !self.findings.iter().any(|finding| {
            finding.severity == Severity::Required
                && matches!(finding.status, CheckStatus::Fail | CheckStatus::Error)
        }) && !self.has_changes()
    }
}

pub fn build_remediation_plan(
    repository: impl Into<String>,
    reports: &[Report],
    managed_changes: &[ManagedChange],
    github_setting_changes: Vec<GithubSettingChange>,
) -> Result<RemediationPlan> {
    let mut findings = reports
        .iter()
        .flat_map(|report| report.results.iter())
        .filter(|result| {
            result.severity != Severity::Off
                && matches!(result.status, CheckStatus::Fail | CheckStatus::Error)
        })
        .cloned()
        .collect::<Vec<_>>();
    findings.sort_by(|left, right| {
        left.check
            .cmp(&right.check)
            .then(left.scope.cmp(&right.scope))
            .then(left.message.cmp(&right.message))
    });

    let mut files = BTreeMap::<PathBuf, PlannedFileChange>::new();
    for result in &findings {
        if result.status != CheckStatus::Fail || result.severity == Severity::Off {
            continue;
        }
        let Some(remediation) = &result.remediation else {
            continue;
        };
        insert_file_change(
            &mut files,
            PlannedFileChange {
                path: remediation.path.clone(),
                operation: remediation.operation,
                sources: vec![FileChangeSource::Check {
                    check: result.check.clone(),
                }],
                content: remediation.content().map(<[u8]>::to_vec),
            },
        )?;
    }
    for change in managed_changes {
        insert_file_change(
            &mut files,
            PlannedFileChange {
                path: change.path.clone(),
                operation: change.kind.into(),
                sources: vec![FileChangeSource::Managed {
                    entry: change.managed.clone(),
                }],
                content: change.content().map(<[u8]>::to_vec),
            },
        )?;
    }

    Ok(RemediationPlan {
        repository: repository.into(),
        findings,
        file_changes: files.into_values().collect(),
        github_setting_changes,
    })
}

pub fn apply_file_changes(repository_root: &Path, changes: &[PlannedFileChange]) -> Result<()> {
    let repository_root = repository_root
        .canonicalize()
        .map_err(|source| Error::io(repository_root, source))?;

    let mut deletes = changes
        .iter()
        .filter(|change| change.operation == FileOperation::Delete)
        .collect::<Vec<_>>();
    deletes.sort_by_key(|change| std::cmp::Reverse(change.path.components().count()));
    for change in deletes {
        let path = checked_path(&repository_root, &change.path)?;
        if path.is_dir() {
            fs::remove_dir_all(&path).map_err(|source| Error::io(&path, source))?;
        } else if path.exists() {
            fs::remove_file(&path).map_err(|source| Error::io(&path, source))?;
        }
    }

    for change in changes
        .iter()
        .filter(|change| change.operation != FileOperation::Delete)
    {
        let path = checked_path(&repository_root, &change.path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| Error::io(parent, source))?;
        }
        if path.is_dir() {
            fs::remove_dir_all(&path).map_err(|source| Error::io(&path, source))?;
        }
        fs::write(
            &path,
            change.content.as_deref().expect("write change has content"),
        )
        .map_err(|source| Error::io(&path, source))?;
    }
    Ok(())
}

fn insert_file_change(
    planned: &mut BTreeMap<PathBuf, PlannedFileChange>,
    mut change: PlannedFileChange,
) -> Result<()> {
    validate_relative(&change.path)?;
    match change.operation {
        FileOperation::Create | FileOperation::Update if change.content.is_none() => {
            return Err(Error::Config(format!(
                "write change for {} has no content",
                change.path.display()
            )));
        }
        FileOperation::Delete if change.content.is_some() => {
            return Err(Error::Config(format!(
                "delete change for {} unexpectedly has content",
                change.path.display()
            )));
        }
        _ => {}
    }

    if let Some(existing) = planned.get_mut(&change.path) {
        if existing.operation != change.operation || existing.content != change.content {
            return Err(Error::Config(format!(
                "remediations propose conflicting changes for {}",
                change.path.display()
            )));
        }
        existing.sources.append(&mut change.sources);
        existing.sources.sort();
        existing.sources.dedup();
        return Ok(());
    }
    if let Some(existing) = planned
        .keys()
        .find(|path| path.starts_with(&change.path) || change.path.starts_with(path))
    {
        return Err(Error::Config(format!(
            "remediations propose overlapping changes for {} and {}",
            existing.display(),
            change.path.display()
        )));
    }
    planned.insert(change.path.clone(), change);
    Ok(())
}

fn checked_path(root: &Path, relative: &Path) -> Result<PathBuf> {
    validate_relative(relative)?;
    let path = root.join(relative);
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            continue;
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::Config(format!(
                    "remediation path crosses symlink {}",
                    current.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(Error::io(&current, error)),
        }
    }
    Ok(path)
}

fn validate_relative(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err(Error::Config(format!(
            "remediation path {} must be a safe repository-relative path",
            path.display()
        )));
    }
    Ok(())
}
