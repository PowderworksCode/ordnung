use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::config::{
    CiExistsConfig, CodegenConfig, DependencyRequirement, GithubSettings, RepoConfig,
    ScriptsConfig, StrayFilesConfig, TestLayoutConfig,
};
use crate::github::GithubRepositoryFacts;
use crate::inventory::Inventory;
use crate::plan::FileOperation;

pub use profile_inventory as registry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    Required,
    Recommended,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckStatus {
    Pass,
    Fail,
    Skip,
    Error,
}

/// Whether a check's findings describe the repository as a whole or one detected
/// project within it.
///
/// A repository-scoped check has one verdict per repository: there is one README,
/// one default branch, one Dependabot configuration. A project-scoped check reports
/// per project root, so a monorepository receives one finding per directory. Policy
/// that selects directories can only apply to the latter, and declaring the scope is
/// what lets that be validated instead of silently misfiring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckScope {
    Repository,
    Project,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CheckCategory {
    RepositoryShape,
    Documentation,
    GithubSafeguards,
    CiSafety,
    BuildToolchain,
    Dependencies,
    MaintenanceAutomation,
}

impl CheckCategory {
    pub const ALL: [Self; 7] = [
        Self::RepositoryShape,
        Self::Documentation,
        Self::GithubSafeguards,
        Self::CiSafety,
        Self::BuildToolchain,
        Self::Dependencies,
        Self::MaintenanceAutomation,
    ];

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::RepositoryShape => "Repository shape",
            Self::Documentation => "Documentation and text",
            Self::GithubSafeguards => "GitHub safeguards",
            Self::CiSafety => "CI safety",
            Self::BuildToolchain => "Build and toolchain",
            Self::Dependencies => "Dependencies",
            Self::MaintenanceAutomation => "Maintenance automation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckResult {
    pub check: String,
    pub status: CheckStatus,
    pub severity: Severity,
    pub scope: PathBuf,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<CheckRemediation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckRemediation {
    pub summary: String,
    pub operation: FileOperation,
    pub path: PathBuf,
    #[serde(skip)]
    content: Option<Vec<u8>>,
}

impl CheckRemediation {
    pub fn create(
        path: impl Into<PathBuf>,
        content: impl Into<Vec<u8>>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            summary: summary.into(),
            operation: FileOperation::Create,
            path: path.into(),
            content: Some(content.into()),
        }
    }

    pub fn update(
        path: impl Into<PathBuf>,
        content: impl Into<Vec<u8>>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            summary: summary.into(),
            operation: FileOperation::Update,
            path: path.into(),
            content: Some(content.into()),
        }
    }

    pub fn delete(path: impl Into<PathBuf>, summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            operation: FileOperation::Delete,
            path: path.into(),
            content: None,
        }
    }

    pub fn content(&self) -> Option<&[u8]> {
        self.content.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub repository: PathBuf,
    pub results: Vec<CheckResult>,
}

impl Report {
    pub fn is_clean(&self) -> bool {
        !self.results.iter().any(|result| {
            result.severity == Severity::Required
                && matches!(result.status, CheckStatus::Fail | CheckStatus::Error)
        })
    }

    pub fn apply_policy(&mut self, policy: &BTreeMap<String, Severity>) {
        for result in &mut self.results {
            if let Some(severity) = policy.get(&result.check) {
                result.severity = *severity;
            }
        }
    }
}

pub struct RepositoryCheckContext<'a> {
    pub root: &'a Path,
    pub inventory: &'a Inventory,
    /// Dependency requirements in force, already merged across policy layers.
    pub dependencies: &'a [DependencyRequirement],
    pub ci_exists: &'a CiExistsConfig,
    pub codegen: &'a [CodegenConfig],
    pub scripts: &'a ScriptsConfig,
    pub stray_files: &'a StrayFilesConfig,
    pub test_layout: &'a TestLayoutConfig,
}

pub type RepositoryCheckRunner =
    for<'a> fn(&'static CheckDefinition, &RepositoryCheckContext<'a>, &mut Vec<CheckResult>);
pub struct GithubCheckContext<'a> {
    pub facts: &'a GithubRepositoryFacts,
    pub settings: &'a GithubSettings,
}

impl std::ops::Deref for GithubCheckContext<'_> {
    type Target = GithubRepositoryFacts;

    fn deref(&self) -> &Self::Target {
        self.facts
    }
}

pub type GithubCheckRunner =
    for<'a> fn(&'static CheckDefinition, &GithubCheckContext<'a>, &mut Vec<CheckResult>);

pub struct CheckDefinition {
    pub id: &'static str,
    pub default_severity: Severity,
    pub category: CheckCategory,
    pub scope: CheckScope,
    pub instructions: &'static str,
    pub repository_runner: Option<RepositoryCheckRunner>,
    pub github_runner: Option<GithubCheckRunner>,
}

pub struct CheckRegistration(pub &'static CheckDefinition);

registry::collect!(CheckRegistration);

static CHECK_DEFINITIONS: LazyLock<Vec<&'static CheckDefinition>> = LazyLock::new(|| {
    let mut definitions = registry::iter::<CheckRegistration>
        .into_iter()
        .map(|registration| registration.0)
        .collect::<Vec<_>>();
    definitions.sort_by_key(|definition| definition.id);
    for pair in definitions.windows(2) {
        assert_ne!(pair[0].id, pair[1].id, "duplicate check ID {}", pair[0].id);
    }
    definitions
});

pub fn check_definitions() -> &'static [&'static CheckDefinition] {
    &CHECK_DEFINITIONS
}

pub fn check_definition(id: &str) -> Option<&'static CheckDefinition> {
    check_definitions()
        .binary_search_by_key(&id, |definition| definition.id)
        .ok()
        .map(|index| check_definitions()[index])
}

pub fn check_ids() -> Vec<&'static str> {
    check_definitions()
        .iter()
        .map(|definition| definition.id)
        .collect()
}

pub fn default_policy() -> BTreeMap<String, Severity> {
    check_definitions()
        .iter()
        .map(|definition| (definition.id.to_owned(), definition.default_severity))
        .collect()
}

/// One `skip` per applicable check, explaining that the repository is archived.
fn archived_report(
    repository: PathBuf,
    applies: impl Fn(&'static CheckDefinition) -> bool,
) -> Report {
    Report {
        repository,
        results: check_definitions()
            .iter()
            .filter(|definition| applies(definition))
            .map(|definition| {
                result(
                    definition,
                    CheckStatus::Skip,
                    PathBuf::new(),
                    "repository is archived and cannot be changed",
                )
            })
            .collect(),
    }
}

pub fn run_repository_checks(root: &Path, inventory: &Inventory) -> Report {
    run_repository_checks_with_repo_config(root, inventory, &RepoConfig::default())
}

pub fn run_repository_checks_with_config(
    root: &Path,
    inventory: &Inventory,
    test_layout: &TestLayoutConfig,
) -> Report {
    run_repository_checks_with_repo_config(
        root,
        inventory,
        &RepoConfig {
            test_layout: test_layout.clone(),
            ..RepoConfig::default()
        },
    )
}

pub fn run_repository_checks_with_repo_config(
    root: &Path,
    inventory: &Inventory,
    config: &RepoConfig,
) -> Report {
    run_repository_checks_with_requirements(root, inventory, config, &config.dependencies)
}

/// Runs repository checks with dependency requirements supplied separately, so a
/// fleet's merged requirements can override what the repository declares locally.
pub fn run_repository_checks_with_requirements(
    root: &Path,
    inventory: &Inventory,
    config: &RepoConfig,
    dependencies: &[DependencyRequirement],
) -> Report {
    run_repository_checks_for_state(root, inventory, config, dependencies, false)
}

/// GitHub refuses writes to an archived repository, and Ordnung refuses to open a
/// pull request against one, so every finding would be unactionable. Reporting the
/// state once is more useful than reporting what cannot be fixed.
pub fn run_repository_checks_for_state(
    root: &Path,
    inventory: &Inventory,
    config: &RepoConfig,
    dependencies: &[DependencyRequirement],
    archived: bool,
) -> Report {
    if archived {
        return archived_report(inventory.root.clone(), |definition| {
            definition.repository_runner.is_some()
        });
    }
    let context = RepositoryCheckContext {
        root,
        inventory,
        dependencies,
        ci_exists: &config.ci_exists,
        codegen: &config.codegen,
        scripts: &config.scripts,
        stray_files: &config.stray_files,
        test_layout: &config.test_layout,
    };
    let mut results = Vec::new();
    for definition in check_definitions() {
        if let Some(run) = definition.repository_runner {
            run(definition, &context, &mut results);
        }
    }

    let mut report = Report {
        repository: inventory.root.clone(),
        results,
    };
    report.apply_policy(&default_policy());
    report
}

pub fn run_github_checks(facts: &GithubRepositoryFacts) -> Report {
    run_github_checks_with_settings(facts, &GithubSettings::default())
}

pub fn run_github_checks_with_settings(
    facts: &GithubRepositoryFacts,
    settings: &GithubSettings,
) -> Report {
    if facts.archived {
        return archived_report(PathBuf::from(&facts.repository), |definition| {
            definition.github_runner.is_some()
        });
    }
    let context = GithubCheckContext { facts, settings };
    let mut results = Vec::new();
    for definition in check_definitions() {
        if let Some(run) = definition.github_runner {
            run(definition, &context, &mut results);
        }
    }
    Report {
        repository: PathBuf::from(&facts.repository),
        results,
    }
}

pub(crate) fn result(
    definition: &'static CheckDefinition,
    status: CheckStatus,
    scope: PathBuf,
    message: impl Into<String>,
) -> CheckResult {
    CheckResult {
        check: definition.id.to_owned(),
        status,
        severity: definition.default_severity,
        scope,
        message: message.into(),
        remediation: None,
    }
}

pub(crate) fn result_with_remediation(
    definition: &'static CheckDefinition,
    status: CheckStatus,
    scope: PathBuf,
    message: impl Into<String>,
    remediation: CheckRemediation,
) -> CheckResult {
    let mut result = result(definition, status, scope, message);
    result.remediation = Some(remediation);
    result
}
