use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use base64::Engine;
use ordnung_core::{
    FileOperation, GithubActionsPermissionsFacts, GithubBranchFacts, GithubBranchProtectionFacts,
    GithubDefaultWorkflowPermissions, GithubLicenseFacts, GithubPullRequestAgeFacts,
    GithubRepositoryFacts, GithubRulesetBypassActor, GithubRulesetFacts, GithubSecurityFacts,
    GithubSettingChange, GithubStaleFacts, GithubValue, GithubWorkflowFacts, GithubWorkflowRun,
    PlannedFileChange, RepoConfig,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const API_VERSION: &str = "2026-03-10";
pub const REMEDIATION_BRANCH: &str = "ordnung/remediation";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PullRequestStatus {
    Created,
    Updated,
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PullRequestMaterialization {
    pub status: PullRequestStatus,
    pub branch: String,
    pub number: u64,
    pub url: String,
    pub commit: String,
}

pub struct GhClient<R = ProcessRunner> {
    runner: R,
}

impl GhClient<ProcessRunner> {
    pub fn new() -> Self {
        let program = std::env::var_os("ORDNUNG_GH").unwrap_or_else(|| OsString::from("gh"));
        Self {
            runner: ProcessRunner { program },
        }
    }
}

impl Default for GhClient<ProcessRunner> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: GhRunner> GhClient<R> {
    pub fn with_runner(runner: R) -> Self {
        Self { runner }
    }

    pub fn fetch_repository(&self, repository: &str) -> Result<GithubRepositoryFacts> {
        validate_repository(repository)?;
        let repo: RepoResponse = self.api_json(&format!("repos/{repository}"))?;
        validate_repository(&repo.full_name)?;

        let branch_name = encode_component(&repo.default_branch);
        let branch: BranchResponse =
            self.api_json(&format!("repos/{}/branches/{branch_name}", repo.full_name))?;
        let branch_facts = self.branch_facts(&repo.full_name, &branch_name, branch);
        let (workflows, pull_request_checks) =
            self.workflows(&repo.full_name, &repo.default_branch)?;
        let security = security_facts(repo.security_and_analysis);
        let vulnerability_alerts =
            self.boolean_setting(&format!("repos/{}/vulnerability-alerts", repo.full_name));
        let automated_security_fixes = self.boolean_setting(&format!(
            "repos/{}/automated-security-fixes",
            repo.full_name
        ));
        let actions_permissions = self.actions_permissions(&repo.full_name);
        let rulesets = self.rulesets(&repo.full_name);
        let action_publication = self.action_publication(&repo.full_name, &repo.default_branch);
        let stale = self.stale_facts(&repo.full_name, &repo.default_branch);

        Ok(GithubRepositoryFacts {
            repository: repo.full_name,
            default_branch: repo.default_branch,
            visibility: repo.visibility,
            archived: repo.archived,
            description: repo.description,
            homepage: repo.homepage,
            license: repo.license.map(|license| GithubLicenseFacts {
                key: license.key,
                name: license.name,
                spdx_id: license.spdx_id,
            }),
            topics: repo.topics,
            has_issues: repo.has_issues,
            allow_auto_merge: repo.allow_auto_merge,
            delete_branch_on_merge: repo.delete_branch_on_merge,
            allow_update_branch: repo.allow_update_branch,
            branch: branch_facts,
            security,
            vulnerability_alerts,
            automated_security_fixes,
            actions_permissions,
            rulesets,
            pull_request_checks,
            workflows,
            action_publication,
            stale,
        })
    }

    pub fn fetch_repo_config(&self, facts: &GithubRepositoryFacts) -> Result<RepoConfig> {
        let relative = format!(
            "{}/{}",
            ordnung_core::CONFIG_DIR,
            ordnung_core::OVERRIDES_FILE
        );
        let endpoint = format!(
            "repos/{}/contents/{relative}?ref={}",
            facts.repository,
            encode_component(&facts.default_branch)
        );
        let Some(content) = self.api_optional(&endpoint, "application/vnd.github.raw+json")? else {
            return Ok(RepoConfig::default());
        };
        let text = String::from_utf8(content)
            .with_context(|| format!("remote {relative} is not UTF-8"))?;
        RepoConfig::parse(format!("github:{}/{relative}", facts.repository), &text)
            .map_err(Into::into)
    }

    pub fn apply_setting_changes(
        &self,
        repository: &str,
        changes: &[GithubSettingChange],
    ) -> Result<()> {
        validate_repository(repository)?;
        if changes.is_empty() {
            return Ok(());
        }
        let mut args = vec![
            OsString::from("api"),
            OsString::from("--method"),
            OsString::from("PATCH"),
            OsString::from("-H"),
            OsString::from("Accept: application/vnd.github+json"),
            OsString::from("-H"),
            OsString::from(format!("X-GitHub-Api-Version: {API_VERSION}")),
            OsString::from(format!("repos/{repository}")),
        ];
        for change in changes {
            args.push(OsString::from("--field"));
            args.push(OsString::from(format!(
                "{}={}",
                change.setting.as_str(),
                change.desired
            )));
        }
        let output = self.runner.run(&args).context("could not execute gh")?;
        if output.success {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("gh api PATCH repos/{repository} failed: {}", stderr.trim())
        }
    }

    pub fn clone_repository(&self, repository: &str, destination: &Path) -> Result<()> {
        validate_repository(repository)?;
        let args = vec![
            OsString::from("repo"),
            OsString::from("clone"),
            OsString::from(repository),
            destination.as_os_str().to_owned(),
            OsString::from("--"),
            OsString::from("--depth=1"),
        ];
        let output = self.runner.run(&args).context("could not execute gh")?;
        if output.success {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("gh repo clone {repository} failed: {}", stderr.trim())
        }
    }

    pub fn materialize_pull_request(
        &self,
        repository: &str,
        default_branch: &str,
        changes: &[PlannedFileChange],
        title: &str,
        body: &str,
    ) -> Result<Option<PullRequestMaterialization>> {
        validate_repository(repository)?;
        if changes.is_empty() {
            return Ok(None);
        }

        let default_ref: GitRefResponse = self.api_json(&format!(
            "repos/{repository}/git/ref/heads/{}",
            encode_component(default_branch)
        ))?;
        let base_commit: GitCommitResponse = self.api_json(&format!(
            "repos/{repository}/git/commits/{}",
            default_ref.object.sha
        ))?;
        let branch_endpoint = format!(
            "repos/{repository}/git/ref/heads/{}",
            encode_component(REMEDIATION_BRANCH)
        );
        let existing_ref = self
            .api_optional_json::<GitRefResponse>(&branch_endpoint)?
            .map(|reference| reference.object.sha);
        let existing_tree = existing_ref
            .as_deref()
            .map(|sha| {
                self.api_json::<GitCommitResponse>(&format!("repos/{repository}/git/commits/{sha}"))
                    .map(|commit| commit.tree.sha)
            })
            .transpose()?;

        let mut entries = Vec::with_capacity(changes.len());
        for change in changes {
            let path = github_path(&change.path)?;
            match change.operation {
                FileOperation::Create | FileOperation::Update => {
                    let content = change.content().expect("planned write has content");
                    let blob: GitObjectResponse = self.api_json_with_input(
                        "POST",
                        &format!("repos/{repository}/git/blobs"),
                        &json!({
                            "content": base64::engine::general_purpose::STANDARD.encode(content),
                            "encoding": "base64",
                        }),
                    )?;
                    entries.push(json!({
                        "path": path,
                        "mode": "100644",
                        "type": "blob",
                        "sha": blob.sha,
                    }));
                }
                FileOperation::Delete => entries.push(json!({
                    "path": path,
                    "mode": "100644",
                    "type": "blob",
                    "sha": Value::Null,
                })),
            }
        }
        let tree: GitObjectResponse = self.api_json_with_input(
            "POST",
            &format!("repos/{repository}/git/trees"),
            &json!({
                "base_tree": base_commit.tree.sha,
                "tree": entries,
            }),
        )?;

        let (commit_sha, branch_changed) = if existing_tree.as_deref() == Some(&tree.sha) {
            (
                existing_ref.expect("existing tree requires existing ref"),
                false,
            )
        } else {
            let commit: GitObjectResponse = self.api_json_with_input(
                "POST",
                &format!("repos/{repository}/git/commits"),
                &json!({
                    "message": "chore: apply Ordnung remediations",
                    "tree": tree.sha,
                    "parents": [default_ref.object.sha],
                }),
            )?;
            if existing_ref.is_some() {
                let _: Value = self.api_json_with_input(
                    "PATCH",
                    &format!(
                        "repos/{repository}/git/refs/heads/{}",
                        encode_component(REMEDIATION_BRANCH)
                    ),
                    &json!({"sha": commit.sha, "force": true}),
                )?;
            } else {
                let _: Value = self.api_json_with_input(
                    "POST",
                    &format!("repos/{repository}/git/refs"),
                    &json!({
                        "ref": format!("refs/heads/{REMEDIATION_BRANCH}"),
                        "sha": commit.sha,
                    }),
                )?;
            }
            (commit.sha, true)
        };

        let owner = repository.split('/').next().expect("validated repository");
        let pulls_endpoint = format!(
            "repos/{repository}/pulls?state=open&head={}",
            encode_component(&format!("{owner}:{REMEDIATION_BRANCH}"))
        );
        let mut pulls: Vec<PullRequestMutationResponse> = self.api_json(&pulls_endpoint)?;
        let (pull, status) = if let Some(existing) = pulls.pop() {
            if existing.title != title || existing.body.as_deref().unwrap_or_default() != body {
                let pull: PullRequestMutationResponse = self.api_json_with_input(
                    "PATCH",
                    &format!("repos/{repository}/pulls/{}", existing.number),
                    &json!({"title": title, "body": body}),
                )?;
                (pull, PullRequestStatus::Updated)
            } else {
                (
                    existing,
                    if branch_changed {
                        PullRequestStatus::Updated
                    } else {
                        PullRequestStatus::Unchanged
                    },
                )
            }
        } else {
            let pull: PullRequestMutationResponse = self.api_json_with_input(
                "POST",
                &format!("repos/{repository}/pulls"),
                &json!({
                    "title": title,
                    "body": body,
                    "head": REMEDIATION_BRANCH,
                    "base": default_branch,
                    "maintainer_can_modify": true,
                }),
            )?;
            (pull, PullRequestStatus::Created)
        };

        Ok(Some(PullRequestMaterialization {
            status,
            branch: REMEDIATION_BRANCH.into(),
            number: pull.number,
            url: pull.html_url,
            commit: commit_sha,
        }))
    }

    fn branch_facts(
        &self,
        repository: &str,
        branch_name: &str,
        branch: BranchResponse,
    ) -> GithubBranchFacts {
        let mut required_checks: BTreeSet<String> = branch
            .protection
            .required_status_checks
            .contexts
            .into_iter()
            .collect();
        required_checks.extend(
            branch
                .protection
                .required_status_checks
                .checks
                .into_iter()
                .map(|check| check.context),
        );
        let mut required_checks_readable = !required_checks.is_empty();

        if !branch.protected {
            return GithubBranchFacts {
                protected: false,
                protection: GithubValue::known(GithubBranchProtectionFacts {
                    pull_requests_required: false,
                    force_pushes_blocked: false,
                    deletion_blocked: false,
                }),
                required_checks: GithubValue::known(required_checks.into_iter().collect()),
                strict_status_checks: GithubValue::known(false),
            };
        }

        let mut strict_values = Vec::new();
        let mut protection_values = Vec::new();
        let mut errors = Vec::new();
        let rules_endpoint = format!("repos/{repository}/rules/branches/{branch_name}");
        match self.api_attempt(&rules_endpoint) {
            Ok(value) => {
                required_checks_readable = true;
                let mut found_rule = false;
                let protection_count = protection_values.len();
                collect_required_rules(
                    &value,
                    &mut required_checks,
                    &mut strict_values,
                    &mut protection_values,
                    &mut found_rule,
                );
                if !found_rule {
                    strict_values.push(false);
                }
                if protection_values.len() == protection_count {
                    protection_values.push(GithubBranchProtectionFacts {
                        pull_requests_required: false,
                        force_pushes_blocked: false,
                        deletion_blocked: false,
                    });
                }
            }
            Err(error) => errors.push(error),
        }

        let classic_endpoint = format!("repos/{repository}/branches/{branch_name}/protection");
        match self.api_attempt(&classic_endpoint) {
            Ok(value) => match serde_json::from_value::<ClassicProtection>(value) {
                Ok(classic) => {
                    required_checks_readable = true;
                    protection_values.push(GithubBranchProtectionFacts {
                        pull_requests_required: classic.required_pull_request_reviews.is_some(),
                        force_pushes_blocked: classic
                            .allow_force_pushes
                            .is_some_and(|setting| !setting.enabled),
                        deletion_blocked: classic
                            .allow_deletions
                            .is_some_and(|setting| !setting.enabled),
                    });
                    if let Some(status_checks) = classic.required_status_checks {
                        strict_values.push(status_checks.strict);
                        required_checks.extend(status_checks.contexts);
                        required_checks
                            .extend(status_checks.checks.into_iter().map(|check| check.context));
                    } else {
                        strict_values.push(false);
                    }
                }
                Err(error) => errors.push(format!("invalid classic protection response: {error}")),
            },
            Err(error) => errors.push(error),
        }

        GithubBranchFacts {
            protected: true,
            protection: if protection_values.is_empty() {
                GithubValue::unavailable(errors.join("; "))
            } else {
                GithubValue::known(GithubBranchProtectionFacts {
                    pull_requests_required: protection_values
                        .iter()
                        .any(|facts| facts.pull_requests_required),
                    force_pushes_blocked: protection_values
                        .iter()
                        .any(|facts| facts.force_pushes_blocked),
                    deletion_blocked: protection_values.iter().any(|facts| facts.deletion_blocked),
                })
            },
            required_checks: if required_checks_readable {
                GithubValue::known(required_checks.into_iter().collect())
            } else {
                GithubValue::unavailable(errors.join("; "))
            },
            strict_status_checks: if strict_values.is_empty() {
                GithubValue::unavailable(errors.join("; "))
            } else {
                GithubValue::known(strict_values.into_iter().any(|strict| strict))
            },
        }
    }

    fn workflows(
        &self,
        repository: &str,
        default_branch: &str,
    ) -> Result<(Vec<GithubWorkflowFacts>, GithubValue<Vec<String>>)> {
        let response: WorkflowList = self.api_json(&format!(
            "repos/{repository}/actions/workflows?per_page=100"
        ))?;
        if response.total_count > response.workflows.len() as u64 {
            bail!(
                "repository {repository} has more than 100 workflows; pagination is not implemented"
            );
        }

        let mut workflows = Vec::new();
        let mut pull_request_checks = BTreeSet::new();
        let mut check_errors = Vec::new();
        for workflow in response.workflows {
            let inspect = workflow.state == "active" && !workflow.path.starts_with("dynamic/");
            let mut dependabot_automerge = Default::default();
            let latest_run = if inspect {
                let content_endpoint = format!(
                    "repos/{repository}/contents/{}?ref={}",
                    encode_path(&workflow.path),
                    encode_component(default_branch)
                );
                match self.api(&content_endpoint, "application/vnd.github.raw+json") {
                    Ok(bytes) => match String::from_utf8(bytes) {
                        Ok(text) => {
                            match entl_github::pull_request_check_jobs(&text) {
                                Ok(checks) => pull_request_checks.extend(checks),
                                Err(error) => {
                                    check_errors.push(format!("{}: {error}", workflow.path))
                                }
                            }
                            match entl_github::inspect_dependabot_automerge_workflow(&text) {
                                Ok(facts) => dependabot_automerge = facts,
                                Err(error) => {
                                    check_errors.push(format!("{}: {error}", workflow.path))
                                }
                            }
                        }
                        Err(error) => check_errors
                            .push(format!("{}: workflow is not UTF-8: {error}", workflow.path)),
                    },
                    Err(error) => check_errors.push(format!("{}: {error:#}", workflow.path)),
                }

                let runs: WorkflowRunList = self.api_json(&format!(
                    "repos/{repository}/actions/workflows/{}/runs?branch={}&status=completed&per_page=1",
                    workflow.id,
                    encode_component(default_branch)
                ))?;
                runs.workflow_runs
                    .into_iter()
                    .next()
                    .map(|run| GithubWorkflowRun {
                        id: run.id,
                        conclusion: run.conclusion,
                        html_url: run.html_url,
                    })
            } else {
                None
            };
            workflows.push(GithubWorkflowFacts {
                id: workflow.id,
                name: workflow.name,
                path: workflow.path,
                state: workflow.state,
                latest_run,
                dependabot_automerge,
            });
        }
        let pull_request_checks = if check_errors.is_empty() {
            GithubValue::known(pull_request_checks.into_iter().collect())
        } else {
            GithubValue::unavailable(check_errors.join("; "))
        };
        Ok((workflows, pull_request_checks))
    }

    fn action_publication(
        &self,
        repository: &str,
        default_branch: &str,
    ) -> GithubValue<Option<ordnung_core::GithubActionPublicationFacts>> {
        let reference = encode_component(default_branch);
        let mut found = None;
        for path in ["action.yml", "action.yaml"] {
            let endpoint = format!("repos/{repository}/contents/{path}?ref={reference}");
            match self.api_optional(&endpoint, "application/vnd.github.raw+json") {
                Ok(Some(bytes)) => {
                    found = Some((PathBuf::from(path), bytes));
                    break;
                }
                Ok(None) => {}
                Err(error) => return GithubValue::unavailable(format!("{error:#}")),
            }
        }
        let Some((manifest_path, manifest)) = found else {
            return GithubValue::known(None);
        };
        let manifest = match String::from_utf8(manifest) {
            Ok(manifest) => manifest,
            Err(error) => {
                return GithubValue::unavailable(format!(
                    "{} is not UTF-8: {error}",
                    manifest_path.display()
                ));
            }
        };
        let readme_endpoint = format!("repos/{repository}/readme?ref={reference}");
        let readme = match self.api_optional(&readme_endpoint, "application/vnd.github.raw+json") {
            Ok(Some(bytes)) => match String::from_utf8(bytes) {
                Ok(readme) => Some(readme),
                Err(error) => {
                    return GithubValue::unavailable(format!("README is not UTF-8: {error}"));
                }
            },
            Ok(None) => None,
            Err(error) => return GithubValue::unavailable(format!("{error:#}")),
        };
        match entl_github::inspect_action_publication(
            manifest_path,
            &manifest,
            readme.as_ref().map(|_| PathBuf::from("README")),
            readme.as_deref(),
        ) {
            Ok(facts) => GithubValue::known(Some(facts)),
            Err(error) => GithubValue::unavailable(error),
        }
    }

    fn stale_facts(&self, repository: &str, default_branch: &str) -> GithubValue<GithubStaleFacts> {
        let pulls_endpoint = format!("repos/{repository}/pulls?state=open&per_page=100");
        let pulls = match self.api_json::<Vec<PullRequestResponse>>(&pulls_endpoint) {
            Ok(pulls) => pulls,
            Err(error) => return GithubValue::unavailable(format!("{error:#}")),
        };
        let today = unix_days_now();
        let mut open_pull_requests = Vec::new();
        for pull in &pulls {
            let Some(updated) = parse_github_date(&pull.updated_at) else {
                return GithubValue::unavailable(format!(
                    "pull request #{} has invalid updated_at {:?}",
                    pull.number, pull.updated_at
                ));
            };
            open_pull_requests.push(GithubPullRequestAgeFacts {
                number: pull.number,
                updated_at: pull.updated_at.clone(),
                idle_days: today.saturating_sub(updated),
            });
        }
        let branches_endpoint = format!("repos/{repository}/branches?per_page=100");
        let branches = match self.api_json::<Vec<BranchSummary>>(&branches_endpoint) {
            Ok(branches) => branches,
            Err(error) => return GithubValue::unavailable(format!("{error:#}")),
        };
        let others = branches
            .iter()
            .filter(|branch| branch.name != default_branch)
            .collect::<Vec<_>>();
        let mut merged_branches = Vec::new();
        for branch in others.iter().take(20) {
            let endpoint = format!(
                "repos/{repository}/compare/{}...{}",
                encode_component(default_branch),
                encode_component(&branch.name)
            );
            match self.api_json::<CompareResponse>(&endpoint) {
                Ok(compare) if compare.ahead_by == 0 => merged_branches.push(branch.name.clone()),
                Ok(_) => {}
                Err(error) => return GithubValue::unavailable(format!("{error:#}")),
            }
        }
        GithubValue::known(GithubStaleFacts {
            open_pull_requests,
            pull_requests_truncated: pulls.len() == 100,
            merged_branches,
            examined_branches: others.len().min(20),
            non_default_branches: others.len(),
            branches_truncated: branches.len() == 100,
        })
    }

    fn api_json<T: for<'de> Deserialize<'de>>(&self, endpoint: &str) -> Result<T> {
        let bytes = self.api(endpoint, "application/vnd.github+json")?;
        serde_json::from_slice(&bytes)
            .with_context(|| format!("invalid JSON from gh api {endpoint}"))
    }

    fn api_optional_json<T: for<'de> Deserialize<'de>>(&self, endpoint: &str) -> Result<Option<T>> {
        self.api_optional(endpoint, "application/vnd.github+json")?
            .map(|bytes| {
                serde_json::from_slice(&bytes)
                    .with_context(|| format!("invalid JSON from gh api {endpoint}"))
            })
            .transpose()
    }

    fn api_json_with_input<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        endpoint: &str,
        input: &Value,
    ) -> Result<T> {
        let bytes = serde_json::to_vec(input).context("could not serialize GitHub API request")?;
        let args = vec![
            OsString::from("api"),
            OsString::from("--method"),
            OsString::from(method),
            OsString::from("-H"),
            OsString::from("Accept: application/vnd.github+json"),
            OsString::from("-H"),
            OsString::from(format!("X-GitHub-Api-Version: {API_VERSION}")),
            OsString::from(endpoint),
            OsString::from("--input"),
            OsString::from("-"),
        ];
        let output = self
            .runner
            .run_with_input(&args, &bytes)
            .context("could not execute gh")?;
        if !output.success {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("gh api {method} {endpoint} failed: {}", stderr.trim());
        }
        serde_json::from_slice(&output.stdout)
            .with_context(|| format!("invalid JSON from gh api {method} {endpoint}"))
    }

    fn boolean_setting(&self, endpoint: &str) -> GithubValue<bool> {
        match self.api(endpoint, "application/vnd.github+json") {
            Ok(bytes) if bytes.is_empty() => GithubValue::known(true),
            Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
                Ok(value) => GithubValue::known(
                    value
                        .get("enabled")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                ),
                Err(error) => GithubValue::unavailable(format!(
                    "invalid JSON from gh api {endpoint}: {error}"
                )),
            },
            Err(error) if format!("{error:#}").contains("HTTP 404") => GithubValue::known(false),
            Err(error) => GithubValue::unavailable(format!("{error:#}")),
        }
    }

    fn actions_permissions(&self, repository: &str) -> GithubValue<GithubActionsPermissionsFacts> {
        let endpoint = format!("repos/{repository}/actions/permissions/workflow");
        let bytes = match self.api(&endpoint, "application/vnd.github+json") {
            Ok(bytes) => bytes,
            Err(error) => return GithubValue::unavailable(format!("{error:#}")),
        };
        let response = match serde_json::from_slice::<WorkflowPermissionsResponse>(&bytes) {
            Ok(response) => response,
            Err(error) => {
                return GithubValue::unavailable(format!(
                    "invalid JSON from gh api {endpoint}: {error}"
                ));
            }
        };
        let default_workflow_permissions = match response.default_workflow_permissions.as_str() {
            "read" => GithubDefaultWorkflowPermissions::Read,
            "write" => GithubDefaultWorkflowPermissions::Write,
            value => {
                return GithubValue::unavailable(format!(
                    "unknown default workflow permission {value:?}"
                ));
            }
        };
        GithubValue::known(GithubActionsPermissionsFacts {
            default_workflow_permissions,
            can_approve_pull_request_reviews: response.can_approve_pull_request_reviews,
        })
    }

    fn rulesets(&self, repository: &str) -> GithubValue<Vec<GithubRulesetFacts>> {
        let endpoint = format!("repos/{repository}/rulesets?targets=branch&per_page=100");
        let summaries = match self.api_json::<Vec<RulesetSummary>>(&endpoint) {
            Ok(summaries) => summaries,
            Err(error) => return GithubValue::unavailable(format!("{error:#}")),
        };
        let mut facts = Vec::new();
        for summary in summaries {
            if summary.target != "branch" || summary.enforcement != "active" {
                continue;
            }
            let detail_endpoint = format!("repos/{repository}/rulesets/{}", summary.id);
            let detail = match self.api_json::<RulesetDetail>(&detail_endpoint) {
                Ok(detail) => detail,
                Err(error) => return GithubValue::unavailable(format!("{error:#}")),
            };
            facts.push(GithubRulesetFacts {
                id: detail.id,
                name: detail.name,
                target: detail.target,
                enforcement: detail.enforcement,
                rule_types: detail
                    .rules
                    .into_iter()
                    .map(|rule| rule.rule_type)
                    .collect(),
                bypass_actors: detail
                    .bypass_actors
                    .into_iter()
                    .map(|actor| GithubRulesetBypassActor {
                        actor_id: actor.actor_id,
                        actor_type: actor.actor_type,
                        bypass_mode: actor.bypass_mode,
                    })
                    .collect(),
            });
        }
        GithubValue::known(facts)
    }

    fn api_attempt(&self, endpoint: &str) -> std::result::Result<Value, String> {
        self.api(endpoint, "application/vnd.github+json")
            .map_err(|error| format!("{error:#}"))
            .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|error| error.to_string()))
    }

    fn api_optional(&self, endpoint: &str, accept: &str) -> Result<Option<Vec<u8>>> {
        match self.api(endpoint, accept) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if format!("{error:#}").contains("HTTP 404") => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn api(&self, endpoint: &str, accept: &str) -> Result<Vec<u8>> {
        let args = vec![
            OsString::from("api"),
            OsString::from("--method"),
            OsString::from("GET"),
            OsString::from("-H"),
            OsString::from(format!("Accept: {accept}")),
            OsString::from("-H"),
            OsString::from(format!("X-GitHub-Api-Version: {API_VERSION}")),
            OsString::from(endpoint),
        ];
        let output = self.runner.run(&args).context("could not execute gh")?;
        if output.success {
            Ok(output.stdout)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let detail = if stderr.trim().is_empty() {
                stdout.trim()
            } else {
                stderr.trim()
            };
            bail!("gh api {endpoint} failed: {detail}")
        }
    }
}

pub trait GhRunner {
    fn run(&self, args: &[OsString]) -> std::io::Result<GhOutput>;

    fn run_with_input(&self, _args: &[OsString], _input: &[u8]) -> std::io::Result<GhOutput> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "runner does not support standard input",
        ))
    }
}

pub struct ProcessRunner {
    program: OsString,
}

impl GhRunner for ProcessRunner {
    fn run(&self, args: &[OsString]) -> std::io::Result<GhOutput> {
        let output = Command::new(&self.program).args(args).output()?;
        Ok(GhOutput {
            success: output.status.success(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }

    fn run_with_input(&self, args: &[OsString], input: &[u8]) -> std::io::Result<GhOutput> {
        let mut child = Command::new(&self.program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        child.stdin.take().expect("piped stdin").write_all(input)?;
        let output = child.wait_with_output()?;
        Ok(GhOutput {
            success: output.status.success(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
}

pub struct GhOutput {
    pub success: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Deserialize)]
struct RepoResponse {
    full_name: String,
    default_branch: String,
    visibility: String,
    archived: bool,
    description: Option<String>,
    homepage: Option<String>,
    license: Option<LicenseResponse>,
    #[serde(default)]
    topics: Vec<String>,
    has_issues: bool,
    #[serde(default)]
    allow_auto_merge: bool,
    #[serde(default)]
    delete_branch_on_merge: bool,
    #[serde(default)]
    allow_update_branch: bool,
    security_and_analysis: Option<SecurityResponse>,
}

#[derive(Deserialize)]
struct GitRefResponse {
    object: GitObjectResponse,
}

#[derive(Deserialize)]
struct GitObjectResponse {
    sha: String,
}

#[derive(Deserialize)]
struct GitCommitResponse {
    tree: GitObjectResponse,
}

#[derive(Deserialize)]
struct PullRequestMutationResponse {
    number: u64,
    html_url: String,
    title: String,
    body: Option<String>,
}

#[derive(Deserialize)]
struct LicenseResponse {
    key: String,
    name: String,
    spdx_id: String,
}

#[derive(Deserialize)]
struct SecurityResponse {
    secret_scanning: Option<SecurityStatus>,
    secret_scanning_push_protection: Option<SecurityStatus>,
}

#[derive(Deserialize)]
struct SecurityStatus {
    status: String,
}

#[derive(Deserialize)]
struct WorkflowPermissionsResponse {
    default_workflow_permissions: String,
    can_approve_pull_request_reviews: bool,
}

#[derive(Deserialize)]
struct RulesetSummary {
    id: u64,
    target: String,
    enforcement: String,
}

#[derive(Deserialize)]
struct RulesetDetail {
    id: u64,
    name: String,
    target: String,
    enforcement: String,
    #[serde(default)]
    rules: Vec<RulesetRule>,
    #[serde(default)]
    bypass_actors: Vec<RulesetBypassActor>,
}

#[derive(Deserialize)]
struct RulesetRule {
    #[serde(rename = "type")]
    rule_type: String,
}

#[derive(Deserialize)]
struct RulesetBypassActor {
    actor_id: Option<u64>,
    actor_type: String,
    bypass_mode: String,
}

#[derive(Deserialize)]
struct BranchResponse {
    #[serde(default)]
    protected: bool,
    #[serde(default)]
    protection: BranchProtection,
}

#[derive(Default, Deserialize)]
struct BranchProtection {
    #[serde(default)]
    required_status_checks: StatusChecks,
}

#[derive(Default, Deserialize)]
struct StatusChecks {
    #[serde(default)]
    contexts: Vec<String>,
    #[serde(default)]
    checks: Vec<StatusCheck>,
}

#[derive(Deserialize)]
struct StatusCheck {
    context: String,
}

#[derive(Deserialize)]
struct ClassicStatusChecks {
    #[serde(default)]
    strict: bool,
    #[serde(default)]
    contexts: Vec<String>,
    #[serde(default)]
    checks: Vec<StatusCheck>,
}

#[derive(Default, Deserialize)]
struct ClassicProtection {
    #[serde(default)]
    required_pull_request_reviews: Option<Value>,
    #[serde(default)]
    allow_force_pushes: Option<EnabledSetting>,
    #[serde(default)]
    allow_deletions: Option<EnabledSetting>,
    #[serde(default)]
    required_status_checks: Option<ClassicStatusChecks>,
}

#[derive(Default, Deserialize)]
struct EnabledSetting {
    #[serde(default)]
    enabled: bool,
}

#[derive(Deserialize)]
struct WorkflowList {
    total_count: u64,
    workflows: Vec<WorkflowResponse>,
}

#[derive(Deserialize)]
struct WorkflowResponse {
    id: u64,
    name: String,
    path: String,
    state: String,
}

#[derive(Deserialize)]
struct WorkflowRunList {
    workflow_runs: Vec<WorkflowRunResponse>,
}

#[derive(Deserialize)]
struct WorkflowRunResponse {
    id: u64,
    conclusion: Option<String>,
    html_url: String,
}

#[derive(Deserialize)]
struct PullRequestResponse {
    number: u64,
    updated_at: String,
}

#[derive(Deserialize)]
struct BranchSummary {
    name: String,
}

#[derive(Deserialize)]
struct CompareResponse {
    ahead_by: u64,
}

fn unix_days_now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86_400
}

fn parse_github_date(value: &str) -> Option<u64> {
    let date = value.get(..10)?;
    let mut components = date.split('-');
    let year = components.next()?.parse::<i64>().ok()?;
    let month = components.next()?.parse::<i64>().ok()?;
    let day = components.next()?.parse::<i64>().ok()?;
    if components.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let adjusted_year = year - i64::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let month_prime = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let unix_days = era * 146_097 + day_of_era - 719_468;
    u64::try_from(unix_days).ok()
}

fn security_facts(security: Option<SecurityResponse>) -> GithubValue<GithubSecurityFacts> {
    let Some(security) = security else {
        return GithubValue::unavailable("GitHub did not return security_and_analysis");
    };
    GithubValue::known(GithubSecurityFacts {
        secret_scanning: security
            .secret_scanning
            .is_some_and(|setting| setting.status == "enabled"),
        push_protection: security
            .secret_scanning_push_protection
            .is_some_and(|setting| setting.status == "enabled"),
    })
}

fn collect_required_rules(
    value: &Value,
    required_checks: &mut BTreeSet<String>,
    strict_values: &mut Vec<bool>,
    protection_values: &mut Vec<GithubBranchProtectionFacts>,
    found_rule: &mut bool,
) {
    match value {
        Value::Array(values) => {
            for value in values {
                collect_required_rules(
                    value,
                    required_checks,
                    strict_values,
                    protection_values,
                    found_rule,
                );
            }
        }
        Value::Object(object) => {
            let mut protection = GithubBranchProtectionFacts {
                pull_requests_required: false,
                force_pushes_blocked: false,
                deletion_blocked: false,
            };
            match object.get("type").and_then(Value::as_str) {
                Some("pull_request") => protection.pull_requests_required = true,
                Some("non_fast_forward") => protection.force_pushes_blocked = true,
                Some("deletion") => protection.deletion_blocked = true,
                Some("required_status_checks") => {
                    *found_rule = true;
                    if let Some(parameters) = object.get("parameters") {
                        if let Some(strict) = parameters
                            .get("strict_required_status_checks_policy")
                            .and_then(Value::as_bool)
                        {
                            strict_values.push(strict);
                        }
                        if let Some(checks) = parameters
                            .get("required_status_checks")
                            .and_then(Value::as_array)
                        {
                            required_checks.extend(checks.iter().filter_map(|check| {
                                check
                                    .get("context")
                                    .and_then(Value::as_str)
                                    .map(str::to_owned)
                            }));
                        }
                    }
                }
                _ => {}
            }
            if protection.pull_requests_required
                || protection.force_pushes_blocked
                || protection.deletion_blocked
            {
                protection_values.push(protection);
            }
            if let Some(rules) = object.get("rules") {
                collect_required_rules(
                    rules,
                    required_checks,
                    strict_values,
                    protection_values,
                    found_rule,
                );
            }
        }
        _ => {}
    }
}

fn validate_repository(repository: &str) -> Result<()> {
    let mut parts = repository.split('/');
    let valid = parts.next().is_some_and(valid_repository_part)
        && parts.next().is_some_and(valid_repository_part)
        && parts.next().is_none();
    if valid {
        Ok(())
    } else {
        bail!("repository {repository:?} must be owner/name")
    }
}

fn valid_repository_part(part: &str) -> bool {
    !part.is_empty()
        && part
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn encode_component(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

fn encode_path(value: &str) -> String {
    value
        .split('/')
        .map(encode_component)
        .collect::<Vec<_>>()
        .join("/")
}

fn github_path(path: &Path) -> Result<String> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            !matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
    {
        bail!(
            "remediation path {} must be a safe repository-relative path",
            path.display()
        );
    }
    Ok(path
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(part) => Some(part.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/"))
}
