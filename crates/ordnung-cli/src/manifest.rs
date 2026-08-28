//! The exported check manifest: what `--list-checks` prints.
//!
//! The manifest is the contract between the binary and its documentation. The
//! website's test suite reads the JSON form (exported to the site by
//! `scripts/checks-manifest.sh`) and fails the build when a page documents a
//! check the binary does not carry, omits one it does, or quotes the wrong
//! severity — so the fields here are load-bearing strings, not decoration.

use ordnung_core::{CheckCategory, CheckDefinition, CheckScope, check_definitions};
use serde::Serialize;

use crate::render::severity_name;

pub const SCHEMA: &str = "ordnung.checks/1";

/// Check IDs that once existed and were withdrawn. The documentation tests
/// refuse any mention of these, so a page cannot keep describing a check the
/// binary no longer carries. A renamed or deleted check ID belongs here.
pub const REMOVED: &[&str] = &[];

#[derive(Serialize)]
pub struct Manifest {
    schema: &'static str,
    version: &'static str,
    checks: Vec<ManifestCheck>,
    removed: &'static [&'static str],
}

#[derive(Serialize)]
struct ManifestCheck {
    id: &'static str,
    summary: &'static str,
    category: &'static str,
    scope: &'static str,
    default_severity: &'static str,
    /// Which audits run the check: the local working tree, the GitHub
    /// repository settings, or both.
    surfaces: Vec<&'static str>,
}

const fn scope_name(scope: CheckScope) -> &'static str {
    match scope {
        CheckScope::Repository => "repository",
        CheckScope::Project => "project",
    }
}

fn surfaces(definition: &CheckDefinition) -> Vec<&'static str> {
    let mut surfaces = Vec::new();
    if definition.repository_runner.is_some() {
        surfaces.push("repository");
    }
    if definition.github_runner.is_some() {
        surfaces.push("github");
    }
    surfaces
}

impl Manifest {
    pub fn build() -> Self {
        Self {
            schema: SCHEMA,
            version: env!("CARGO_PKG_VERSION"),
            checks: check_definitions()
                .iter()
                .map(|definition| ManifestCheck {
                    id: definition.id,
                    summary: definition.instructions,
                    category: definition.category.display_name(),
                    scope: scope_name(definition.scope),
                    default_severity: severity_name(definition.default_severity),
                    surfaces: surfaces(definition),
                })
                .collect(),
            removed: REMOVED,
        }
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for category in CheckCategory::ALL {
            let members = self
                .checks
                .iter()
                .filter(|check| check.category == category.display_name())
                .collect::<Vec<_>>();
            if members.is_empty() {
                continue;
            }
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(category.display_name());
            out.push('\n');
            for check in members {
                out.push_str(&format!(
                    "  {:<26} {:<12} {:<11} {}\n",
                    check.id, check.default_severity, check.scope, check.summary
                ));
            }
        }
        out
    }
}
