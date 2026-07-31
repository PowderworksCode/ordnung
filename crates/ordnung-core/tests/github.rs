use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::LazyLock;

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
fn branch_protection_reports_each_missing_safeguard() {
    let mut facts = facts();
    facts.branch.protection = GithubValue::known(GithubBranchProtectionFacts {
        pull_requests_required: false,
        force_pushes_blocked: false,
        deletion_blocked: false,
    });

    let report = run_github_checks(&facts);
    let protection = report
        .results
        .iter()
        .find(|result| result.check == "branch-protection")
        .unwrap();
    assert_eq!(protection.status, CheckStatus::Fail);
    assert!(
        protection
            .message
            .contains("pull requests are not required")
    );
    assert!(protection.message.contains("force pushes are allowed"));
    assert!(protection.message.contains("branch deletion is allowed"));
}

#[test]
fn dependabot_reports_disabled_and_unavailable_security_settings() {
    let mut repository = facts();
    repository.vulnerability_alerts = GithubValue::known(false);
    repository.automated_security_fixes = GithubValue::unavailable("HTTP 403");

    let report = run_github_checks(&repository);
    let dependabot = report
        .results
        .iter()
        .find(|result| result.check == "dependabot")
        .unwrap();
    assert_eq!(dependabot.status, CheckStatus::Fail);
    assert!(dependabot.message.contains("vulnerability alerts"));
    assert!(dependabot.message.contains("HTTP 403"));

    repository.vulnerability_alerts = GithubValue::known(true);
    let report = run_github_checks(&repository);
    let dependabot = report
        .results
        .iter()
        .find(|result| result.check == "dependabot")
        .unwrap();
    assert_eq!(dependabot.status, CheckStatus::Skip);
}

#[test]
fn workflow_permissions_require_read_only_without_pr_approval() {
    let mut repository = facts();
    repository.actions_permissions = GithubValue::known(GithubActionsPermissionsFacts {
        default_workflow_permissions: GithubDefaultWorkflowPermissions::Write,
        can_approve_pull_request_reviews: true,
    });

    let report = run_github_checks(&repository);
    let permissions = report
        .results
        .iter()
        .find(|result| result.check == "workflow-permissions")
        .unwrap();
    assert_eq!(permissions.status, CheckStatus::Fail);
    assert!(permissions.message.contains("read-write"));
    assert!(permissions.message.contains("approve"));

    repository.actions_permissions = GithubValue::unavailable("HTTP 403");
    let report = run_github_checks(&repository);
    let permissions = report
        .results
        .iter()
        .find(|result| result.check == "workflow-permissions")
        .unwrap();
    assert_eq!(permissions.status, CheckStatus::Skip);
}

#[test]
fn unavailable_private_branch_protection_is_skipped() {
    let mut facts = facts();
    facts.visibility = "private".into();
    facts.branch.protection = GithubValue::unavailable("HTTP 403");

    let report = run_github_checks(&facts);
    let protection = report
        .results
        .iter()
        .find(|result| result.check == "branch-protection")
        .unwrap();
    assert_eq!(protection.status, CheckStatus::Skip);
}

#[test]
fn required_checks_reports_each_unprotected_pr_job() {
    let mut facts = facts();
    facts.pull_request_checks = GithubValue::known(vec!["Build".into(), "CI".into()]);

    let report = run_github_checks(&facts);
    let required = report
        .results
        .iter()
        .find(|result| result.check == "required-checks")
        .unwrap();
    assert_eq!(required.status, CheckStatus::Fail);
    assert_eq!(
        required.message,
        "pull-request checks not required on the default branch: Build"
    );
}

#[test]
fn required_checks_skips_when_no_pr_workflow_posts_checks() {
    let mut facts = facts();
    facts.pull_request_checks = GithubValue::known(Vec::new());

    let report = run_github_checks(&facts);
    let required = report
        .results
        .iter()
        .find(|result| result.check == "required-checks")
        .unwrap();
    assert_eq!(required.status, CheckStatus::Skip);
}

#[test]
fn unreadable_required_checks_skip_for_private_repositories() {
    let mut facts = facts();
    facts.visibility = "private".into();
    facts.branch.required_checks = GithubValue::unavailable("HTTP 403");

    let report = run_github_checks(&facts);
    let required = report
        .results
        .iter()
        .find(|result| result.check == "required-checks")
        .unwrap();
    assert_eq!(required.status, CheckStatus::Skip);
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
fn website_requires_and_probes_github_homepage_metadata() {
    let mut repository = facts();
    repository.homepage = None;
    let report = run_github_checks(&repository);
    let website = report
        .results
        .iter()
        .find(|result| result.check == "website")
        .unwrap();
    assert_eq!(website.status, CheckStatus::Fail);
    assert!(website.message.contains("homepage is not set"));

    repository.homepage = Some(format!("{}/missing", WEBSITE_SERVER.as_str()));
    let report = run_github_checks(&repository);
    let website = report
        .results
        .iter()
        .find(|result| result.check == "website")
        .unwrap();
    assert_eq!(website.status, CheckStatus::Fail);
    assert!(website.message.contains("HTTP 404"));

    let unavailable = TcpListener::bind("127.0.0.1:0").unwrap();
    repository.homepage = Some(format!("http://{}", unavailable.local_addr().unwrap()));
    drop(unavailable);
    let report = run_github_checks(&repository);
    let website = report
        .results
        .iter()
        .find(|result| result.check == "website")
        .unwrap();
    assert_eq!(website.status, CheckStatus::Error);
}

#[test]
fn license_reports_github_classification_without_requiring_it() {
    let mut repository = facts();
    let report = run_github_checks(&repository);
    let license = report
        .results
        .iter()
        .find(|result| result.check == "license")
        .unwrap();
    assert_eq!(license.status, CheckStatus::Pass);
    assert!(license.message.contains("MIT"));

    repository.license = Some(GithubLicenseFacts {
        key: "other".into(),
        name: "Other".into(),
        spdx_id: "NOASSERTION".into(),
    });
    let report = run_github_checks(&repository);
    let unclassified = report
        .results
        .iter()
        .find(|result| result.check == "license")
        .unwrap();
    assert_eq!(unclassified.status, CheckStatus::Skip);

    repository.license = None;
    let report = run_github_checks(&repository);
    let missing = report
        .results
        .iter()
        .find(|result| result.check == "license")
        .unwrap();
    assert_eq!(missing.status, CheckStatus::Skip);
}

#[test]
fn unavailable_strict_policy_is_an_error() {
    let mut facts = facts();
    facts.branch.strict_status_checks = GithubValue::unavailable("HTTP 403");
    let report = run_github_checks(&facts);
    assert!(!report.is_clean());
    assert!(report.results.iter().any(|result| {
        result.check == "strict-status-checks" && result.status == CheckStatus::Error
    }));
}

#[test]
fn unavailable_private_strict_policy_is_skipped() {
    let mut facts = facts();
    facts.visibility = "private".into();
    facts.branch.strict_status_checks = GithubValue::unavailable("HTTP 403");
    let report = run_github_checks(&facts);
    assert!(report.results.iter().any(|result| {
        result.check == "strict-status-checks" && result.status == CheckStatus::Skip
    }));
}

#[test]
fn strict_status_checks_distinguish_missing_required_checks() {
    let mut facts = facts();
    facts.allow_update_branch = false;
    facts.branch.required_checks = GithubValue::known(Vec::new());
    facts.branch.strict_status_checks = GithubValue::known(false);

    let report = run_github_checks(&facts);
    let strict = report
        .results
        .iter()
        .find(|result| result.check == "strict-status-checks")
        .unwrap();
    assert_eq!(strict.status, CheckStatus::Fail);
    assert!(strict.message.contains("no required status checks"));
    assert!(strict.message.contains("suggestions are also disabled"));
}

#[test]
fn strict_status_checks_recommend_update_branch_without_failing() {
    let mut facts = facts();
    facts.allow_update_branch = false;

    let report = run_github_checks(&facts);
    let strict = report
        .results
        .iter()
        .find(|result| result.check == "strict-status-checks")
        .unwrap();
    assert_eq!(strict.status, CheckStatus::Pass);
    assert!(strict.message.contains("enable update-branch suggestions"));
}

#[test]
fn dynamic_workflows_do_not_affect_ci_health() {
    let mut facts = facts();
    facts.workflows.push(GithubWorkflowFacts {
        id: 3,
        name: "Dependabot".into(),
        path: "dynamic/dependabot/dependabot-updates".into(),
        state: "active".into(),
        latest_run: Some(GithubWorkflowRun {
            id: 12,
            conclusion: Some("failure".into()),
            html_url: "https://example.test/run/12".into(),
        }),
        dependabot_automerge: Default::default(),
    });
    let report = run_github_checks(&facts);
    let ci = report
        .results
        .iter()
        .find(|result| result.check == "ci-green")
        .unwrap();
    assert_eq!(ci.status, CheckStatus::Pass);
}

#[test]
fn quiet_workflows_are_not_treated_as_red() {
    let mut facts = facts();
    facts.workflows.push(GithubWorkflowFacts {
        id: 3,
        name: "PR only".into(),
        path: ".github/workflows/pr.yml".into(),
        state: "active".into(),
        latest_run: None,
        dependabot_automerge: Default::default(),
    });

    let report = run_github_checks(&facts);
    let ci = report
        .results
        .iter()
        .find(|result| result.check == "ci-green")
        .unwrap();
    assert_eq!(ci.status, CheckStatus::Pass);
    assert!(
        ci.message
            .contains("no completed main runs yet for: PR only")
    );
}

#[test]
fn ci_green_excludes_self_audit_family_workflows() {
    let mut facts = facts();
    facts.workflows = vec![GithubWorkflowFacts {
        id: 3,
        name: "housekeeping".into(),
        path: ".github/workflows/housekeeping.yml".into(),
        state: "active".into(),
        latest_run: Some(GithubWorkflowRun {
            id: 13,
            conclusion: Some("failure".into()),
            html_url: "https://example.test/run/13".into(),
        }),
        dependabot_automerge: Default::default(),
    }];

    let report = run_github_checks(&facts);
    let ci = report
        .results
        .iter()
        .find(|result| result.check == "ci-green")
        .unwrap();
    assert_eq!(ci.status, CheckStatus::Skip);
    assert!(ci.message.contains("housekeeping"));
}

#[test]
fn ci_green_skips_when_every_workflow_is_quiet() {
    let mut facts = facts();
    facts
        .workflows
        .iter_mut()
        .for_each(|workflow| workflow.latest_run = None);

    let report = run_github_checks(&facts);
    let ci = report
        .results
        .iter()
        .find(|result| result.check == "ci-green")
        .unwrap();
    assert_eq!(ci.status, CheckStatus::Skip);
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

#[test]
fn allow_auto_merge_uses_the_effective_setting_policy() {
    let facts = facts();
    let disabled = GithubSettings {
        allow_auto_merge: Some(false),
        ..GithubSettings::default()
    };
    let report = run_github_checks_with_settings(&facts, &disabled);
    let check = report
        .results
        .iter()
        .find(|result| result.check == "allow-auto-merge")
        .unwrap();
    assert_eq!(check.status, CheckStatus::Fail);

    let enabled = GithubSettings {
        allow_auto_merge: Some(true),
        ..GithubSettings::default()
    };
    let report = run_github_checks_with_settings(&facts, &enabled);
    assert!(report.results.iter().any(|result| {
        result.check == "allow-auto-merge" && result.status == CheckStatus::Pass
    }));
}

#[test]
fn gating_rulesets_require_an_explicit_bypass_actor() {
    let mut facts = facts();
    facts.rulesets = GithubValue::known(vec![GithubRulesetFacts {
        id: 42,
        name: "main".into(),
        target: "branch".into(),
        enforcement: "active".into(),
        rule_types: ["pull_request".into()].into(),
        bypass_actors: Vec::new(),
    }]);
    let report = run_github_checks(&facts);
    let check = report
        .results
        .iter()
        .find(|result| result.check == "ruleset-bypass")
        .unwrap();
    assert_eq!(check.status, CheckStatus::Fail);
    assert!(check.message.contains("main"));

    let GithubValue::Known { value } = &mut facts.rulesets else {
        unreachable!();
    };
    value[0].bypass_actors.push(GithubRulesetBypassActor {
        actor_id: Some(5),
        actor_type: "RepositoryRole".into(),
        bypass_mode: "always".into(),
    });
    let report = run_github_checks(&facts);
    assert!(
        report.results.iter().any(|result| {
            result.check == "ruleset-bypass" && result.status == CheckStatus::Pass
        })
    );
}

#[test]
fn public_actions_link_their_exact_marketplace_listing() {
    let mut repository = facts();
    repository.action_publication = GithubValue::known(Some(GithubActionPublicationFacts {
        manifest_path: "action.yml".into(),
        name: "Setup Powderworks".into(),
        marketplace_slug: "setup-powderworks".into(),
        marketplace_url: "https://github.com/marketplace/actions/setup-powderworks".into(),
        readme_path: Some("README.md".into()),
        marketplace_linked: false,
    }));
    let report = run_github_checks(&repository);
    let badge = report
        .results
        .iter()
        .find(|result| result.check == "action-badge")
        .unwrap();
    assert_eq!(badge.status, CheckStatus::Fail);
    assert!(badge.message.contains("setup-powderworks"));

    let GithubValue::Known {
        value: Some(action),
    } = &mut repository.action_publication
    else {
        unreachable!();
    };
    action.marketplace_linked = true;
    let report = run_github_checks(&repository);
    assert!(
        report
            .results
            .iter()
            .any(|result| { result.check == "action-badge" && result.status == CheckStatus::Pass })
    );
}

#[test]
fn dependabot_automerge_requires_every_safety_gate() {
    let mut repository = facts();
    repository.workflows.push(GithubWorkflowFacts {
        id: 3,
        name: "Dependabot auto-merge".into(),
        path: ".github/workflows/dependabot-automerge.yml".into(),
        state: "active".into(),
        latest_run: None,
        dependabot_automerge: DependabotAutomergeWorkflowFacts {
            pull_request_trigger: true,
            dependabot_only: true,
            fetches_metadata: true,
            excludes_major_updates: false,
            enables_auto_merge: true,
        },
    });
    let settings = GithubSettings {
        allow_auto_merge: Some(true),
        ..GithubSettings::default()
    };
    let report = run_github_checks_with_settings(&repository, &settings);
    let automerge = report
        .results
        .iter()
        .find(|result| result.check == "dependabot-automerge")
        .unwrap();
    assert_eq!(automerge.status, CheckStatus::Fail);
    assert!(automerge.message.contains("major-update exclusion"));

    repository
        .workflows
        .last_mut()
        .unwrap()
        .dependabot_automerge
        .excludes_major_updates = true;
    let report = run_github_checks_with_settings(&repository, &settings);
    assert!(report.results.iter().any(|result| {
        result.check == "dependabot-automerge" && result.status == CheckStatus::Pass
    }));
}

#[test]
fn stale_reports_idle_pulls_merged_branches_and_cleanup_setting() {
    let mut repository = facts();
    repository.delete_branch_on_merge = false;
    repository.stale = GithubValue::known(GithubStaleFacts {
        open_pull_requests: vec![GithubPullRequestAgeFacts {
            number: 17,
            updated_at: "2020-01-01T00:00:00Z".into(),
            idle_days: 45,
        }],
        merged_branches: vec!["finished".into()],
        examined_branches: 20,
        non_default_branches: 25,
        ..GithubStaleFacts::default()
    });
    let report = run_github_checks(&repository);
    let stale = report
        .results
        .iter()
        .find(|result| result.check == "stale")
        .unwrap();
    assert_eq!(stale.status, CheckStatus::Fail);
    assert!(stale.message.contains("#17 (45d)"));
    assert!(stale.message.contains("finished"));
    assert!(stale.message.contains("automatic branch deletion"));
    assert!(stale.message.contains("20 of 25"));
}
