// Tests for `src/github.rs`: the shape of the facts themselves and the
// settings plan over them. A test about one check lives beside that check,
// under tests/checks/.
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::LazyLock;

use ordnung_core::Severity;
use ordnung_core::{
    CheckStatus, DependabotAutomergeWorkflowFacts, GithubActionPublicationFacts,
    GithubActionsPermissionsFacts, GithubBranchFacts, GithubBranchProtectionFacts,
    GithubDefaultWorkflowPermissions, GithubLicenseFacts, GithubPullRequestAgeFacts,
    GithubRepositoryFacts, GithubRulesetBypassActor, GithubRulesetFacts, GithubSecurityFacts,
    GithubSetting, GithubSettings, GithubStaleFacts, GithubValue, GithubWorkflowFacts,
    GithubWorkflowRun, plan_github_settings, run_github_checks, run_github_checks_with_settings,
};

static WEBSITE_SERVER: LazyLock<String> = LazyLock::new(|| {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut request = [0; 2048];
            let read = stream.read(&mut request).unwrap();
            let path = String::from_utf8_lossy(&request[..read])
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .to_owned();
            let (status, body) = if path == "/missing" {
                ("404 Not Found", "missing")
            } else {
                ("200 OK", "ok")
            };
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });
    format!("http://{address}")
});

fn facts() -> GithubRepositoryFacts {
    GithubRepositoryFacts {
        repository: "owner/repo".into(),
        default_branch: "main".into(),
        visibility: "public".into(),
        archived: false,
        description: Some("A repository".into()),
        homepage: Some(format!("{}/ok", WEBSITE_SERVER.as_str())),
        license: Some(GithubLicenseFacts {
            key: "mit".into(),
            name: "MIT License".into(),
            spdx_id: "MIT".into(),
        }),
        topics: vec!["rust".into()],
        has_issues: true,
        allow_auto_merge: true,
        delete_branch_on_merge: true,
        allow_update_branch: true,
        branch: GithubBranchFacts {
            protected: true,
            protection: GithubValue::known(GithubBranchProtectionFacts {
                pull_requests_required: true,
                force_pushes_blocked: true,
                deletion_blocked: true,
            }),
            required_checks: GithubValue::known(vec!["CI".into()]),
            strict_status_checks: GithubValue::known(true),
        },
        security: GithubValue::known(GithubSecurityFacts {
            secret_scanning: true,
            push_protection: true,
        }),
        vulnerability_alerts: GithubValue::known(true),
        automated_security_fixes: GithubValue::known(true),
        actions_permissions: GithubValue::known(GithubActionsPermissionsFacts {
            default_workflow_permissions: GithubDefaultWorkflowPermissions::Read,
            can_approve_pull_request_reviews: false,
        }),
        rulesets: GithubValue::known(Vec::new()),
        pull_request_checks: GithubValue::known(vec!["CI".into()]),
        workflows: vec![
            GithubWorkflowFacts {
                id: 1,
                name: "CI".into(),
                path: ".github/workflows/ci.yml".into(),
                state: "active".into(),
                latest_run: Some(GithubWorkflowRun {
                    id: 10,
                    conclusion: Some("success".into()),
                    html_url: "https://example.test/run/10".into(),
                }),
                dependabot_automerge: Default::default(),
            },
            GithubWorkflowFacts {
                id: 2,
                name: "Update branches".into(),
                path: ".github/workflows/auto-update-pr-branches.yml".into(),
                state: "active".into(),
                latest_run: Some(GithubWorkflowRun {
                    id: 11,
                    conclusion: Some("success".into()),
                    html_url: "https://example.test/run/11".into(),
                }),
                dependabot_automerge: Default::default(),
            },
        ],
        action_publication: GithubValue::known(None),
        stale: GithubValue::known(GithubStaleFacts::default()),
    }
}

#[test]
fn healthy_github_facts_pass() {
    let report = run_github_checks(&facts());
    assert!(report.is_clean());
    assert!(
        report
            .results
            .iter()
            .all(|result| { matches!(result.status, CheckStatus::Pass | CheckStatus::Skip) })
    );
}

#[test]
fn github_setting_plan_contains_only_drift() {
    let facts = facts();
    let desired = GithubSettings {
        allow_auto_merge: Some(true),
        delete_branch_on_merge: Some(false),
        allow_update_branch: None,
    };
    let changes = plan_github_settings(&facts, &desired);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].setting, GithubSetting::DeleteBranchOnMerge);
    assert!(changes[0].current);
    assert!(!changes[0].desired);
}

/// GitHub refuses writes to an archived repository and Ordnung refuses to open a
/// pull request against one, so every finding would be unactionable. The state is
/// reported once per check instead of as a wall of failures nobody can clear.
#[test]
fn an_archived_repository_reports_the_state_instead_of_findings() {
    let mut facts = facts();
    // Deliberately broken settings that would otherwise fail loudly.
    facts.archived = true;
    facts.branch.strict_status_checks = GithubValue::known(false);

    let report = run_github_checks(&facts);
    assert!(
        report.is_clean(),
        "an archived repository cannot be unclean"
    );
    assert!(
        !report.results.is_empty(),
        "the archived state should still be visible"
    );
    assert!(
        report
            .results
            .iter()
            .all(|result| result.status == CheckStatus::Skip
                && result.message.contains("archived")),
        "every result should skip with the archived reason"
    );
    // One result per GitHub-backed check, not per finding.
    let github_checks = ordnung_core::check_definitions()
        .iter()
        .filter(|definition| definition.github_runner.is_some())
        .count();
    assert_eq!(report.results.len(), github_checks);
}
