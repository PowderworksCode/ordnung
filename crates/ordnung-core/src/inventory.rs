use std::collections::BTreeSet;
use std::path::Component;
use std::path::{Path, PathBuf};

use entl_codebase::{
    PackageKind, PackageScript, SHELL_LANGUAGE, TaskKind, WorkspaceKind, language_conventions,
};
use entl_github::GithubInventory;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::profile::{
    EcosystemId, EcosystemProfile, LanguageId, LanguageProfile, ecosystem_profile, language_profile,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectCapability {
    CargoWorkspace,
    StaticSite,
    Tauri,
}

impl ProjectCapability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CargoWorkspace => "cargo-workspace",
            Self::StaticSite => "static-site",
            Self::Tauri => "tauri",
        }
    }
}

impl std::fmt::Display for ProjectCapability {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_str().fmt(formatter)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageInstance {
    pub root: PathBuf,
    pub manifest: PathBuf,
    pub ecosystem: EcosystemId,
    pub language: LanguageId,
    pub workspace_root: Option<PathBuf>,
    pub lockfile_owner: PathBuf,
    pub lockfile: Option<PathBuf>,
    #[serde(default)]
    pub scripts: Vec<PackageScript>,
    #[serde(default)]
    pub dependencies: Vec<entl_codebase::Dependency>,
    pub evidence: BTreeSet<PathBuf>,
}

impl PackageInstance {
    pub fn ecosystem_profile(&self) -> &'static EcosystemProfile {
        ecosystem_profile(self.ecosystem.as_str())
            .expect("discovered package references a registered ecosystem")
    }

    pub fn language_profile(&self) -> &'static LanguageProfile {
        language_profile(self.language.as_str())
            .expect("discovered package references a registered language")
    }

    pub fn is_workspace_member(&self) -> bool {
        self.workspace_root
            .as_ref()
            .is_some_and(|workspace| workspace != &self.root)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub root: PathBuf,
    pub languages: BTreeSet<LanguageId>,
    pub capabilities: BTreeSet<ProjectCapability>,
    pub ecosystems: BTreeSet<EcosystemId>,
    pub evidence: BTreeSet<PathBuf>,
}

impl Project {
    pub fn has_language(&self, language: &str) -> bool {
        self.languages
            .iter()
            .any(|candidate| candidate.as_str() == language)
    }

    pub fn has_language_profile(&self, language: &LanguageProfile) -> bool {
        self.has_language(language.id)
    }

    pub fn has_capability(&self, capability: ProjectCapability) -> bool {
        self.capabilities.contains(&capability)
    }

    pub fn uses_ecosystem(&self, ecosystem: &str) -> bool {
        self.ecosystems
            .iter()
            .any(|candidate| candidate.as_str() == ecosystem)
    }

    pub fn uses_ecosystem_profile(&self, ecosystem: &EcosystemProfile) -> bool {
        self.uses_ecosystem(ecosystem.id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryIssue {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct InventoryOptions {
    pub ignore: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    pub root: PathBuf,
    #[serde(default)]
    pub files: BTreeSet<PathBuf>,
    #[serde(default)]
    pub shell_scripts: BTreeSet<PathBuf>,
    pub projects: Vec<Project>,
    #[serde(default)]
    pub artifacts: Vec<entl_codebase::Artifact>,
    #[serde(default)]
    pub packages: Vec<PackageInstance>,
    #[serde(default)]
    pub github: GithubInventory,
    pub issues: Vec<InventoryIssue>,
}

impl Inventory {
    pub fn packages_at(&self, root: &Path) -> impl Iterator<Item = &PackageInstance> {
        self.packages
            .iter()
            .filter(move |package| package.root == root)
    }

    pub fn has_task(&self, language: &str, kind: TaskKind) -> bool {
        self.github.has_task(language, kind)
    }
}

pub fn inspect_repository(root: &Path, options: &InventoryOptions) -> Result<Inventory> {
    let codebase = entl_codebase::inspect(
        root,
        &entl_codebase::InventoryOptions {
            include_hidden: true,
            additional_ignores: options.ignore.clone(),
            ..entl_codebase::InventoryOptions::default()
        },
    )?;
    let github = entl_github::inspect(&codebase);
    let files = codebase
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect();
    let shell_scripts = codebase
        .files_with_language_profile(&SHELL_LANGUAGE)
        .map(|file| file.path.clone())
        .collect();
    let mut projects = codebase
        .projects
        .iter()
        .filter(|project| !is_hidden_path(&project.root))
        .map(|project| Project {
            root: project.root.clone(),
            languages: effective_languages(
                project
                    .languages
                    .iter()
                    .filter_map(|language| {
                        let profile = language_profile(language.language.as_str())?;
                        language_conventions(profile).map(|_| language.language.clone())
                    })
                    .collect(),
            ),
            capabilities: [
                ("cargo-workspace", ProjectCapability::CargoWorkspace),
                ("static-site", ProjectCapability::StaticSite),
                ("tauri", ProjectCapability::Tauri),
            ]
            .into_iter()
            .filter(|(facet, _)| project.has_facet(facet))
            .map(|(_, capability)| capability)
            .collect(),
            ecosystems: project.ecosystems.clone(),
            evidence: project.evidence.clone(),
        })
        .filter(|project| {
            !project.languages.is_empty()
                || !project.ecosystems.is_empty()
                || !project.capabilities.is_empty()
        })
        .collect::<Vec<_>>();

    let mut packages = codebase
        .packages
        .iter()
        .filter(|package| !is_hidden_path(&package.root))
        .filter_map(|package| package_instance(package, &codebase))
        .collect::<Vec<_>>();
    for workspace in &codebase.workspaces {
        if workspace.kind != WorkspaceKind::Cargo
            || is_hidden_path(&workspace.root)
            || codebase
                .packages
                .iter()
                .any(|package| package.kind == PackageKind::Cargo && package.root == workspace.root)
        {
            continue;
        }
        let profile = ecosystem_profile("cargo").expect("Cargo profile is registered");
        let language_profile = profile
            .implied_languages
            .first()
            .expect("Cargo profile implies Rust");
        let ecosystem = EcosystemId::from(profile);
        let language = LanguageId::from(*language_profile);
        let lockfile = profile
            .lockfiles
            .iter()
            .map(|lockfile| workspace.root.join(lockfile))
            .find(|path| codebase.has_file(path));
        let mut evidence = BTreeSet::from([workspace.manifest.clone()]);
        evidence.extend(lockfile.iter().cloned());
        let project = projects
            .iter_mut()
            .find(|project| project.root == workspace.root)
            .expect("workspace project is present in the Entl inventory");
        project.languages.insert(language.clone());
        project.ecosystems.insert(ecosystem.clone());
        project.evidence.extend(evidence.iter().cloned());
        packages.push(PackageInstance {
            root: workspace.root.clone(),
            manifest: workspace.manifest.clone(),
            ecosystem,
            language,
            workspace_root: Some(workspace.root.clone()),
            lockfile_owner: workspace.root.clone(),
            lockfile,
            scripts: Vec::new(),
            dependencies: Vec::new(),
            evidence,
        });
    }
    packages
        .sort_by(|left, right| (&left.root, &left.ecosystem).cmp(&(&right.root, &right.ecosystem)));

    let issues = codebase
        .diagnostics
        .iter()
        .map(|diagnostic| InventoryIssue {
            path: diagnostic.path.clone(),
            message: diagnostic.message.clone(),
        })
        .chain(github.diagnostics.iter().map(|diagnostic| InventoryIssue {
            path: diagnostic.path.clone(),
            message: diagnostic.message.clone(),
        }))
        .collect();
    Ok(Inventory {
        root: codebase.root,
        files,
        shell_scripts,
        projects,
        artifacts: codebase
            .artifacts
            .into_iter()
            .filter(|artifact| !is_hidden_path(&artifact.root))
            .collect(),
        packages,
        github,
        issues,
    })
}

fn package_instance(
    package: &entl_codebase::Package,
    codebase: &entl_codebase::CodebaseInventory,
) -> Option<PackageInstance> {
    let ecosystem = package.ecosystem.clone()?;
    let profile = ecosystem_profile(ecosystem.as_str())?;
    let language = profile
        .implied_languages
        .first()
        .map(|language| LanguageId::from(language.id))
        .or_else(|| {
            package
                .languages
                .first()
                .map(|language| language.language.clone())
        })?;
    let workspace_root = package.workspace.as_ref().and_then(|workspace| {
        codebase
            .workspace(workspace)
            .map(|workspace| workspace.root.clone())
    });
    Some(PackageInstance {
        root: package.root.clone(),
        manifest: package.manifest.clone(),
        ecosystem,
        language,
        workspace_root,
        lockfile_owner: package.lockfile_owner.clone(),
        lockfile: package.lockfile.clone(),
        scripts: package.scripts.clone(),
        dependencies: package.dependencies.clone(),
        evidence: package.evidence.clone(),
    })
}

fn is_hidden_path(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(component, Component::Normal(name) if name.to_string_lossy().starts_with('.'))
    })
}

fn effective_languages(languages: BTreeSet<LanguageId>) -> BTreeSet<LanguageId> {
    languages
        .iter()
        .filter(|language| {
            let Some(profile) = language_profile(language.as_str()) else {
                return true;
            };
            !languages.iter().any(|candidate| {
                language_profile(candidate.as_str())
                    .is_some_and(|candidate| candidate.supersedes(profile))
            })
        })
        .cloned()
        .collect()
}
