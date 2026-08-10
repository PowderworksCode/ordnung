//! What Ordnung prints, and what it leaves out.
//!
//! The name helpers are the wire spelling of each enum, shared by human output,
//! the JSON envelope, and the remediation pull request body. Changing one of
//! those strings changes Ordnung's output contract, so they live in one place
//! rather than being spelled inline at each call site.

use std::collections::BTreeMap;
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

/// The checks `ordnung check` cannot run, because they read GitHub state rather
/// than the working tree.
///
/// A clean `check` reads as "this repository is in order" when it means "the
/// local checks passed" — and several of the checks it cannot reach are
/// `required`. Saying so is the difference between a partial audit and a
/// misleading one.
pub struct UnreachedChecks {
    pub total: usize,
    pub required: usize,
}

/// Counts against the resolved policy, not the defaults, so a repository that
/// has switched one of these off is not told it is missing.
pub fn unreached_github_checks(policy: &BTreeMap<String, Severity>) -> UnreachedChecks {
    let reachable_severities = ordnung_core::check_definitions()
        .iter()
        .filter(|definition| {
            definition.github_runner.is_some() && definition.repository_runner.is_none()
        })
        .map(|definition| {
            policy
                .get(definition.id)
                .copied()
                .unwrap_or(definition.default_severity)
        })
        .filter(|severity| *severity != Severity::Off)
        .collect::<Vec<_>>();
    UnreachedChecks {
        required: reachable_severities
            .iter()
            .filter(|severity| **severity == Severity::Required)
            .count(),
        total: reachable_severities.len(),
    }
}

impl UnreachedChecks {
    /// The note printed after a local-only run, or nothing when policy has
    /// switched every GitHub-backed check off.
    pub fn note(&self, path: &str) -> Option<String> {
        if self.total == 0 {
            return None;
        }
        let checks = if self.total == 1 { "check" } else { "checks" };
        let required = match self.required {
            0 => String::new(),
            count => format!(", {count} of them required"),
        };
        Some(format!(
            "note: {} GitHub-backed {checks} did not run{required}. \
             Run `ordnung repo-check {path} --repo owner/name` for the full audit.",
            self.total
        ))
    }
}
