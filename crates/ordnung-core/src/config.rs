use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use entl::codebase::{language_conventions, language_profiles};

use crate::check::Severity;
use crate::error::{Error, Result};
use crate::profile::{LanguageProfile, language_profile};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepoConfig {
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default)]
    pub checks: BTreeMap<String, CheckPolicy>,
    #[serde(default)]
    pub overrides: BTreeMap<String, LocalOverride>,
    #[serde(default)]
    pub github: GithubSettings,
    #[serde(default)]
    pub github_overrides: GithubSettingsOverrides,
    #[serde(default)]
    pub ci_exists: CiExistsConfig,
    #[serde(default)]
    pub codegen: Vec<CodegenConfig>,
    #[serde(default)]
    pub scripts: ScriptsConfig,
    #[serde(default)]
    pub stray_files: StrayFilesConfig,
    #[serde(default)]
    pub test_layout: TestLayoutConfig,
    #[serde(default, rename = "dependency")]
    pub dependencies: Vec<DependencyRequirement>,
}

impl RepoConfig {
    pub fn load_optional(repo_root: &Path) -> Result<Self> {
        let path = repo_root
            .join(crate::fleet::CONFIG_DIR)
            .join(crate::fleet::OVERRIDES_FILE);
        if !path.is_file() {
            return Ok(Self::default());
        }
        let text = fs::read_to_string(&path).map_err(|source| Error::io(&path, source))?;
        Self::parse(path, &text)
    }

    pub fn parse(path: impl Into<PathBuf>, text: &str) -> Result<Self> {
        let path = path.into();
        let config: Self = toml::from_str(text).map_err(|error| Error::Parse {
            path,
            message: error.to_string(),
        })?;
        config.ci_exists.validate()?;
        CodegenConfig::validate_all(&config.codegen)?;
        config.scripts.validate()?;
        config.stray_files.validate()?;
        config.test_layout.validate()?;
        for requirement in &config.dependencies {
            requirement.validate()?;
        }
        Ok(config)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StrayFilesConfig {
    pub notes: PathBuf,
    pub allow: Vec<PathBuf>,
}

impl Default for StrayFilesConfig {
    fn default() -> Self {
        Self {
            notes: "notes".into(),
            allow: Vec::new(),
        }
    }
}

impl StrayFilesConfig {
    fn validate(&self) -> Result<()> {
        validate_relative_path("stray-files notes", &self.notes, false)?;
        let mut allow = BTreeSet::new();
        for path in &self.allow {
            validate_relative_path("stray-files allow entry", path, false)?;
            if path.components().count() != 1 {
                return Err(Error::Config(format!(
                    "stray-files allow entry must name a root file: {:?}",
                    path.display().to_string()
                )));
            }
            if !allow.insert(path) {
                return Err(Error::Config(format!(
                    "duplicate stray-files allow entry {:?}",
                    path.display().to_string()
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ScriptsConfig {
    pub directory: PathBuf,
    pub development: PathBuf,
    pub allow: Vec<PathBuf>,
    pub ignore_directories: Vec<String>,
}

impl Default for ScriptsConfig {
    fn default() -> Self {
        Self {
            directory: PathBuf::from("scripts"),
            development: PathBuf::from("dev.sh"),
            allow: Vec::new(),
            ignore_directories: vec![
                "node_modules".into(),
                "vendor".into(),
                "target".into(),
                "dist".into(),
                "build".into(),
                "__pycache__".into(),
            ],
        }
    }
}

impl ScriptsConfig {
    pub fn validate(&self) -> Result<()> {
        validate_relative_path("scripts directory", &self.directory, false)?;
        validate_relative_path("scripts development entry", &self.development, false)?;
        let mut allowed = BTreeSet::new();
        for path in &self.allow {
            validate_relative_path("scripts allow entry", path, false)?;
            if !allowed.insert(path) {
                return Err(Error::Config(format!(
                    "duplicate scripts allow entry {:?}",
                    path.display().to_string()
                )));
            }
        }
        let mut ignored = BTreeSet::new();
        for directory in &self.ignore_directories {
            if directory.is_empty()
                || directory == "."
                || directory == ".."
                || directory.contains('/')
                || directory.contains('\\')
            {
                return Err(Error::Config(format!(
                    "scripts ignored directory must be one path component: {directory:?}"
                )));
            }
            if !ignored.insert(directory) {
                return Err(Error::Config(format!(
                    "duplicate scripts ignored directory {directory:?}"
                )));
            }
        }
        Ok(())
    }

    pub fn development_path(&self) -> PathBuf {
        self.directory.join(&self.development)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodegenConfig {
    pub name: String,
    #[serde(default)]
    pub root: PathBuf,
    pub command: String,
    pub outputs: Vec<String>,
}

impl CodegenConfig {
    fn validate_all(entries: &[Self]) -> Result<()> {
        let mut names = std::collections::BTreeSet::new();
        for entry in entries {
            if entry.name.trim().is_empty() {
                return Err(Error::Config("codegen name must not be empty".into()));
            }
            if !names.insert(entry.name.as_str()) {
                return Err(Error::Config(format!(
                    "duplicate codegen name {:?}",
                    entry.name
                )));
            }
            validate_relative_path("codegen root", &entry.root, true)?;
            let tokens = shell_words::split(&entry.command).map_err(|error| {
                Error::Config(format!(
                    "invalid codegen command for {:?}: {error}",
                    entry.name
                ))
            })?;
            if tokens.is_empty()
                || tokens
                    .iter()
                    .any(|token| matches!(token.as_str(), "&&" | "||" | ";" | "|"))
                || entl::codebase::normalize_invocation(&tokens).is_none()
            {
                return Err(Error::Config(format!(
                    "codegen command for {:?} must be one executable invocation",
                    entry.name
                )));
            }
            if entry.outputs.is_empty() {
                return Err(Error::Config(format!(
                    "codegen {:?} must declare at least one output pattern",
                    entry.name
                )));
            }
            for output in &entry.outputs {
                let path = Path::new(output);
                validate_relative_path("codegen output", path, false)?;
                globset::Glob::new(output).map_err(|error| {
                    Error::Config(format!(
                        "invalid output pattern {output:?} for codegen {:?}: {error}",
                        entry.name
                    ))
                })?;
            }
        }
        Ok(())
    }

    pub fn normalized_command(&self) -> (String, Vec<String>) {
        let tokens = shell_words::split(&self.command)
            .expect("validated codegen commands remain valid shell words");
        entl::codebase::normalize_invocation(&tokens)
            .expect("validated codegen commands contain an invocation")
    }

    pub fn scope_root(&self) -> &Path {
        if self.root == Path::new(".") {
            Path::new("")
        } else {
            &self.root
        }
    }
}

fn validate_relative_path(label: &str, path: &Path, allow_empty: bool) -> Result<()> {
    if (!allow_empty && path.as_os_str().is_empty())
        || path.is_absolute()
        || path.components().any(|component| {
            !matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
    {
        return Err(Error::Config(format!(
            "{label} must be a relative repository path: {:?}",
            path.display().to_string()
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CiExistsConfig {
    pub ignore: Vec<String>,
}

impl CiExistsConfig {
    pub fn validate(&self) -> Result<()> {
        for pattern in &self.ignore {
            let normalized = pattern.trim_matches('/');
            if normalized.is_empty()
                || pattern.starts_with('/')
                || normalized.split('/').any(|component| component == "..")
            {
                return Err(Error::Config(format!(
                    "ci_exists ignore pattern must be a non-empty relative path: {pattern:?}"
                )));
            }
            globset::Glob::new(normalized).map_err(|error| {
                Error::Config(format!(
                    "invalid ci_exists ignore pattern {pattern:?}: {error}"
                ))
            })?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckPolicy {
    pub severity: Severity,
    #[serde(default)]
    pub allow_override: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalOverride {
    pub severity: Severity,
    pub reason: String,
}

/// Packages every matching package must declare as a dependency.
///
/// Selectors match a discovered package, which carries exactly one language and
/// one ecosystem, so `require` names are unambiguous for a single registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyRequirement {
    pub name: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub ecosystem: Option<String>,
    #[serde(default)]
    pub require: Vec<String>,
    /// Restricts which dependency kind satisfies the requirement. Any kind
    /// satisfies it by default.
    #[serde(default)]
    pub kind: Option<entl::codebase::DependencyKind>,
    #[serde(default)]
    pub state: crate::fleet::ManagedState,
}

impl DependencyRequirement {
    pub fn validate(&self) -> Result<()> {
        use crate::fleet::ManagedState;
        if self.name.trim().is_empty() {
            return Err(Error::Config(
                "dependency requirement name cannot be empty".into(),
            ));
        }
        match self.state {
            // Removing a dependency is never safe to infer: other code may use it.
            ManagedState::Absent => {
                return Err(Error::Config(format!(
                    "dependency requirement {:?} cannot be absent; \
                     Ordnung does not remove dependencies",
                    self.name
                )));
            }
            ManagedState::Unmanaged => return Ok(()),
            ManagedState::Present => {}
        }
        if self.language.is_none() && self.ecosystem.is_none() {
            return Err(Error::Config(format!(
                "dependency requirement {:?} must select a language or an ecosystem",
                self.name
            )));
        }
        if let Some(language) = &self.language {
            if language_profile(language).is_none() {
                return Err(Error::Config(format!(
                    "unknown language profile {language:?} in dependency requirement {:?}",
                    self.name
                )));
            }
        }
        if let Some(ecosystem) = &self.ecosystem {
            if crate::profile::ecosystem_profile(ecosystem).is_none() {
                return Err(Error::Config(format!(
                    "unknown ecosystem profile {ecosystem:?} in dependency requirement {:?}",
                    self.name
                )));
            }
        }
        if self.require.is_empty() {
            return Err(Error::Config(format!(
                "dependency requirement {:?} lists no packages to require",
                self.name
            )));
        }
        if self.require.iter().any(|package| package.trim().is_empty()) {
            return Err(Error::Config(format!(
                "dependency requirement {:?} contains an empty package name",
                self.name
            )));
        }
        Ok(())
    }

    /// Whether this requirement applies to a discovered package.
    ///
    /// A package's own language comes from its manifest, so a Node or Bun package
    /// reports `javascript` even when the project it belongs to is TypeScript.
    /// Both are accepted, otherwise `language = "typescript"` would silently match
    /// nothing.
    pub fn matches<'a>(
        &self,
        languages: impl IntoIterator<Item = &'a str>,
        ecosystem: &str,
    ) -> bool {
        let language_matches = match self.language.as_deref() {
            None => true,
            Some(selector) => languages.into_iter().any(|language| language == selector),
        };
        language_matches
            && self
                .ecosystem
                .as_deref()
                .is_none_or(|selector| selector == ecosystem)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GithubSettings {
    pub allow_auto_merge: Option<bool>,
    pub delete_branch_on_merge: Option<bool>,
    pub allow_update_branch: Option<bool>,
}

impl GithubSettings {
    pub fn is_empty(&self) -> bool {
        self.allow_auto_merge.is_none()
            && self.delete_branch_on_merge.is_none()
            && self.allow_update_branch.is_none()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GithubSettingsPolicy {
    pub allow_auto_merge: Option<BooleanSettingPolicy>,
    pub delete_branch_on_merge: Option<BooleanSettingPolicy>,
    pub allow_update_branch: Option<BooleanSettingPolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BooleanSettingPolicy {
    pub value: bool,
    #[serde(default)]
    pub allow_override: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GithubSettingsOverrides {
    pub allow_auto_merge: Option<BooleanSettingOverride>,
    pub delete_branch_on_merge: Option<BooleanSettingOverride>,
    pub allow_update_branch: Option<BooleanSettingOverride>,
}

impl GithubSettingsOverrides {
    fn is_empty(&self) -> bool {
        self.allow_auto_merge.is_none()
            && self.delete_branch_on_merge.is_none()
            && self.allow_update_branch.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BooleanSettingOverride {
    pub value: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TestLayoutConfig {
    pub ignore: Vec<String>,
    #[serde(flatten)]
    pub languages: BTreeMap<String, LanguageTestLayout>,
}

impl Default for TestLayoutConfig {
    fn default() -> Self {
        Self {
            ignore: Vec::new(),
            languages: language_profiles()
                .iter()
                .filter(|language| language.conventions.is_some())
                .map(|language| {
                    (
                        language.id.into(),
                        LanguageTestLayout::for_profile(language),
                    )
                })
                .collect(),
        }
    }
}

impl TestLayoutConfig {
    pub fn layout_for(&self, profile: &LanguageProfile) -> LanguageTestLayout {
        self.languages
            .get(profile.id)
            .cloned()
            .unwrap_or_else(|| LanguageTestLayout::for_profile(profile))
    }

    pub fn validate(&self) -> Result<()> {
        for language in self.languages.keys() {
            let Some(profile) = language_profile(language) else {
                return Err(Error::Config(format!(
                    "unknown test-layout language profile {language:?}"
                )));
            };
            if language_conventions(profile).is_none() {
                return Err(Error::Config(format!(
                    "unsupported test-layout language profile {language:?}"
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LanguageTestLayout {
    pub source_roots: Vec<PathBuf>,
    pub test_root: PathBuf,
    pub test_suffixes: Vec<String>,
}

impl Default for LanguageTestLayout {
    fn default() -> Self {
        Self {
            source_roots: vec![PathBuf::from("src")],
            test_root: PathBuf::from("tests"),
            test_suffixes: vec![String::new(), ".test".into(), ".spec".into()],
        }
    }
}

impl LanguageTestLayout {
    pub fn for_profile(profile: &LanguageProfile) -> Self {
        language_conventions(profile).map_or_else(Self::default, |conventions| Self {
            source_roots: conventions
                .test_layout
                .source_roots
                .iter()
                .map(PathBuf::from)
                .collect(),
            test_root: conventions.test_layout.test_root.into(),
            test_suffixes: conventions
                .test_layout
                .test_suffixes
                .iter()
                .map(|suffix| (*suffix).into())
                .collect(),
        })
    }
}

pub fn resolve_policy(
    defaults: &BTreeMap<String, Severity>,
    fleet: Option<&BTreeMap<String, CheckPolicy>>,
    local: &RepoConfig,
) -> Result<BTreeMap<String, Severity>> {
    let mut resolved = defaults.clone();

    let validate_check = |name: &str| {
        if defaults.contains_key(name) {
            Ok(())
        } else {
            Err(Error::Config(format!("unknown check {name:?}")))
        }
    };

    match fleet {
        None => {
            for (name, policy) in &local.checks {
                validate_check(name)?;
                resolved.insert(name.clone(), policy.severity);
            }
            if !local.overrides.is_empty() {
                return Err(Error::Config(
                    "[overrides] is only valid when fleet policy is active".into(),
                ));
            }
        }
        Some(fleet_policy) => {
            if !local.checks.is_empty() {
                return Err(Error::Config(
                    "fleet members must request exceptions under [overrides], not [checks]".into(),
                ));
            }
            for (name, policy) in fleet_policy {
                validate_check(name)?;
                resolved.insert(name.clone(), policy.severity);
            }
            for (name, local_override) in &local.overrides {
                validate_check(name)?;
                let Some(policy) = fleet_policy.get(name) else {
                    return Err(Error::Config(format!(
                        "override for {name:?} has no matching fleet policy"
                    )));
                };
                if !policy.allow_override {
                    return Err(Error::Config(format!(
                        "fleet policy does not permit overriding {name:?}"
                    )));
                }
                if local_override.reason.trim().is_empty() {
                    return Err(Error::Config(format!(
                        "override for {name:?} requires a non-empty reason"
                    )));
                }
                resolved.insert(name.clone(), local_override.severity);
            }
        }
    }

    Ok(resolved)
}

pub fn resolve_github_settings(
    fleet: Option<&GithubSettingsPolicy>,
    local: &RepoConfig,
) -> Result<GithubSettings> {
    match fleet {
        None => {
            if !local.github_overrides.is_empty() {
                return Err(Error::Config(
                    "[github_overrides] is only valid when fleet policy is active".into(),
                ));
            }
            Ok(local.github.clone())
        }
        Some(fleet) => {
            if !local.github.is_empty() {
                return Err(Error::Config(
                    "fleet members must request GitHub setting exceptions under [github_overrides]"
                        .into(),
                ));
            }
            Ok(GithubSettings {
                allow_auto_merge: resolve_boolean_setting(
                    "allow_auto_merge",
                    fleet.allow_auto_merge.as_ref(),
                    local.github_overrides.allow_auto_merge.as_ref(),
                )?,
                delete_branch_on_merge: resolve_boolean_setting(
                    "delete_branch_on_merge",
                    fleet.delete_branch_on_merge.as_ref(),
                    local.github_overrides.delete_branch_on_merge.as_ref(),
                )?,
                allow_update_branch: resolve_boolean_setting(
                    "allow_update_branch",
                    fleet.allow_update_branch.as_ref(),
                    local.github_overrides.allow_update_branch.as_ref(),
                )?,
            })
        }
    }
}

fn resolve_boolean_setting(
    name: &str,
    fleet: Option<&BooleanSettingPolicy>,
    local: Option<&BooleanSettingOverride>,
) -> Result<Option<bool>> {
    let Some(local) = local else {
        return Ok(fleet.map(|policy| policy.value));
    };
    let Some(fleet) = fleet else {
        return Err(Error::Config(format!(
            "GitHub setting override for {name:?} has no matching fleet policy"
        )));
    };
    if !fleet.allow_override {
        return Err(Error::Config(format!(
            "fleet policy does not permit overriding GitHub setting {name:?}"
        )));
    }
    if local.reason.trim().is_empty() {
        return Err(Error::Config(format!(
            "GitHub setting override for {name:?} requires a non-empty reason"
        )));
    }
    Ok(Some(local.value))
}
