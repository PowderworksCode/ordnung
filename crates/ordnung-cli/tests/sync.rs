//! Fleet sync orchestration, driven end to end by a fake `gh` runner.
//!
//! Before `sync` was extracted from `main.rs` this path was only reachable by
//! running the binary against a real repository, so the decision logic joining
//! the adapter to the planner had no coverage at all. These tests exercise it
//! without a network, a `gh` binary, or a GitHub account.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ordnung_cli::gh::{GhClient, GhOutput, GhRunner};
use ordnung_cli::sync::{
    ensure_explicit_member, fleet_requirements, plan_local_sync, sync_fleet_member,
};
use ordnung_core::fleet::ManagedState;
use ordnung_core::{DependencyRequirement, FleetConfig, RepoConfig};

fn success(body: &str) -> GhOutput {
    GhOutput {
        success: true,
        stdout: body.as_bytes().to_vec(),
        stderr: Vec::new(),
    }
}

/// A `gh` stand-in that answers by endpoint rather than by call order, and
/// materializes a checkout on disk when asked to clone.
///
/// Routing beats a positional response queue here: `fetch_repository` issues a
/// different number of calls depending on what it finds, so an ordered fixture
/// breaks whenever an unrelated fact changes.
#[derive(Clone)]
struct FakeRunner {
    archived: bool,
    /// Whether `ordnung/remediation` already exists on the remote.
    remediation_branch_exists: bool,
    calls: Arc<Mutex<Vec<Vec<String>>>>,
    /// Request bodies sent with mutating calls.
    inputs: Arc<Mutex<Vec<String>>>,
    /// Files written into the destination when `gh repo clone` is invoked.
    checkout: Arc<Vec<(PathBuf, String)>>,
}

impl FakeRunner {
    fn new(checkout: Vec<(PathBuf, String)>) -> Self {
        Self {
            archived: false,
            remediation_branch_exists: false,
            calls: Arc::new(Mutex::new(Vec::new())),
            inputs: Arc::new(Mutex::new(Vec::new())),
            checkout: Arc::new(checkout),
        }
    }

    fn archived(mut self) -> Self {
        self.archived = true;
        self
    }

    fn with_existing_remediation_branch(mut self) -> Self {
        self.remediation_branch_exists = true;
        self
    }

    fn materialize_clone(&self, destination: &Path) {
        for (relative, contents) in self.checkout.iter() {
            let path = destination.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, contents).unwrap();
        }
    }

    fn respond(&self, endpoint: &str) -> GhOutput {
        if endpoint.ends_with("repos/owner/member") {
            return success(&format!(
                r#"{{
                    "full_name":"owner/member","default_branch":"main","visibility":"public",
                    "archived":{},"description":"Member","homepage":null,
                    "license":{{"key":"mit","name":"MIT License","spdx_id":"MIT"}},"topics":[],
                    "has_issues":true,"allow_auto_merge":true,"delete_branch_on_merge":true,
                    "allow_update_branch":true,
                    "security_and_analysis":{{"secret_scanning":{{"status":"enabled"}},
                    "secret_scanning_push_protection":{{"status":"enabled"}}}}
                }}"#,
                self.archived
            ));
        }
        if endpoint.contains("/branches/main") {
            return success(
                r#"{"protected":true,"protection":{"required_status_checks":{"contexts":[],"checks":[]}}}"#,
            );
        }
        if endpoint.contains("actions/workflows") {
            return success(r#"{"total_count":0,"workflows":[]}"#);
        }
        if endpoint.ends_with("vulnerability-alerts") {
            return success("");
        }
        if endpoint.ends_with("automated-security-fixes") {
            return success(r#"{"enabled":true}"#);
        }
        if endpoint.contains("actions/permissions/workflow") {
            return success(
                r#"{"default_workflow_permissions":"read","can_approve_pull_request_reviews":false}"#,
            );
        }
        if endpoint.contains("rulesets") || endpoint.contains("/pulls") {
            return success("[]");
        }
        if endpoint.contains("/branches") {
            return success("[]");
        }
        if endpoint.contains("compare") {
            return success(r#"{"ahead_by":0}"#);
        }
        if endpoint.contains("git/ref/heads/ordnung%2Fremediation") {
            return if self.remediation_branch_exists {
                success(r#"{"object":{"sha":"stale000000000000000000000000000000000000"}}"#)
            } else {
                GhOutput {
                    success: false,
                    stdout: Vec::new(),
                    stderr: b"gh: HTTP 404".to_vec(),
                }
            };
        }
        if endpoint.contains("git/ref/heads/main") {
            return success(r#"{"object":{"sha":"base000000000000000000000000000000000000"}}"#);
        }
        if endpoint.contains("git/commits/") {
            return success(r#"{"tree":{"sha":"tree0000000000000000000000000000000000000"}}"#);
        }
        // Anything else is genuinely absent; optional lookups read 404 as "no fact".
        GhOutput {
            success: false,
            stdout: Vec::new(),
            stderr: b"gh: HTTP 404".to_vec(),
        }
    }

    fn recorded(&self) -> Vec<Vec<String>> {
        self.calls.lock().unwrap().clone()
    }

    fn bodies(&self) -> Vec<String> {
        self.inputs.lock().unwrap().clone()
    }
}

impl GhRunner for FakeRunner {
    fn run(&self, args: &[OsString]) -> std::io::Result<GhOutput> {
        let rendered = args
            .iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let response = if rendered.first().map(String::as_str) == Some("repo")
            && rendered.get(1).map(String::as_str) == Some("clone")
        {
            self.materialize_clone(Path::new(&rendered[3]));
            success("")
        } else {
            self.respond(rendered.last().map(String::as_str).unwrap_or_default())
        };
        self.calls.lock().unwrap().push(rendered);
        Ok(response)
    }

    /// Every write this path makes returns an object carrying a sha, and the
    /// pull-request creation additionally wants a number and a URL. One
    /// permissive shape satisfies all of them.
    fn run_with_input(&self, args: &[OsString], input: &[u8]) -> std::io::Result<GhOutput> {
        self.calls.lock().unwrap().push(
            args.iter()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect(),
        );
        self.inputs
            .lock()
            .unwrap()
            .push(String::from_utf8_lossy(input).into_owned());
        Ok(success(
            r#"{"sha":"written00000000000000000000000000000000",
                "tree":{"sha":"written00000000000000000000000000000000"},
                "number":1,"html_url":"https://example.test/pull/1",
                "title":"chore: apply Ordnung remediations"}"#,
        ))
    }
}

fn write_fleet(directory: &Path, body: &str) -> PathBuf {
    let path = directory.join("fleet.toml");
    fs::write(&path, body).unwrap();
    path
}

const ONE_MEMBER_FLEET: &str = r#"
name = "test-fleet"

[[member]]
repo = "owner/member"
stage = "incubating"
"#;

fn cloned(calls: &[Vec<String>]) -> bool {
    calls
        .iter()
        .any(|call| call.first().map(String::as_str) == Some("repo"))
}

fn mutating(calls: &[Vec<String>]) -> Vec<&Vec<String>> {
    calls
        .iter()
        .filter(|call| {
            call.iter()
                .any(|argument| matches!(argument.as_str(), "POST" | "PATCH" | "PUT" | "DELETE"))
        })
        .collect()
}

#[test]
fn a_non_member_repository_is_rejected_before_any_api_call() {
    let directory = tempfile::tempdir().unwrap();
    let fleet = FleetConfig::load(&write_fleet(directory.path(), ONE_MEMBER_FLEET)).unwrap();

    let error = ensure_explicit_member(&fleet, "owner/stranger").unwrap_err();
    let message = format!("{error}");
    assert!(message.contains("owner/stranger"), "{message}");
    assert!(message.contains("test-fleet"), "{message}");

    ensure_explicit_member(&fleet, "owner/member").unwrap();
}

#[test]
fn fleet_requirements_override_same_named_local_ones_and_keep_the_rest() {
    let directory = tempfile::tempdir().unwrap();
    let fleet = FleetConfig::load(&write_fleet(
        directory.path(),
        r#"
name = "test-fleet"

[[member]]
repo = "owner/member"
stage = "incubating"

[[dependency]]
name = "serde"
language = "rust"
require = ["1.0.200"]
"#,
    ))
    .unwrap();

    let requirement = |name: &str, version: &str| DependencyRequirement {
        name: name.into(),
        language: Some("rust".into()),
        ecosystem: None,
        require: vec![version.into()],
        kind: None,
        state: ManagedState::Present,
    };
    let local = RepoConfig {
        dependencies: vec![requirement("serde", "1.0.100"), requirement("clap", "4")],
        ..RepoConfig::default()
    };

    let merged = fleet_requirements(&local, &fleet);

    let serde = merged
        .iter()
        .find(|candidate| candidate.name == "serde")
        .expect("serde requirement survives");
    assert_eq!(
        serde.require,
        ["1.0.200"],
        "the fleet's version overrides the member's"
    );
    assert!(
        merged.iter().any(|candidate| candidate.name == "clap"),
        "a member may add requirements of its own"
    );
    assert_eq!(merged.len(), 2, "no duplicate entry for serde");
}

#[test]
fn a_dry_run_plans_without_issuing_a_single_mutating_request() {
    let directory = tempfile::tempdir().unwrap();
    let fleet = FleetConfig::load(&write_fleet(directory.path(), ONE_MEMBER_FLEET)).unwrap();

    let runner = FakeRunner::new(vec![
        (PathBuf::from("README.md"), "# member\n".into()),
        (PathBuf::from(".gitignore"), "target/\n".into()),
    ]);
    let client = GhClient::with_runner(runner.clone());

    let outcome = sync_fleet_member(&client, &fleet, "owner/member", false).unwrap();

    assert!(!outcome.applied, "a dry run does not apply");
    assert!(
        outcome.pull_request.is_none(),
        "a dry run opens no pull request"
    );
    assert!(
        !outcome.plan.findings.is_empty(),
        "the member is still audited"
    );

    let calls = runner.recorded();
    assert!(
        cloned(&calls),
        "the member is cloned so its working tree can be inventoried"
    );
    let mutations = mutating(&calls);
    assert!(
        mutations.is_empty(),
        "a dry run issues no mutating request, got: {mutations:?}"
    );
}

#[test]
fn applying_opens_a_pull_request_and_writes_settings() {
    let directory = tempfile::tempdir().unwrap();
    let fleet = FleetConfig::load(&write_fleet(directory.path(), ONE_MEMBER_FLEET)).unwrap();

    let runner = FakeRunner::new(vec![(PathBuf::from("README.md"), "# member\n".into())]);
    let client = GhClient::with_runner(runner.clone());

    let outcome = sync_fleet_member(&client, &fleet, "owner/member", true).unwrap();

    assert!(outcome.applied, "the outcome records that it applied");

    let calls = runner.recorded();
    assert!(
        !mutating(&calls).is_empty(),
        "applying issues mutating requests"
    );
    assert!(
        calls.iter().any(|call| call
            .iter()
            .any(|argument| argument.contains("ordnung%2Fremediation"))),
        "the remediation branch is the one written to"
    );
    assert!(
        outcome.pull_request.is_some(),
        "applying materializes a pull request"
    );

    assert!(
        runner
            .bodies()
            .iter()
            .any(|body| body.contains("refs/heads/ordnung/remediation")),
        "a fresh remediation branch is created"
    );
}

#[test]
fn an_archived_member_is_refused_before_it_is_cloned() {
    let directory = tempfile::tempdir().unwrap();
    let fleet = FleetConfig::load(&write_fleet(directory.path(), ONE_MEMBER_FLEET)).unwrap();

    let runner = FakeRunner::new(Vec::new()).archived();
    let client = GhClient::with_runner(runner.clone());

    let error = sync_fleet_member(&client, &fleet, "owner/member", true).unwrap_err();
    assert!(format!("{error}").contains("archived"), "{error}");

    let calls = runner.recorded();
    assert!(!cloned(&calls), "an archived member is never cloned");
    assert!(
        mutating(&calls).is_empty(),
        "an archived member is never written to"
    );
}

#[test]
fn local_sync_plans_managed_files_without_writing_them() {
    let fleet_directory = tempfile::tempdir().unwrap();
    let member = tempfile::tempdir().unwrap();
    fs::create_dir_all(fleet_directory.path().join("managed")).unwrap();
    fs::write(
        fleet_directory.path().join("managed/CONTRIBUTING.md"),
        "# Contributing\n",
    )
    .unwrap();
    let fleet = FleetConfig::load(&write_fleet(
        fleet_directory.path(),
        r#"
name = "test-fleet"

[[member]]
repo = "owner/member"
stage = "incubating"

[[managed]]
name = "contributing"
source = "managed/CONTRIBUTING.md"
destination = "CONTRIBUTING.md"
"#,
    ))
    .unwrap();
    fs::write(member.path().join("README.md"), "# member\n").unwrap();

    let plan = plan_local_sync(&fleet, "owner/member", member.path()).unwrap();

    assert!(
        plan.file_changes
            .iter()
            .any(|change| change.path == Path::new("CONTRIBUTING.md")),
        "the managed file is planned"
    );
    assert!(
        !member.path().join("CONTRIBUTING.md").exists(),
        "planning writes nothing"
    );
}

/// Pins the behaviour flagged in notes/flagged.md A3: when the remediation
/// branch already exists, Ordnung force-updates it rather than building on it,
/// so any commit already there is discarded.
#[test]
fn an_existing_remediation_branch_is_force_updated() {
    let directory = tempfile::tempdir().unwrap();
    let fleet = FleetConfig::load(&write_fleet(directory.path(), ONE_MEMBER_FLEET)).unwrap();

    let runner = FakeRunner::new(vec![(PathBuf::from("README.md"), "# member\n".into())])
        .with_existing_remediation_branch();
    let client = GhClient::with_runner(runner.clone());

    sync_fleet_member(&client, &fleet, "owner/member", true).unwrap();

    let forced = runner
        .bodies()
        .iter()
        .any(|body| body.contains("\"force\":true"));
    assert!(
        forced,
        "the existing branch is force-updated: {:?}",
        runner.bodies()
    );
    assert!(
        runner
            .recorded()
            .iter()
            .any(|call| call.contains(&"PATCH".to_string())),
        "the force update is a PATCH to the ref"
    );
}
