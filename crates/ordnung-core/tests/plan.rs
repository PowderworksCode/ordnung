use std::path::PathBuf;

use ordnung_core::{
    CheckRemediation, CheckResult, CheckStatus, FileOperation, Report, Severity,
    apply_file_changes, build_remediation_plan,
};

fn finding(check: &str, remediation: CheckRemediation) -> CheckResult {
    CheckResult {
        check: check.into(),
        status: CheckStatus::Fail,
        severity: Severity::Required,
        scope: remediation.path.clone(),
        message: "drift".into(),
        remediation: Some(remediation),
    }
}

#[test]
fn check_remediations_are_deterministic_and_hide_payloads_in_json() {
    let report = Report {
        repository: PathBuf::from("."),
        results: vec![finding(
            "field-guide",
            CheckRemediation::create(
                "notes/field_guide.md",
                b"# Guide\n".to_vec(),
                "create guide",
            ),
        )],
    };
    let plan = build_remediation_plan("owner/repo", &[report], &[], Vec::new()).unwrap();

    assert_eq!(plan.file_changes.len(), 1);
    assert_eq!(plan.file_changes[0].operation, FileOperation::Create);
    assert_eq!(
        plan.file_changes[0].content(),
        Some(b"# Guide\n".as_slice())
    );
    let json = serde_json::to_string(&plan).unwrap();
    assert!(!json.contains("# Guide"));
    assert!(json.contains("field-guide"));
}

#[test]
fn conflicting_check_remediations_are_rejected() {
    let report = Report {
        repository: PathBuf::from("."),
        results: vec![
            finding(
                "first",
                CheckRemediation::create("shared.txt", b"first".to_vec(), "first"),
            ),
            finding(
                "second",
                CheckRemediation::create("shared.txt", b"second".to_vec(), "second"),
            ),
        ],
    };

    let error = build_remediation_plan("owner/repo", &[report], &[], Vec::new()).unwrap_err();
    assert!(error.to_string().contains("conflicting changes"));
}

#[test]
fn overlapping_parent_and_child_remediations_are_rejected() {
    let report = Report {
        repository: PathBuf::from("."),
        results: vec![
            finding(
                "first",
                CheckRemediation::delete("notes", "remove old notes"),
            ),
            finding(
                "second",
                CheckRemediation::create("notes/guide.md", b"guide".to_vec(), "create guide"),
            ),
        ],
    };

    let error = build_remediation_plan("owner/repo", &[report], &[], Vec::new()).unwrap_err();
    assert!(error.to_string().contains("overlapping changes"));
}

#[test]
fn planned_file_changes_apply_idempotently() {
    let repository = tempfile::tempdir().unwrap();
    let report = Report {
        repository: repository.path().into(),
        results: vec![finding(
            "changelog",
            CheckRemediation::create(
                "CHANGELOG.md",
                b"# Changelog\n".to_vec(),
                "create changelog",
            ),
        )],
    };
    let plan = build_remediation_plan("owner/repo", &[report], &[], Vec::new()).unwrap();

    apply_file_changes(repository.path(), &plan.file_changes).unwrap();
    apply_file_changes(repository.path(), &plan.file_changes).unwrap();
    assert_eq!(
        std::fs::read(repository.path().join("CHANGELOG.md")).unwrap(),
        b"# Changelog\n"
    );
}
