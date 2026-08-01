use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

use super::test_layout;

/// Off by default because one test file per source file is a considerably stronger
/// claim than keeping tests out of source files: it fires on entry points, module
/// roots, and files already covered by a shared suite.
pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "test-mirror",
    default_severity: Severity::Off,
    category: CheckCategory::BuildToolchain,
    instructions: "Give every source file a mirrored test file under the configured test root, matching its path and a configured test suffix.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    for resolved in test_layout::resolve(definition, context, true, results) {
        let mut missing = 0;
        for source_file in &resolved.source_files {
            if test_layout::has_mirrored_test(
                &resolved.project_root,
                source_file,
                resolved.language,
                &resolved.layout,
            ) {
                continue;
            }
            missing += 1;
            results.push(result(
                definition,
                CheckStatus::Fail,
                resolved.scope(context.root, source_file),
                format!(
                    "no mirrored test file under {}",
                    resolved
                        .project
                        .root
                        .join(&resolved.layout.test_root)
                        .display()
                ),
            ));
        }
        if missing == 0 {
            results.push(result(
                definition,
                CheckStatus::Pass,
                resolved.project.root.clone(),
                format!(
                    "all {} {} source file(s) have mirrored tests",
                    resolved.source_files.len(),
                    resolved.language.display_name
                ),
            ));
        }
    }
}
