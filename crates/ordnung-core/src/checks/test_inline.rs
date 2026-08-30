use std::fs;

use entl::codebase::language_conventions;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

use super::test_layout;

/// Off by default because Rust's inline `#[cfg(test)]` module is idiomatic, so
/// requiring its absence is a position rather than a consensus. A fleet that wants
/// tests kept out of source files raises this.
pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "test-inline",
    default_severity: Severity::Off,
    category: CheckCategory::BuildToolchain,
    scope: CheckScope::Project,
    instructions: "Keep tests out of source files; move an inline test module under the configured test root.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    for resolved in test_layout::resolve(definition, context, false, results) {
        let mut violations = 0;
        for source_file in &resolved.source_files {
            let scope = resolved.scope(context.root, source_file);
            match fs::read_to_string(source_file) {
                Ok(content) => {
                    if let Some(indicator) = language_conventions(resolved.language)
                        .and_then(|conventions| conventions.inline_test_indicator(&content))
                    {
                        violations += 1;
                        results.push(result(
                            definition,
                            CheckStatus::Fail,
                            scope,
                            format!(
                                "inline test indicator {indicator:?} belongs under the external test root"
                            ),
                        ));
                    }
                }
                Err(error) => {
                    violations += 1;
                    results.push(result(
                        definition,
                        CheckStatus::Error,
                        scope,
                        format!("could not read source file: {error}"),
                    ));
                }
            }
        }
        if violations == 0 {
            results.push(result(
                definition,
                CheckStatus::Pass,
                resolved.project.root.clone(),
                format!(
                    "{} {} source file(s) contain no inline tests",
                    resolved.source_files.len(),
                    resolved.language.display_name
                ),
            ));
        }
    }
}
