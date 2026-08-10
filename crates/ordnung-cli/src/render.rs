//! What Ordnung prints, and what it leaves out.
//!
//! The name helpers are the wire spelling of each enum, shared by human output,
//! the JSON envelope, and the remediation pull request body. Changing one of
//! those strings changes Ordnung's output contract, so they live in one place
//! rather than being spelled inline at each call site.

use std::path::Path;

use ordnung_core::{CheckStatus, FileOperation, Report, Severity};

pub fn file_operation_name(operation: FileOperation) -> &'static str {
    match operation {
        FileOperation::Create => "create",
        FileOperation::Update => "update",
        FileOperation::Delete => "delete",
    }
}

/// The repository root prints as `.` rather than as an empty column.
pub fn display_scope(path: &Path) -> String {
    if path.as_os_str().is_empty() {
        ".".into()
    } else {
        path.display().to_string()
    }
}

pub fn status_name(status: CheckStatus) -> &'static str {
    match status {
        CheckStatus::Pass => "pass",
        CheckStatus::Fail => "fail",
        CheckStatus::Skip => "skip",
        CheckStatus::Error => "error",
    }
}

pub fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Required => "required",
        Severity::Recommended => "recommended",
        Severity::Off => "off",
    }
}

/// Drops `off`-severity findings unless the caller asked for everything.
///
/// A check at severity `off` still runs — severity is resolved from policy after
/// the fact — but it is an opinion the effective policy has switched off.
/// Printing it as `fail` beside a real failure is what made a first run read as
/// dozens of problems when there were a handful.
///
/// Exit codes are unaffected: only `required` findings gate those, and an `off`
/// finding is never `required`.
pub fn retain_reported(report: &mut Report, all: bool) {
    if !all {
        report
            .results
            .retain(|result| result.severity != Severity::Off);
    }
}

/// How many findings `retain_reported` would drop.
pub fn disabled_count(report: &Report) -> usize {
    report
        .results
        .iter()
        .filter(|result| result.severity == Severity::Off)
        .count()
}
