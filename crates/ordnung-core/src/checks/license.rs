use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckStatus,
    GithubCheckContext, RepositoryCheckContext, Severity, registry, result,
};

const CANDIDATES: [&str; 5] = [
    "LICENSE",
    "LICENSE.md",
    "LICENSE.txt",
    "COPYING",
    "UNLICENSE",
];

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "license",
    default_severity: Severity::Required,
    category: CheckCategory::Documentation,
    instructions: "Keep a root LICENSE, LICENSE.md, LICENSE.txt, COPYING, or UNLICENSE file; GitHub SPDX classification is useful but nonstandard license text is allowed.",
    repository_runner: Some(run_repository),
    github_runner: Some(run_github),
};

registry::submit! { CheckRegistration(&CHECK) }

fn run_repository(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let license = CANDIDATES.iter().find_map(|candidate| {
        let path = PathBuf::from(candidate);
        context.inventory.files.contains(&path).then_some(path)
    });
    results.push(result(
        definition,
        if license.is_some() {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        license.clone().unwrap_or_else(|| PathBuf::from("LICENSE")),
        license.map_or_else(
            || "no root license file found".to_owned(),
            |path| format!("root license file present at {}", path.display()),
        ),
    ));
}

fn run_github(
    definition: &'static CheckDefinition,
    facts: &GithubCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let (status, message) = match &facts.license {
        Some(license) if license.spdx_id != "NOASSERTION" => (
            CheckStatus::Pass,
            format!("GitHub detects {} ({})", license.name, license.spdx_id),
        ),
        Some(license) => (
            CheckStatus::Skip,
            format!(
                "GitHub has not classified {} ({}) but nonstandard text is allowed",
                license.name, license.key
            ),
        ),
        None => (
            CheckStatus::Skip,
            "GitHub reports no classified license; the repository check verifies file presence"
                .to_owned(),
        ),
    };
    results.push(result(definition, status, PathBuf::new(), message));
}
