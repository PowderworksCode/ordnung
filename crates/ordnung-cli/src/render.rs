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

/// The lowest severity worth printing.
///
/// `off` findings still run — severity is resolved from policy after the fact —
/// but printing them as `fail` beside a real failure is what made a first run
/// read as dozens of problems when there were a handful. The floor defaults to
/// `recommended`, which excludes them.
///
/// Exit codes are unaffected by the floor: only `required` findings gate those.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeverityFloor {
    Required,
    Recommended,
    All,
}

impl SeverityFloor {
    fn admits(self, severity: Severity) -> bool {
        match self {
            Self::All => true,
            Self::Recommended => severity != Severity::Off,
            Self::Required => severity == Severity::Required,
        }
    }
}

pub fn retain_reported(report: &mut Report, floor: SeverityFloor) {
    report
        .results
        .retain(|result| floor.admits(result.severity));
}

/// How many findings a given floor would drop.
pub fn hidden_count(report: &Report, floor: SeverityFloor) -> usize {
    report
        .results
        .iter()
        .filter(|result| !floor.admits(result.severity))
        .count()
}

/// The closing line of a human-readable run: what was checked, what it found,
/// and why the exit code is what it is.
///
/// Without this the output simply stopped, and answering "did this pass?" meant
/// scrolling back and reading every line.
pub struct Summary {
    pub shown: usize,
    pub hidden: usize,
    pub pass: usize,
    pub fail: usize,
    pub skip: usize,
    pub error: usize,
    pub required_failures: usize,
}

/// Counts the reported findings; `hidden` and `required_failures` are counted
/// from the unfiltered report, so the summary describes the whole run.
pub fn summarize(reported: &[&Report], hidden: usize, required_failures: usize) -> Summary {
    let results = || reported.iter().flat_map(|report| report.results.iter());
    let count = |wanted: CheckStatus| results().filter(|result| result.status == wanted).count();
    Summary {
        shown: results().count(),
        hidden,
        pass: count(CheckStatus::Pass),
        fail: count(CheckStatus::Fail),
        skip: count(CheckStatus::Skip),
        error: count(CheckStatus::Error),
        required_failures,
    }
}

impl Summary {
    pub fn line(&self) -> String {
        let mut parts = Vec::new();
        for (count, label) in [
            (self.pass, "pass"),
            (self.fail, "fail"),
            (self.skip, "skip"),
            (self.error, "error"),
        ] {
            if count > 0 {
                parts.push(format!("{count} {label}"));
            }
        }
        let hidden = match self.hidden {
            0 => String::new(),
            count => format!(" ({count} hidden, see --all)"),
        };
        let verdict = match self.required_failures {
            0 => "no required failures (exit 0)".to_string(),
            1 => "1 required failure (exit 1)".to_string(),
            count => format!("{count} required failures (exit 1)"),
        };
        // "results", not "checks": one check reports once per project or per file,
        // so 47 checks routinely produce many more findings than that.
        format!(
            "{} result{}{hidden}: {} — {verdict}",
            self.shown,
            if self.shown == 1 { "" } else { "s" },
            parts.join(", "),
        )
    }
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
