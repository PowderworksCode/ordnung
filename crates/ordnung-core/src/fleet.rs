use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{CheckPolicy, DependencyRequirement, GithubSettingsPolicy};
use crate::error::{Error, Result};
use crate::inventory::{Inventory, Project, ProjectCapability};
use crate::profile::{EcosystemId, LanguageId, ecosystem_profile, language_profile};

/// The directory that holds every Ordnung configuration file for a repository.
///
/// The directory is the unit of publication: an imported layer is fetched whole,
/// so managed sources resolve against it without a separate repository-root concept.
pub const CONFIG_DIR: &str = ".ordnung";
/// A fleet instance: members plus the policy applied to them.
pub const FLEET_FILE: &str = "fleet.toml";
/// A reusable policy library. Declares no members and is never synced directly.
pub const POLICY_FILE: &str = "policy.toml";
/// A member repository's local exceptions.
pub const OVERRIDES_FILE: &str = "overrides.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FleetConfig {
    pub name: String,
    #[serde(default, rename = "member")]
    pub members: Vec<FleetMember>,
    #[serde(default)]
    pub extends: Vec<Extends>,
    #[serde(default)]
    pub policy: FleetPolicy,
    #[serde(default, rename = "managed")]
    pub managed: Vec<ManagedEntry>,
    #[serde(default, rename = "dependency")]
    pub dependencies: Vec<DependencyRequirement>,
    /// Dependency requirements after inherited layers are merged in.
    #[serde(skip)]
    resolved_dependencies: Vec<DependencyRequirement>,
    /// Managed entries after inherited layers are merged in, each paired with the
    /// layer root that owns its source. Derived at load; never part of the file.
    #[serde(skip)]
    resolved_managed: Vec<ResolvedManaged>,
}

/// A reusable policy layer. Has no members, so importing one can never drag
/// another fleet's repositories along.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyLibrary {
    pub name: String,
    #[serde(default)]
    pub extends: Vec<Extends>,
    #[serde(default)]
    pub policy: FleetPolicy,
    #[serde(default, rename = "managed")]
    pub managed: Vec<ManagedEntry>,
    #[serde(default, rename = "dependency")]
    pub dependencies: Vec<DependencyRequirement>,
}

/// A reference to an inherited policy layer.
///
/// Git references pin a full commit revision: a plan that changes because an
/// upstream branch moved would not be deterministic, and an imported layer can
/// write files into every member repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Extends {
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub git: Option<String>,
    #[serde(default)]
    pub rev: Option<String>,
}

impl Extends {
    fn validate(&self) -> Result<()> {
        match (&self.path, &self.git) {
            // With git, path selects a directory inside the fetched repository, so
            // one repository can publish more than one policy tier.
            (Some(subpath), Some(_)) => {
                validate_relative(subpath, "extends path within a repository")?;
                self.validate_git()
            }
            (None, None) => Err(Error::Config(
                "extends entry must declare either path or git".into(),
            )),
            (Some(path), None) => {
                if self.rev.is_some() {
                    return Err(Error::Config(
                        "extends entry with path cannot declare rev".into(),
                    ));
                }
                // Unlike a managed source, an extends path deliberately names a
                // location outside this configuration directory, so parent
                // components and absolute paths are both legitimate.
                if path.as_os_str().is_empty() {
                    return Err(Error::Config("extends path cannot be empty".into()));
                }
                Ok(())
            }
            (None, Some(_)) => self.validate_git(),
        }
    }

    fn validate_git(&self) -> Result<()> {
        let url = self.git.as_deref().expect("git is present");
        if url.trim().is_empty() {
            return Err(Error::Config("extends git url cannot be empty".into()));
        }
        let Some(rev) = &self.rev else {
            return Err(Error::Config(format!(
                "extends entry for {url:?} requires rev; \
                 a moving reference cannot produce a deterministic plan"
            )));
        };
        if rev.len() != 40 || !rev.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(Error::Config(format!(
                "extends rev {rev:?} must be a full 40-character commit revision"
            )));
        }
        Ok(())
    }
}

/// A managed entry paired with the layer root that owns its source content.
#[derive(Debug, Clone)]
pub struct ResolvedManaged {
    pub root: PathBuf,
    pub entry: ManagedEntry,
}

/// One configuration layer contributing policy and managed entries.
struct Layer {
    root: PathBuf,
    policy: FleetPolicy,
    managed: Vec<ManagedEntry>,
    dependencies: Vec<DependencyRequirement>,
}

impl FleetConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path).map_err(|source| Error::io(path, source))?;
        let mut config: Self = toml::from_str(&text).map_err(|error| Error::Parse {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
        let root = path.parent().unwrap_or_else(|| Path::new("."));
        config.validate(root)?;

        let mut layers = Vec::new();
        let mut visiting = Vec::new();
        resolve_extends(root, &config.extends, &mut layers, &mut visiting)?;
        layers.push(Layer {
            root: root.to_path_buf(),
            policy: config.policy.clone(),
            managed: config.managed.clone(),
            dependencies: config.dependencies.clone(),
        });

        config.policy = merge_policy(&layers);
        config.resolved_managed = merge_managed(&layers)?;
        config.resolved_dependencies = merge_dependencies(&layers)?;
        config.validate_resolved()?;
        Ok(config)
    }

    /// Managed entries after inheritance, each paired with its owning layer root.
    pub fn effective_managed(&self) -> &[ResolvedManaged] {
        &self.resolved_managed
    }

    /// Dependency requirements after inheritance.
    pub fn effective_dependencies(&self) -> &[DependencyRequirement] {
        &self.resolved_dependencies
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
        for extends in &self.extends {
            extends.validate()?;
        }
        for requirement in &self.dependencies {
            requirement.validate()?;
        }
        validate_layer_managed(&self.managed, fleet_root)
    }

    /// Checks that can only run once inheritance is merged: repository targets
    /// must name members, and each destination must have exactly one owner.
    fn validate_resolved(&self) -> Result<()> {
        let repos: BTreeSet<&String> = self.members.iter().map(|member| &member.repo).collect();
        let mut ownership: Vec<&ResolvedManaged> = Vec::new();
        for resolved in &self.resolved_managed {
            let managed = &resolved.entry;
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
            if let Some(existing) = ownership
                .iter()
                .find(|other| managed_entries_overlap(&other.entry, managed))
            {
                return Err(Error::Config(format!(
                    "managed destination {} is already owned by entry {:?} from {}; \
                     reuse that name to override it, or set state = \"unmanaged\" to drop it",
                    managed.destination.display(),
                    existing.entry.name,
                    existing.root.display()
                )));
            }
            ownership.push(resolved);
        }
        Ok(())
    }
}

impl PolicyLibrary {
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

    pub fn validate(&self, root: &Path) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(Error::Config("policy name cannot be empty".into()));
        }
        for extends in &self.extends {
            extends.validate()?;
        }
        for requirement in &self.dependencies {
            requirement.validate()?;
        }
        for managed in &self.managed {
            if !managed.only.is_empty() {
                return Err(Error::Config(format!(
                    "managed entry {:?} declares only, which names member repositories; \
                     a policy library has no members",
                    managed.name
                )));
            }
        }
        validate_layer_managed(&self.managed, root)
    }
}

/// Validation that applies to a single layer's own declarations, before merging.
fn validate_layer_managed(entries: &[ManagedEntry], root: &Path) -> Result<()> {
    let mut names = BTreeSet::new();
    for managed in entries {
        {
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
                    let source_path = root.join(source);
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
                ManagedState::Absent | ManagedState::Unmanaged if managed.source.is_some() => {
                    return Err(Error::Config(format!(
                        "{:?} cannot declare a source when state is {}",
                        managed.name,
                        if managed.state == ManagedState::Absent {
                            "absent"
                        } else {
                            "unmanaged"
                        }
                    )));
                }
                ManagedState::Absent | ManagedState::Unmanaged => {}
            }
        }
    }
    Ok(())
}

/// Resolves inherited layers depth first, so a layer always appears after
/// everything it inherits from and later layers override earlier ones.
fn resolve_extends(
    root: &Path,
    extends: &[Extends],
    layers: &mut Vec<Layer>,
    visiting: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in extends {
        entry.validate()?;
        let layer_root = match (&entry.path, &entry.git) {
            (subpath, Some(url)) => fetch_git_layer(
                url,
                entry.rev.as_ref().expect("validated"),
                subpath.as_deref(),
            )?,
            (Some(path), None) => root.join(path),
            (None, None) => unreachable!("validated"),
        };
        let layer_root = layer_root
            .canonicalize()
            .map_err(|source| Error::Config(format!("{}: {source}", layer_root.display())))?;

        if visiting.contains(&layer_root) {
            return Err(Error::Config(format!(
                "extends cycle through {}",
                layer_root.display()
            )));
        }
        if layers.iter().any(|layer| layer.root == layer_root) {
            continue;
        }

        let policy_path = layer_root.join(POLICY_FILE);
        if !policy_path.is_file() {
            if layer_root.join(FLEET_FILE).is_file() {
                return Err(Error::Config(format!(
                    "{} contains {FLEET_FILE} but no {POLICY_FILE}; \
                     extends must reference a policy library, because members are never inherited",
                    layer_root.display()
                )));
            }
            return Err(Error::Config(format!(
                "{} contains no {POLICY_FILE}",
                layer_root.display()
            )));
        }

        let library = PolicyLibrary::load(&policy_path)?;
        visiting.push(layer_root.clone());
        resolve_extends(&layer_root, &library.extends, layers, visiting)?;
        visiting.pop();
        layers.push(Layer {
            root: layer_root,
            policy: library.policy,
            managed: library.managed,
            dependencies: library.dependencies,
        });
    }
    Ok(())
}

/// Fetches a pinned revision into a content-addressed cache. A pinned revision
/// is immutable, so an existing entry is reused without touching the network.
/// `subpath` selects a directory within the fetched repository, so one repository
/// can publish several policy tiers. It defaults to the repository's own
/// `.ordnung` directory, which is the shape of a dedicated configuration repository.
fn fetch_git_layer(url: &str, rev: &str, subpath: Option<&Path>) -> Result<PathBuf> {
    let cache = cache_root()?.join(format!("{}-{rev}", url_slug(url)));
    let relative = subpath.unwrap_or(Path::new(CONFIG_DIR));
    let layer = cache.join(relative);
    // A pinned revision is immutable, so an existing checkout is reused as is.
    if cache.join(".git").is_dir() {
        return require_layer_dir(&layer, url, rev, relative);
    }
    if cache.exists() {
        fs::remove_dir_all(&cache).map_err(|source| Error::io(&cache, source))?;
    }
    fs::create_dir_all(&cache).map_err(|source| Error::io(&cache, source))?;

    git(&cache, &["init", "--quiet"])?;
    git(&cache, &["remote", "add", "origin", url])?;
    git(&cache, &["fetch", "--quiet", "--depth", "1", "origin", rev])?;
    git(&cache, &["checkout", "--quiet", "FETCH_HEAD"])?;

    require_layer_dir(&layer, url, rev, relative)
}

fn require_layer_dir(layer: &Path, url: &str, rev: &str, relative: &Path) -> Result<PathBuf> {
    if layer.is_dir() {
        return Ok(layer.to_path_buf());
    }
    Err(Error::Config(format!(
        "{url} at {rev} contains no {} directory",
        relative.display()
    )))
}

fn git(dir: &Path, args: &[&str]) -> Result<()> {
    let output = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|source| Error::io(dir, source))?;
    if !output.status.success() {
        return Err(Error::Config(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

fn cache_root() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("ORDNUNG_CACHE_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var_os("HOME")
        .ok_or_else(|| Error::Config("HOME is not set; set ORDNUNG_CACHE_DIR".into()))?;
    Ok(PathBuf::from(home).join(".cache").join("ordnung"))
}

/// A filesystem-safe, collision-resistant stand-in for a remote URL.
fn url_slug(url: &str) -> String {
    let name: String = url
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let digest = url.bytes().fold(1469598103934665603u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(1099511628211)
    });
    let trimmed: String = name.trim_matches('-').chars().take(48).collect();
    format!("{trimmed}-{digest:016x}")
}

/// Later layers win. Check severities merge per check id; GitHub settings per field.
fn merge_policy(layers: &[Layer]) -> FleetPolicy {
    let mut merged = FleetPolicy::default();
    for layer in layers {
        for (name, policy) in &layer.policy.checks {
            merged.checks.insert(name.clone(), policy.clone());
        }
        let github = &layer.policy.github;
        if let Some(value) = &github.allow_auto_merge {
            merged.github.allow_auto_merge = Some(value.clone());
        }
        if let Some(value) = &github.delete_branch_on_merge {
            merged.github.delete_branch_on_merge = Some(value.clone());
        }
        if let Some(value) = &github.allow_update_branch {
            merged.github.allow_update_branch = Some(value.clone());
        }
    }
    merged
}

/// Merges managed entries by name, preserving first-declaration order so plans
/// stay stable. `Unmanaged` drops an inherited entry rather than deleting files.
fn merge_managed(layers: &[Layer]) -> Result<Vec<ResolvedManaged>> {
    let mut merged: Vec<ResolvedManaged> = Vec::new();
    for layer in layers {
        for entry in &layer.managed {
            let existing = merged
                .iter()
                .position(|resolved| resolved.entry.name == entry.name);
            match (entry.state, existing) {
                (ManagedState::Unmanaged, Some(index)) => {
                    merged.remove(index);
                }
                (ManagedState::Unmanaged, None) => {
                    return Err(Error::Config(format!(
                        "unmanaged entry {:?} does not match any inherited managed entry",
                        entry.name
                    )));
                }
                (_, Some(index)) => {
                    merged[index] = ResolvedManaged {
                        root: layer.root.clone(),
                        entry: entry.clone(),
                    };
                }
                (_, None) => merged.push(ResolvedManaged {
                    root: layer.root.clone(),
                    entry: entry.clone(),
                }),
            }
        }
    }
    Ok(merged)
}

/// Merges dependency requirements by name, mirroring managed entries:
/// a later layer replaces an inherited entry, and `unmanaged` drops it.
fn merge_dependencies(layers: &[Layer]) -> Result<Vec<DependencyRequirement>> {
    let mut merged: Vec<DependencyRequirement> = Vec::new();
    for layer in layers {
        for requirement in &layer.dependencies {
            let existing = merged
                .iter()
                .position(|candidate| candidate.name == requirement.name);
            match (requirement.state, existing) {
                (ManagedState::Unmanaged, Some(index)) => {
                    merged.remove(index);
                }
                (ManagedState::Unmanaged, None) => {
                    return Err(Error::Config(format!(
                        "unmanaged dependency requirement {:?} does not match any inherited requirement",
                        requirement.name
                    )));
                }
                (_, Some(index)) => merged[index] = requirement.clone(),
                (_, None) => merged.push(requirement.clone()),
            }
        }
    }
    Ok(merged)
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

/// What a managed entry asserts about its destination in each member repository.
///
/// `Absent` is an assertion that deletes; `Unmanaged` only drops an inherited
/// entry and never touches member files. Keeping them distinct means opting out
/// of an upstream entry cannot silently delete files across the fleet.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagedState {
    #[default]
    Present,
    Absent,
    Unmanaged,
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
    member_repo: &str,
    member_root: &Path,
    inventory: &Inventory,
    entries: &[ResolvedManaged],
) -> Result<Vec<ManagedChange>> {
    validate_repo_name(member_repo)?;
    let member_root = member_root
        .canonicalize()
        .map_err(|source| Error::io(member_root, source))?;
    let mut planned: BTreeMap<PathBuf, ManagedChange> = BTreeMap::new();

    for resolved in entries {
        let entry = &resolved.entry;
        // Sources resolve against the layer that declared them, not the fleet.
        let fleet_root = resolved.root.as_path();
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
                // Merging already removed these; they never reach a member repo.
                ManagedState::Unmanaged => {}
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
