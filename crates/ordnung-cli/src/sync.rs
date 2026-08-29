//! Fleet orchestration.
//!
//! This is the decision logic that joins the `gh` adapter below it to the check
//! and planning layers in `ordnung-core` above it: what a fleet member's
//! effective policy is, what changes it needs, and whether they are applied.
//!
//! It lives here rather than in `main.rs` so it is reachable from tests. Every
//! entry point is generic over [`GhRunner`], so the whole sync path can be
//! driven by a fake runner without a network or a `gh` binary.
//!
//! Nothing in this module prints or chooses an exit code. Callers render the
//! returned values.

use std::path::Path;

use anyhow::{Context, Result, bail};
use ordnung_core::{
    DependencyRequirement, FileChangeSource, FleetConfig, GithubRepositoryFacts,
    GithubSettingChange, InventoryOptions, RemediationPlan, RepoConfig, Report,
    build_remediation_plan, default_policy, inspect_repository, plan_github_settings,
    plan_managed_changes_for_member, resolve_github_settings, resolve_policy,
    run_github_checks_with_settings, run_repository_checks_with_requirements,
};
use serde::Serialize;

use crate::gh::{GhClient, GhRunner, PullRequestMaterialization};
use crate::render::file_operation_name;

/// The result of syncing one fleet member: what was planned, whether it was
/// applied, and the pull request it produced.
#[derive(Debug, Serialize)]
pub struct GithubSyncOutcome {
    pub ok: bool,
    pub plan: RemediationPlan,
    pub applied: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pull_request: Option<PullRequestMaterialization>,
}

/// One member's GitHub audit, or the error that prevented it. A failure against
/// one member must not abandon the rest of the fleet.
#[derive(Debug, Serialize)]
pub struct FleetGithubOutcome {
    pub repository: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<Report>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// One member's sync outcome, or the error that prevented it.
#[derive(Debug, Serialize)]
pub struct FleetGithubSyncMemberOutcome {
    pub repository: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<GithubSyncOutcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Fleet-wide commands act only on explicitly listed members, so a typo in a
/// repository name is an error rather than a silent no-op.
pub fn ensure_explicit_member(fleet: &FleetConfig, repository: &str) -> Result<()> {
    if fleet.members.iter().any(|member| member.repo == repository) {
        Ok(())
    } else {
        bail!(
            "repository {repository:?} is not an explicit member of fleet {:?}",
            fleet.name
        )
    }
}

/// Fleet requirements override same-named local ones, so a member cannot quietly
/// drop a requirement the fleet imposes, but may add requirements of its own.
pub fn fleet_requirements(local: &RepoConfig, fleet: &FleetConfig) -> Vec<DependencyRequirement> {
    let mut merged = local.dependencies.clone();
    for requirement in fleet.effective_dependencies() {
        match merged
            .iter()
            .position(|candidate| candidate.name == requirement.name)
        {
            Some(index) => merged[index] = requirement.clone(),
            None => merged.push(requirement.clone()),
        }
    }
    merged
}

/// Runs the GitHub-backed checks for one member under the fleet's policy.
pub fn check_fleet_member<R: GhRunner>(
    client: &GhClient<R>,
    fleet: &FleetConfig,
    repository: &str,
) -> Result<Report> {
    let facts = client.fetch_repository(repository)?;
    let local = client.fetch_repo_config(&facts)?;
    let policy = resolve_policy(
        &default_policy(),
        Some(&fleet.checks_for(repository)),
        &local,
    )?;
    let settings = resolve_github_settings(Some(&fleet.policy.github), &local)?;
    let mut report = run_github_checks_with_settings(&facts, &settings);
    report.apply_policy(&policy);
    Ok(report)
}

/// Audits every explicit member, collecting per-member errors rather than
/// stopping at the first one.
pub fn check_fleet_members<R: GhRunner>(
    client: &GhClient<R>,
    fleet: &FleetConfig,
) -> Vec<FleetGithubOutcome> {
    fleet
        .members
        .iter()
        .map(
            |member| match check_fleet_member(client, fleet, &member.repo) {
                Ok(report) => FleetGithubOutcome {
                    repository: report.repository.display().to_string(),
                    report: Some(report),
                    error: None,
                },
                Err(error) => FleetGithubOutcome {
                    repository: member.repo.clone(),
                    report: None,
                    error: Some(format!("{error:#}")),
                },
            },
        )
        .collect()
}

/// Plans, and optionally applies, the GitHub settings the fleet mandates for one
/// member.
pub fn plan_fleet_member_settings<R: GhRunner>(
    client: &GhClient<R>,
    fleet: &FleetConfig,
    repository: &str,
) -> Result<(GithubRepositoryFacts, Vec<GithubSettingChange>)> {
    let facts = client.fetch_repository(repository)?;
    if facts.archived {
        bail!(
            "repository {:?} is archived and cannot be changed",
            facts.repository
        );
    }
    let local = client.fetch_repo_config(&facts)?;
    let desired = resolve_github_settings(Some(&fleet.policy.github), &local)?;
    let changes = plan_github_settings(&facts, &desired);
    Ok((facts, changes))
}

/// Syncs one fleet member: resolve its effective policy against a fresh
/// checkout, plan every change the fleet requires, and — only when `apply` is
/// set — write repository settings and materialize the remediation pull request.
///
/// The settings write is immediate and unreviewable; the file changes arrive as
/// a pull request. Both are gated by the same `apply` flag.
pub fn sync_fleet_member<R: GhRunner>(
    client: &GhClient<R>,
    fleet: &FleetConfig,
    repository: &str,
    apply: bool,
) -> Result<GithubSyncOutcome> {
    let facts = client.fetch_repository(repository)?;
    if facts.archived {
        bail!(
            "repository {:?} is archived and cannot be changed",
            facts.repository
        );
    }

    let temporary = tempfile::tempdir().context("cannot create temporary checkout directory")?;
    let checkout = temporary.path().join("repository");
    client.clone_repository(&facts.repository, &checkout)?;

    let local = RepoConfig::load_optional(&checkout)?;
    let policy = resolve_policy(
        &default_policy(),
        Some(&fleet.checks_for(repository)),
        &local,
    )?;
    let settings = resolve_github_settings(Some(&fleet.policy.github), &local)?;
    let inventory = inspect_repository(
        &checkout,
        &InventoryOptions {
            ignore: local.ignore.clone(),
        },
    )?;
    let mut repository_report = run_repository_checks_with_requirements(
        &checkout,
        &inventory,
        &local,
        &fleet_requirements(&local, fleet),
    );
    repository_report.apply_policy(&policy);
    let mut github_report = run_github_checks_with_settings(&facts, &settings);
    github_report.apply_policy(&policy);

    let managed_changes = plan_managed_changes_for_member(
        repository,
        fleet.member(repository).and_then(|m| m.website.as_deref()),
        &checkout,
        &inventory,
        fleet.effective_managed(),
    )?;
    let setting_changes = plan_github_settings(&facts, &settings);
    let plan = build_remediation_plan(
        facts.repository.clone(),
        &[repository_report, github_report],
        &managed_changes,
        setting_changes,
    )?;

    let pull_request = if apply {
        client.apply_setting_changes(&facts.repository, &plan.github_setting_changes)?;
        client.materialize_pull_request(
            &facts.repository,
            &facts.default_branch,
            &plan.file_changes,
            "chore: apply Ordnung remediations",
            &pull_request_body(&plan),
        )?
    } else {
        None
    };
    let has_file_drift = !plan.file_changes.is_empty();
    let has_unapplied_setting_drift = !apply && !plan.github_setting_changes.is_empty();
    let has_required_findings = plan.findings.iter().any(|finding| {
        finding.severity == ordnung_core::Severity::Required
            && matches!(
                finding.status,
                ordnung_core::CheckStatus::Fail | ordnung_core::CheckStatus::Error
            )
    });
    let clean = !has_file_drift && !has_unapplied_setting_drift && !has_required_findings;
    Ok(GithubSyncOutcome {
        ok: clean,
        plan,
        applied: apply,
        pull_request,
    })
}

/// Syncs every explicit member, collecting per-member errors rather than
/// stopping at the first one.
pub fn sync_fleet_members<R: GhRunner>(
    client: &GhClient<R>,
    fleet: &FleetConfig,
    apply: bool,
) -> Vec<FleetGithubSyncMemberOutcome> {
    fleet
        .members
        .iter()
        .map(
            |member| match sync_fleet_member(client, fleet, &member.repo, apply) {
                Ok(outcome) => FleetGithubSyncMemberOutcome {
                    repository: member.repo.clone(),
                    outcome: Some(outcome),
                    error: None,
                },
                Err(error) => FleetGithubSyncMemberOutcome {
                    repository: member.repo.clone(),
                    outcome: None,
                    error: Some(format!("{error:#}")),
                },
            },
        )
        .collect()
}

/// Plans the fleet-managed changes for a member from a local checkout, without
/// touching GitHub.
pub fn plan_local_sync(
    fleet: &FleetConfig,
    repository: &str,
    repo_root: &Path,
) -> Result<RemediationPlan> {
    let local = RepoConfig::load_optional(repo_root)?;
    let policy = resolve_policy(
        &default_policy(),
        Some(&fleet.checks_for(repository)),
        &local,
    )?;
    let inventory = inspect_repository(
        repo_root,
        &InventoryOptions {
            ignore: local.ignore.clone(),
        },
    )?;
    let managed_changes = plan_managed_changes_for_member(
        repository,
        fleet.member(repository).and_then(|m| m.website.as_deref()),
        repo_root,
        &inventory,
        fleet.effective_managed(),
    )?;
    let mut report = run_repository_checks_with_requirements(
        repo_root,
        &inventory,
        &local,
        &fleet_requirements(&local, fleet),
    );
    report.apply_policy(&policy);
    Ok(build_remediation_plan(
        repository,
        &[report],
        &managed_changes,
        Vec::new(),
    )?)
}

/// The body of the remediation pull request. Lists what the plan changes and
/// which check or managed entry asked for it, so a reviewer does not have to
/// read the diff to know why the pull request exists.
pub fn pull_request_body(plan: &RemediationPlan) -> String {
    use std::fmt::Write as _;

    let mut body = String::from(
        "Ordnung found repository drift and generated this consolidated remediation.\n\n## File changes\n",
    );
    for change in &plan.file_changes {
        let sources = change
            .sources
            .iter()
            .map(|source| match source {
                FileChangeSource::Check { check } => format!("`{check}`"),
                FileChangeSource::Managed { entry } => format!("managed `{entry}`"),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            body,
            "- {} `{}` ({sources})",
            file_operation_name(change.operation),
            change.path.display()
        );
    }
    body.push_str("\nThe default branch remains out of policy until this pull request lands.\n");
    body
}
