//! Stable names for the values Ordnung prints.
//!
//! These are the wire spelling of each enum, shared by human output, the JSON
//! envelope, and the remediation pull request body. Changing one of these
//! strings changes Ordnung's output contract, so they live in one place rather
//! than being spelled inline at each call site.

use std::path::Path;

use ordnung_core::{CheckStatus, FileOperation, Severity};

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
