use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckRemediation, CheckResult, CheckScope,
    CheckStatus, RepositoryCheckContext, Severity, registry, result, result_with_remediation,
};

const FILE_NAME: &str = "field_guide.md";
const PREFERRED_PATH: &str = "notes/field_guide.md";

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "field-guide",
    default_severity: Severity::Off,
    category: CheckCategory::Documentation,
    scope: CheckScope::Repository,
    instructions: "At the start of work, find and read `field_guide.md`; append concise, durable discoveries that will help future agents. Keep the file in the repository, preferably at `notes/field_guide.md`.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let guides = context
        .inventory
        .files
        .iter()
        .filter(|path| path.file_name().is_some_and(|name| name == FILE_NAME))
        .collect::<Vec<_>>();

    let Some(first) = guides.first() else {
        results.push(result_with_remediation(
            definition,
            CheckStatus::Fail,
            PathBuf::from(PREFERRED_PATH),
            format!("no {FILE_NAME} found; create {PREFERRED_PATH}"),
            CheckRemediation::create(
                PREFERRED_PATH,
                b"# Agent Field Guide\n\nRecord concise, durable discoveries that will help future agents work in this repository.\n".to_vec(),
                "create the agent field guide",
            ),
        ));
        return;
    };

    results.push(result(
        definition,
        CheckStatus::Pass,
        (*first).clone(),
        if guides.len() == 1 {
            format!("agent field guide found at {}", first.display())
        } else {
            format!(
                "agent field guides found at {}",
                guides
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        },
    ));
}
