use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{CheckPolicy, GithubSettingsPolicy};
use crate::error::{Error, Result};
use crate::inventory::{Inventory, Project, ProjectCapability};
use crate::profile::{EcosystemId, LanguageId, ecosystem_profile, language_profile};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FleetConfig {
    pub name: String,
    #[serde(default, rename = "member")]
    pub members: Vec<FleetMember>,
    #[serde(default)]
    pub policy: FleetPolicy,
    #[serde(default, rename = "managed")]
    pub managed: Vec<ManagedEntry>,
}

impl FleetConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path).map_err(|source| Error::io(path, source))?;
        let config: Self = toml::from_str(&text).map_err(|error| Error::Parse {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
        let root = path.parent().unwrap_or_else(|| Path::new("."));
        config.validate(root)?;
        Ok(config)
    }

    pub fn validate(&self, fleet_root: &Path) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(Error::Config("fleet name cannot be empty".into()));
        }
        if self.members.is_empty() {
            return Err(Error::Config(
                "fleet must contain at least one [[member]]".into(),
            ));
        }

        let mut repos = BTreeSet::new();
        for member in &self.members {
            validate_repo_name(&member.repo)?;
            if !repos.insert(&member.repo) {
                return Err(Error::Config(format!(
                    "duplicate fleet member {:?}",
                    member.repo
                )));
            }
        }

        let mut names = BTreeSet::new();
        let mut ownership: Vec<&ManagedEntry> = Vec::new();
        for managed in &self.managed {
            if managed.name.trim().is_empty() {
                return Err(Error::Config("managed entry name cannot be empty".into()));
            }
            if !names.insert(&managed.name) {
                return Err(Error::Config(format!(
                    "duplicate managed entry {:?}",
                    managed.name
                )));
            }
            validate_relative(&managed.destination, "managed destination")?;
            for repo in &managed.only {
                validate_repo_name(repo)?;
                if !repos.contains(repo) {
                    return Err(Error::Config(format!(
                        "managed entry {:?} targets non-member repository {repo:?}",
                        managed.name
                    )));
                }
            }
            let unique_targets: BTreeSet<&String> = managed.only.iter().collect();
            if unique_targets.len() != managed.only.len() {
                return Err(Error::Config(format!(
                    "managed entry {:?} contains a duplicate repository target",
                    managed.name
                )));
            }
            if managed.relative_to == RelativeTo::Project && managed.when.is_none() {
                return Err(Error::Config(format!(
                    "project-relative managed entry {:?} requires a project selector",
                    managed.name
                )));
            }
            if let Some(selector) = &managed.when {
                selector.validate()?;
            }
            match managed.state {
                ManagedState::Present => {
                    let Some(source) = &managed.source else {
                        return Err(Error::Config(format!(
                            "managed entry {:?} requires source when state is present",
                            managed.name
                        )));
                    };
                    validate_relative(source, "managed source")?;
                    let source_path = fleet_root.join(source);
                    let metadata = fs::symlink_metadata(&source_path).map_err(|error| {
                        Error::Config(format!("{}: {error}", source_path.display()))
                    })?;
                    if metadata.file_type().is_symlink() {
                        return Err(Error::Config(format!(
                            "managed source {} cannot be a symlink",
                            source.display()
                        )));
                    }
                    if metadata.is_dir() {
                        directory_files(&source_path)?;
                    } else if !metadata.is_file() {
                        return Err(Error::Config(format!(
                            "managed source {} must be a file or directory",
                            source.display()
                        )));
                    }
                }
                ManagedState::Absent if managed.source.is_some() => {
                    return Err(Error::Config(format!(
                        "tombstone {:?} cannot declare a source",
                        managed.name
                    )));
                }
                ManagedState::Absent => {}
            }

            if ownership
                .iter()
                .any(|existing| managed_entries_overlap(existing, managed))
            {
                return Err(Error::Config(format!(
                    "managed destination {} overlaps existing ownership",
                    managed.destination.display()
                )));
            }
            ownership.push(managed);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FleetMember {
    pub repo: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FleetPolicy {
    #[serde(default)]
    pub checks: BTreeMap<String, CheckPolicy>,
    #[serde(default)]
    pub github: GithubSettingsPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedEntry {
    pub name: String,
    pub source: Option<PathBuf>,
    pub destination: PathBuf,
    #[serde(default)]
    pub state: ManagedState,
    #[serde(default)]
    pub relative_to: RelativeTo,
    pub when: Option<ProjectSelector>,
    #[serde(default)]
    pub only: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagedState {
    #[default]
    Present,
    Absent,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RelativeTo {
    #[default]
    Repo,
    Project,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSelector {
    pub language: Option<LanguageId>,
    pub capability: Option<ProjectCapability>,
    pub ecosystem: Option<EcosystemId>,
}

impl ProjectSelector {
    fn validate(&self) -> Result<()> {
        if self.language.is_none() && self.capability.is_none() && self.ecosystem.is_none() {
            return Err(Error::Config(
                "project selector must declare language, capability, or ecosystem".into(),
            ));
        }
        if let Some(language) = &self.language {
            if language_profile(language.as_str()).is_none() {
                return Err(Error::Config(format!(
                    "unknown language profile {language:?} in project selector"
                )));
            }
        }
        if let Some(ecosystem) = &self.ecosystem {
            if ecosystem_profile(ecosystem.as_str()).is_none() {
                return Err(Error::Config(format!(
                    "unknown ecosystem profile {ecosystem:?} in project selector"
                )));
            }
        }
        Ok(())
    }

    pub fn matches(&self, project: &Project) -> bool {
        self.language
            .as_ref()
            .is_none_or(|language| project.has_language(language.as_str()))
            && self
                .capability
                .is_none_or(|capability| project.has_capability(capability))
            && self
                .ecosystem
                .as_ref()
                .is_none_or(|ecosystem| project.uses_ecosystem(ecosystem.as_str()))
    }
}

impl std::fmt::Display for ProjectSelector {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut fields = Vec::new();
        if let Some(language) = &self.language {
            fields.push(format!("language={language}"));
        }
        if let Some(capability) = self.capability {
            fields.push(format!("capability={capability}"));
        }
        if let Some(ecosystem) = &self.ecosystem {
            fields.push(format!("ecosystem={ecosystem}"));
        }
        fields.join(", ").fmt(formatter)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChangeKind {
    Create,
    Update,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManagedChange {
    pub managed: String,
    pub path: PathBuf,
    pub kind: ChangeKind,
    #[serde(skip)]
    content: Option<Vec<u8>>,
}

impl ManagedChange {
    pub fn content(&self) -> Option<&[u8]> {
        self.content.as_deref()
    }
}

pub fn plan_managed_changes(
    fleet_root: &Path,
    member_repo: &str,
    member_root: &Path,
    inventory: &Inventory,
    entries: &[ManagedEntry],
) -> Result<Vec<ManagedChange>> {
    validate_repo_name(member_repo)?;
    let member_root = member_root
        .canonicalize()
        .map_err(|source| Error::io(member_root, source))?;
    let mut planned: BTreeMap<PathBuf, ManagedChange> = BTreeMap::new();

    for entry in entries {
        if !entry.only.is_empty() && !entry.only.iter().any(|repo| repo == member_repo) {
            continue;
        }
        for base in target_bases(&member_root, inventory, entry) {
            let destination = base.join(&entry.destination);
            ensure_inside(&member_root, &destination)?;
            ensure_no_symlink_path(&member_root, &destination)?;
            match entry.state {
                ManagedState::Absent => {
                    if destination.exists() {
                        insert_change(
                            &mut planned,
                            ManagedChange {
                                managed: entry.name.clone(),
                                path: relative_path(&member_root, &destination)?,
                                kind: ChangeKind::Delete,
                                content: None,
                            },
                        )?;
                    }
                }
                ManagedState::Present => {
                    let source = fleet_root.join(entry.source.as_ref().expect("validated"));
                    if source.is_dir() {
                        plan_directory(
                            &entry.name,
                            &source,
                            &destination,
                            &member_root,
                            &mut planned,
                        )?;
                    } else {
                        plan_file(
                            &entry.name,
                            &source,
                            &destination,
                            &member_root,
                            &mut planned,
                        )?;
                    }
                }
            }
        }
    }

    Ok(planned.into_values().collect())
}

pub fn apply_changes(member_root: &Path, changes: &[ManagedChange]) -> Result<()> {
    let member_root = member_root
        .canonicalize()
        .map_err(|source| Error::io(member_root, source))?;

    let mut deletes: Vec<&ManagedChange> = changes
        .iter()
        .filter(|change| change.kind == ChangeKind::Delete)
        .collect();
    deletes.sort_by_key(|change| std::cmp::Reverse(change.path.components().count()));
    for change in deletes {
        validate_relative(&change.path, "change path")?;
        let path = member_root.join(&change.path);
        ensure_no_symlink_path(&member_root, &path)?;
        if path.is_dir() {
            fs::remove_dir_all(&path).map_err(|source| Error::io(&path, source))?;
        } else if path.exists() {
            fs::remove_file(&path).map_err(|source| Error::io(&path, source))?;
        }
    }

    for change in changes
        .iter()
        .filter(|change| change.kind != ChangeKind::Delete)
    {
        validate_relative(&change.path, "change path")?;
        let path = member_root.join(&change.path);
        ensure_no_symlink_path(&member_root, &path)?;
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

fn target_bases(member_root: &Path, inventory: &Inventory, entry: &ManagedEntry) -> Vec<PathBuf> {
    match entry.relative_to {
        RelativeTo::Repo => vec![member_root.to_path_buf()],
        RelativeTo::Project => inventory
            .projects
            .iter()
            .filter(|project| {
                entry
                    .when
                    .as_ref()
                    .is_some_and(|selector| selector.matches(project))
            })
            .map(|project| member_root.join(&project.root))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    }
}

fn plan_file(
    managed: &str,
    source: &Path,
    destination: &Path,
    member_root: &Path,
    planned: &mut BTreeMap<PathBuf, ManagedChange>,
) -> Result<()> {
    let content = fs::read(source).map_err(|error| Error::io(source, error))?;
    let current = fs::read(destination).ok();
    if current.as_deref() == Some(content.as_slice()) {
        return Ok(());
    }
    insert_change(
        planned,
        ManagedChange {
            managed: managed.into(),
            path: relative_path(member_root, destination)?,
            kind: if destination.exists() {
                ChangeKind::Update
            } else {
                ChangeKind::Create
            },
            content: Some(content),
        },
    )
}

fn plan_directory(
    managed: &str,
    source: &Path,
    destination: &Path,
    member_root: &Path,
    planned: &mut BTreeMap<PathBuf, ManagedChange>,
) -> Result<()> {
    let desired = directory_files(source)?;
    let current = if destination.is_dir() {
        directory_files(destination)?
    } else {
        if destination.exists() {
            insert_change(
                planned,
                ManagedChange {
                    managed: managed.into(),
                    path: relative_path(member_root, destination)?,
                    kind: ChangeKind::Delete,
                    content: None,
                },
            )?;
        }
        BTreeMap::new()
    };

    for (relative, content) in &desired {
        let target = destination.join(relative);
        if current.get(relative) == Some(content) {
            continue;
        }
        insert_change(
            planned,
            ManagedChange {
                managed: managed.into(),
                path: relative_path(member_root, &target)?,
                kind: if target.exists() {
                    ChangeKind::Update
                } else {
                    ChangeKind::Create
                },
                content: Some(content.clone()),
            },
        )?;
    }

    for relative in current.keys().filter(|path| !desired.contains_key(*path)) {
        insert_change(
            planned,
            ManagedChange {
                managed: managed.into(),
                path: relative_path(member_root, &destination.join(relative))?,
                kind: ChangeKind::Delete,
                content: None,
            },
        )?;
    }
    Ok(())
}

fn directory_files(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut files = BTreeMap::new();
    collect_directory_files(root, root, &mut files)?;
    Ok(files)
}

fn collect_directory_files(
    root: &Path,
    directory: &Path,
    files: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> Result<()> {
    let entries = fs::read_dir(directory).map_err(|source| Error::io(directory, source))?;
    for entry in entries {
        let entry = entry.map_err(|source| Error::io(directory, source))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| Error::io(&path, source))?;
        if file_type.is_symlink() {
            return Err(Error::Config(format!(
                "managed directory contains symlink {}",
                path.display()
            )));
        }
        if file_type.is_dir() {
            collect_directory_files(root, &path, files)?;
        } else if file_type.is_file() {
            files.insert(
                path.strip_prefix(root)
                    .expect("directory descendant")
                    .to_path_buf(),
                fs::read(&path).map_err(|source| Error::io(&path, source))?,
            );
        }
    }
    Ok(())
}

fn insert_change(
    planned: &mut BTreeMap<PathBuf, ManagedChange>,
    change: ManagedChange,
) -> Result<()> {
    if let Some(existing) = planned.get(&change.path) {
        if existing != &change {
            return Err(Error::Config(format!(
                "managed entries propose conflicting changes for {}",
                change.path.display()
            )));
        }
        return Ok(());
    }
    planned.insert(change.path.clone(), change);
    Ok(())
}

fn validate_repo_name(repo: &str) -> Result<()> {
    let mut parts = repo.split('/');
    let valid = parts.next().is_some_and(|part| !part.is_empty())
        && parts.next().is_some_and(|part| !part.is_empty())
        && parts.next().is_none();
    if valid {
        Ok(())
    } else {
        Err(Error::Config(format!(
            "repository {repo:?} must be owner/name"
        )))
    }
}

fn validate_relative(path: &Path, label: &str) -> Result<()> {
    let has_normal_component = path
        .components()
        .any(|component| matches!(component, Component::Normal(_)));
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || !has_normal_component
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err(Error::Config(format!(
            "{label} {} must be a safe relative path",
            path.display()
        )));
    }
    Ok(())
}

fn managed_entries_overlap(left: &ManagedEntry, right: &ManagedEntry) -> bool {
    if left.relative_to != right.relative_to || !repository_targets_overlap(&left.only, &right.only)
    {
        return false;
    }

    left.destination == right.destination
        || left.destination.starts_with(&right.destination)
        || right.destination.starts_with(&left.destination)
}

fn repository_targets_overlap(left: &[String], right: &[String]) -> bool {
    left.is_empty()
        || right.is_empty()
        || left
            .iter()
            .any(|repo| right.iter().any(|candidate| candidate == repo))
}

fn ensure_inside(root: &Path, path: &Path) -> Result<()> {
    if path.starts_with(root) {
        Ok(())
    } else {
        Err(Error::Config(format!(
            "managed path {} escapes repository root",
            path.display()
        )))
    }
}

fn ensure_no_symlink_path(root: &Path, path: &Path) -> Result<()> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| Error::Config(format!("{} escapes repository root", path.display())))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::Config(format!(
                    "managed path crosses symlink {}",
                    current.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(Error::io(&current, error)),
        }
    }
    Ok(())
}

fn relative_path(root: &Path, path: &Path) -> Result<PathBuf> {
    path.strip_prefix(root)
        .map(Path::to_path_buf)
        .map_err(|_| Error::Config(format!("{} escapes repository root", path.display())))
}
