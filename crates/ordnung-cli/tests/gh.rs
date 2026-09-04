use std::collections::VecDeque;
use std::ffi::OsString;
use std::sync::{Arc, Mutex};

use ordnung_cli::gh::{GhClient, GhOutput, GhRunner, PullRequestStatus, REMEDIATION_BRANCH};
use ordnung_core::{
    CheckRemediation, CheckResult, CheckStatus, GithubSetting, GithubSettingChange, Report,
    Severity, build_remediation_plan,
};

#[derive(Clone)]
struct FakeRunner {
    outputs: Arc<Mutex<VecDeque<GhOutput>>>,
    calls: Arc<Mutex<Vec<Vec<String>>>>,
    inputs: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl FakeRunner {
    fn new(outputs: Vec<GhOutput>) -> Self {
        Self {
            outputs: Arc::new(Mutex::new(outputs.into())),
            calls: Arc::new(Mutex::new(Vec::new())),
            inputs: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl GhRunner for FakeRunner {
    fn run(&self, args: &[OsString]) -> std::io::Result<GhOutput> {
        self.calls.lock().unwrap().push(
            args.iter()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect(),
        );
        Ok(self.outputs.lock().unwrap().pop_front().unwrap())
    }

    fn run_with_input(&self, args: &[OsString], input: &[u8]) -> std::io::Result<GhOutput> {
        self.calls.lock().unwrap().push(
            args.iter()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect(),
        );
        self.inputs.lock().unwrap().push(input.to_vec());
        Ok(self.outputs.lock().unwrap().pop_front().unwrap())
    }
}

fn success(json: &str) -> GhOutput {
    GhOutput {
        success: true,
        stdout: json.as_bytes().to_vec(),
        stderr: Vec::new(),
    }
}

fn failure(message: &str) -> GhOutput {
    GhOutput {
        success: false,
        stdout: Vec::new(),
        stderr: message.as_bytes().to_vec(),
    }
}

fn planned_file_change() -> ordnung_core::PlannedFileChange {
    let report = Report {
        repository: ".".into(),
        results: vec![CheckResult {
            check: "field-guide".into(),
            status: CheckStatus::Fail,
            severity: Severity::Required,
            scope: "notes/field_guide.md".into(),
            message: "missing".into(),
            remediation: Some(CheckRemediation::create(
                "notes/field_guide.md",
                b"# Guide\n".to_vec(),
                "create guide",
            )),
        }],
    };
    build_remediation_plan("owner/repo", &[report], &[], Vec::new())
        .unwrap()
        .file_changes
        .into_iter()
        .next()
        .unwrap()
}

#[test]
fn creates_one_remediation_branch_and_pull_request() {
    let runner = FakeRunner::new(vec![
        success(r#"{"object":{"sha":"base-commit"}}"#),
        success(r#"{"tree":{"sha":"base-tree"}}"#),
        failure("gh: Not Found (HTTP 404)"),
        success(r#"{"sha":"blob-sha"}"#),
        success(r#"{"sha":"tree-sha"}"#),
        success(r#"{"sha":"commit-sha"}"#),
        success("{}"),
        success("[]"),
        success(
            r#"{"number":7,"html_url":"https://example.test/pr/7","title":"Ordnung","body":"Body"}"#,
        ),
    ]);
    let calls = Arc::clone(&runner.calls);
    let inputs = Arc::clone(&runner.inputs);
    let client = GhClient::with_runner(runner);

    let pull = client
        .materialize_pull_request(
            "owner/repo",
            "main",
            &[planned_file_change()],
            "Ordnung",
            "Body",
        )
        .unwrap()
        .unwrap();

    assert_eq!(pull.status, PullRequestStatus::Created);
    assert_eq!(pull.branch, REMEDIATION_BRANCH);
    assert_eq!(pull.number, 7);
    assert!(calls.lock().unwrap().iter().any(|call| {
        call.iter()
            .any(|argument| argument == "repos/owner/repo/git/refs")
    }));
    let inputs = inputs.lock().unwrap();
    assert!(
        inputs.iter().any(|input| {
            String::from_utf8_lossy(input).contains("\"content\":\"IyBHdWlkZQo=\"")
        })
    );
}

#[test]
fn reuses_pull_request_when_the_planned_tree_is_unchanged() {
    let runner = FakeRunner::new(vec![
        success(r#"{"object":{"sha":"base-commit"}}"#),
        success(r#"{"tree":{"sha":"base-tree"}}"#),
        success(r#"{"object":{"sha":"existing-commit"}}"#),
        success(r#"{"tree":{"sha":"tree-sha"}}"#),
        success(r#"{"sha":"blob-sha"}"#),
        success(r#"{"sha":"tree-sha"}"#),
        success(
            r#"[{"number":7,"html_url":"https://example.test/pr/7","title":"Ordnung","body":"Body"}]"#,
        ),
    ]);
    let calls = Arc::clone(&runner.calls);
    let client = GhClient::with_runner(runner);

    let pull = client
        .materialize_pull_request(
            "owner/repo",
            "main",
            &[planned_file_change()],
            "Ordnung",
            "Body",
        )
        .unwrap()
        .unwrap();

    assert_eq!(pull.status, PullRequestStatus::Unchanged);
    assert_eq!(pull.commit, "existing-commit");
    assert!(!calls.lock().unwrap().iter().any(|call| {
        call.iter()
            .any(|argument| argument == "repos/owner/repo/git/commits")
    }));
}

#[test]
fn fetches_canonical_repo_and_latest_workflow_run() {
    let runner = FakeRunner::new(vec![
        success(
            r#"{
                "full_name":"new-owner/repo","default_branch":"release/next","visibility":"public",
                "archived":false,"description":"Repo","homepage":null,
                "license":{"key":"mit","name":"MIT License","spdx_id":"MIT"},"topics":[],
                "has_issues":true,"allow_auto_merge":false,"delete_branch_on_merge":true,
                "allow_update_branch":false,
                "security_and_analysis":{"secret_scanning":{"status":"enabled"},
                "secret_scanning_push_protection":{"status":"enabled"}}
            }"#,
        ),
        success(
            r#"{"protected":false,"protection":{"required_status_checks":{"contexts":[],"checks":[]}}}"#,
        ),
        success(
            r#"{"total_count":2,"workflows":[
                {"id":1,"name":"CI","path":".github/workflows/ci.yml","state":"active"},
                {"id":2,"name":"Dependabot","path":"dynamic/dependabot/dependabot-updates","state":"active"}
            ]}"#,
        ),
        success("on: {pull_request: {}}\njobs: {test: {name: CI, steps: []}}\n"),
        success(
            r#"{"workflow_runs":[{"id":10,"conclusion":"success","html_url":"https://example.test/10"}]}"#,
        ),
        success(""),
        success(r#"{"enabled":true}"#),
        success(
            r#"{"default_workflow_permissions":"read","can_approve_pull_request_reviews":false}"#,
        ),
        success("[]"),
        success("name: Setup Powderworks\n"),
        success("[Install](https://github.com/marketplace/actions/setup-powderworks)\n"),
        success(r#"[{"number":7,"updated_at":"2020-01-01T00:00:00Z"}]"#),
        success(r#"[{"name":"release/next"},{"name":"merged"}]"#),
        success(r#"{"ahead_by":0}"#),
    ]);
    let calls = Arc::clone(&runner.calls);
    let client = GhClient::with_runner(runner);

    let facts = client.fetch_repository("old-owner/repo").unwrap();
    assert_eq!(facts.repository, "new-owner/repo");
    assert_eq!(facts.license.as_ref().unwrap().spdx_id, "MIT");
    assert_eq!(facts.workflows.len(), 2);
    assert_eq!(
        facts.pull_request_checks,
        ordnung_core::GithubValue::known(vec!["CI".into()])
    );
    assert_eq!(facts.workflows[0].latest_run.as_ref().unwrap().id, 10);
    assert!(facts.workflows[1].latest_run.is_none());
    assert_eq!(
        facts.vulnerability_alerts,
        ordnung_core::GithubValue::known(true)
    );
    assert_eq!(
        facts.automated_security_fixes,
        ordnung_core::GithubValue::known(true)
    );
    assert!(matches!(
        facts.actions_permissions,
        ordnung_core::GithubValue::Known { .. }
    ));
    let action = match facts.action_publication {
        ordnung_core::GithubValue::Known { value: Some(value) } => value,
        other => panic!("unexpected action publication facts: {other:?}"),
    };
    assert!(action.marketplace_linked);
    let stale = match facts.stale {
        ordnung_core::GithubValue::Known { value } => value,
        ordnung_core::GithubValue::Unavailable { reason } => panic!("{reason}"),
    };
    assert_eq!(stale.open_pull_requests[0].number, 7);
    assert_eq!(stale.merged_branches, ["merged"]);
    let calls = calls.lock().unwrap();
    assert!(calls[1].last().unwrap().contains("release%2Fnext"));
    assert!(calls[3].last().unwrap().contains("ref=release%2Fnext"));
    assert!(calls[4].last().unwrap().contains("branch=release%2Fnext"));
    assert!(calls[5].last().unwrap().ends_with("vulnerability-alerts"));
    assert!(
        calls[6]
            .last()
            .unwrap()
            .ends_with("automated-security-fixes")
    );
    assert!(
        calls[7]
            .last()
            .unwrap()
            .ends_with("actions/permissions/workflow")
    );
}

#[test]
fn combines_rulesets_and_classic_branch_protection() {
    let runner = FakeRunner::new(vec![
        success(
            r#"{
                "full_name":"owner/repo","default_branch":"main","visibility":"public",
                "archived":false,"description":"Repo","homepage":null,"topics":[],
                "has_issues":true,"allow_auto_merge":false,"delete_branch_on_merge":true,
                "allow_update_branch":false
            }"#,
        ),
        success(
            r#"{"protected":true,"protection":{"required_status_checks":{"contexts":[],"checks":[]}}}"#,
        ),
        success(
            r#"[
                {"type":"pull_request"},
                {"type":"non_fast_forward"},
                {"type":"required_status_checks","parameters":{
                    "strict_required_status_checks_policy":true,
                    "required_status_checks":[{"context":"CI"}]
                }}
            ]"#,
        ),
        success(
            r#"{
                "allow_force_pushes":{"enabled":true},
                "allow_deletions":{"enabled":false},
                "required_status_checks":{"strict":false,"contexts":["Lint"],"checks":[]}
            }"#,
        ),
        success(r#"{"total_count":0,"workflows":[]}"#),
        success(""),
        success(r#"{"enabled":true}"#),
        success(
            r#"{"default_workflow_permissions":"read","can_approve_pull_request_reviews":false}"#,
        ),
        success(r#"[{"id":42,"name":"main","target":"branch","enforcement":"active"}]"#),
        success(
            r#"{
                "id":42,"name":"main","target":"branch","enforcement":"active",
                "rules":[{"type":"pull_request"}],
                "bypass_actors":[{"actor_id":5,"actor_type":"RepositoryRole","bypass_mode":"always"}]
            }"#,
        ),
        failure("HTTP 404"),
        failure("HTTP 404"),
        success("[]"),
        success("[]"),
    ]);
    let calls = Arc::clone(&runner.calls);
    let client = GhClient::with_runner(runner);

    let facts = client.fetch_repository("owner/repo").unwrap();
    let protection = match facts.branch.protection {
        ordnung_core::GithubValue::Known { value } => value,
        ordnung_core::GithubValue::Unavailable { reason } => panic!("{reason}"),
    };
    assert!(protection.pull_requests_required);
    assert!(protection.force_pushes_blocked);
    assert!(protection.deletion_blocked);
    assert_eq!(
        facts.branch.required_checks,
        ordnung_core::GithubValue::known(vec!["CI".into(), "Lint".into()])
    );
    assert_eq!(
        facts.branch.strict_status_checks,
        ordnung_core::GithubValue::known(true)
    );
    let rulesets = match facts.rulesets {
        ordnung_core::GithubValue::Known { value } => value,
        ordnung_core::GithubValue::Unavailable { reason } => panic!("{reason}"),
    };
    assert!(rulesets[0].is_active_gating_branch_ruleset());
    assert_eq!(rulesets[0].bypass_actors[0].actor_type, "RepositoryRole");
    assert_eq!(
        calls.lock().unwrap()[3].last().unwrap(),
        "repos/owner/repo/branches/main/protection"
    );
}

#[test]
fn preserves_unavailable_protection_facts() {
    let runner = FakeRunner::new(vec![
        success(
            r#"{
                "full_name":"owner/repo","default_branch":"main","visibility":"private",
                "archived":false,"description":"Repo","homepage":null,"topics":[],
                "has_issues":true
            }"#,
        ),
        success(
            r#"{"protected":true,"protection":{"required_status_checks":{"contexts":[],"checks":[]}}}"#,
        ),
        failure("HTTP 403"),
        failure("HTTP 403"),
        success(r#"{"total_count":0,"workflows":[]}"#),
        failure("HTTP 403"),
        failure("HTTP 403"),
        failure("HTTP 403"),
        failure("HTTP 403"),
        failure("HTTP 403"),
        failure("HTTP 403"),
    ]);
    let client = GhClient::with_runner(runner);

    let facts = client.fetch_repository("owner/repo").unwrap();
    assert!(matches!(
        facts.branch.protection,
        ordnung_core::GithubValue::Unavailable { .. }
    ));
    assert!(matches!(
        facts.branch.required_checks,
        ordnung_core::GithubValue::Unavailable { .. }
    ));
    assert!(matches!(
        facts.vulnerability_alerts,
        ordnung_core::GithubValue::Unavailable { .. }
    ));
    assert!(matches!(
        facts.automated_security_fixes,
        ordnung_core::GithubValue::Unavailable { .. }
    ));
    assert!(matches!(
        facts.actions_permissions,
        ordnung_core::GithubValue::Unavailable { .. }
    ));
    assert!(matches!(
        facts.rulesets,
        ordnung_core::GithubValue::Unavailable { .. }
    ));
}

#[test]
fn invalid_repository_names_are_rejected() {
    let client = GhClient::with_runner(FakeRunner::new(Vec::new()));
    assert!(client.fetch_repository("owner/repo/extra").is_err());
}

#[test]
fn applies_settings_with_a_typed_patch_request() {
    let runner = FakeRunner::new(vec![success("{}")]);
    let calls = Arc::clone(&runner.calls);
    let client = GhClient::with_runner(runner);
    client
        .apply_setting_changes(
            "owner/repo",
            &[GithubSettingChange {
                setting: GithubSetting::AllowUpdateBranch,
                current: false,
                desired: true,
            }],
        )
        .unwrap();

    let calls = calls.lock().unwrap();
    let args = &calls[0];
    assert!(args.windows(2).any(|pair| pair == ["--method", "PATCH"]));
    assert!(
        args.windows(2)
            .any(|pair| pair == ["--field", "allow_update_branch=true"])
    );
}
