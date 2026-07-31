pub mod check;
mod checks;
pub mod config;
pub mod error;
pub mod fleet;
pub mod github;
pub mod inventory;
pub mod plan;
pub mod profile;

pub use check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckRemediation, CheckResult, CheckStatus,
    GithubCheckRunner, Report, RepositoryCheckContext, RepositoryCheckRunner, Severity,
    check_definition, check_definitions, check_ids, default_policy, run_github_checks,
    run_github_checks_with_settings, run_repository_checks, run_repository_checks_with_config,
    run_repository_checks_with_repo_config,
};
pub use config::{
    BooleanSettingOverride, BooleanSettingPolicy, CheckPolicy, CiExistsConfig, CodegenConfig,
    GithubSettings, GithubSettingsOverrides, GithubSettingsPolicy, LanguageTestLayout,
    LocalOverride, RepoConfig, ScriptsConfig, StrayFilesConfig, TestLayoutConfig,
    resolve_github_settings, resolve_policy,
};
pub use entl_codebase::{
    Artifact, ArtifactId, ArtifactProfile, LanguageConventions, TaskKind, TestLayoutDefaults,
    ToolId, TypecheckConvention, artifact_profile, artifact_profiles, language_conventions,
};
pub use entl_github::{GithubInventory, TaskInvocation, Workflow, WorkflowCommand};
pub use error::{Error, Result};
pub use fleet::{ChangeKind, FleetConfig, ManagedChange, apply_changes, plan_managed_changes};
pub use github::{
    DependabotAutomergeWorkflowFacts, GithubActionPublicationFacts, GithubActionsPermissionsFacts,
    GithubBranchFacts, GithubBranchProtectionFacts, GithubDefaultWorkflowPermissions,
    GithubLicenseFacts, GithubPullRequestAgeFacts, GithubRepositoryFacts, GithubRulesetBypassActor,
    GithubRulesetFacts, GithubSecurityFacts, GithubSetting, GithubSettingChange, GithubStaleFacts,
    GithubValue, GithubWorkflowFacts, GithubWorkflowRun, plan_github_settings,
};
pub use inventory::{
    Inventory, InventoryIssue, InventoryOptions, PackageInstance, Project, ProjectCapability,
    inspect_repository,
};
pub use plan::{
    FileChangeSource, FileOperation, PlannedFileChange, RemediationPlan, apply_file_changes,
    build_remediation_plan,
};
pub use profile::{
    DependencyPinPolicy, DependencyPinStatus, DependencyPinSyntax, EcosystemId, EcosystemProfile,
    EcosystemRegistration, EcosystemRole, LanguageId, LanguageProfile, LanguageRegistration,
    LanguageRole, ManifestSelection, ecosystem_profile, ecosystem_profiles, language_profile,
    language_profiles,
};
