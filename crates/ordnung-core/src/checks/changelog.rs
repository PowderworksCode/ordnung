use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckRemediation, CheckResult, CheckScope,
    CheckStatus, RepositoryCheckContext, Severity, registry, result, result_with_remediation,
};

const CANDIDATES: [&str; 5] = [
    "CHANGELOG.md",
    "CHANGELOG",
    "CHANGELOG.txt",
    "CHANGES.md",
    "HISTORY.md",
];

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "changelog",
    default_severity: Severity::Recommended,
    category: CheckCategory::Documentation,
    scope: CheckScope::Repository,
    instructions: "Keep a root CHANGELOG.md, CHANGELOG, CHANGELOG.txt, CHANGES.md, or HISTORY.md; format and versioning style are repository choices.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let path = CANDIDATES.iter().find_map(|name| {
        let path = PathBuf::from(name);
        context.inventory.files.contains(&path).then_some(path)
    });
    if let Some(path) = path {
        results.push(result(
            definition,
            CheckStatus::Pass,
            path.clone(),
            format!("root changelog present at {}", path.display()),
        ));
    } else {
        results.push(result_with_remediation(
            definition,
            CheckStatus::Fail,
            "CHANGELOG.md".into(),
            "no root changelog found",
            CheckRemediation::create(
                "CHANGELOG.md",
                b"# Changelog\n\nNotable changes to this project are recorded here.\n".to_vec(),
                "create a root changelog",
            ),
        ));
    }
}
