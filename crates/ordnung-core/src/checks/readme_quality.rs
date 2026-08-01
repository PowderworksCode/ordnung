use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

use super::readme::{inspect, relative_target_exists, root_readme};

const MIN_WORDS: usize = 150;
const MAX_WORDS: usize = 1_500;
const BROKEN_LINK_LIMIT: usize = 5;

/// A definition of a good README rather than a universal one: the length band and
/// the expected sections are a house style, so this is advisory by default and a
/// fleet that wants the shape enforced raises it. Whether a README exists at all
/// is the `readme` check.
pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "readme-quality",
    default_severity: Severity::Recommended,
    category: CheckCategory::Documentation,
    instructions: "Keep the root README between 150 and 1,500 words with install/getting-started, usage/docs, contributing, and license sections, and no broken repository-relative Markdown links.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    // Absence is the `readme` check's finding; reporting it twice adds no signal.
    let Some(path) = root_readme(&context.inventory.files) else {
        results.push(result(
            definition,
            CheckStatus::Skip,
            PathBuf::from("README.md"),
            "no root README to assess",
        ));
        return;
    };
    let text = match std::fs::read_to_string(context.root.join(path)) {
        Ok(text) => text,
        Err(error) => {
            results.push(result(
                definition,
                CheckStatus::Error,
                path.clone(),
                format!("could not read {}: {error}", path.display()),
            ));
            return;
        }
    };

    let facts = inspect(&text);
    let mut problems = Vec::new();
    if facts.words < MIN_WORDS {
        problems.push(format!("under {MIN_WORDS} words ({})", facts.words));
    }
    if facts.words > MAX_WORDS {
        problems.push(format!("over {MAX_WORDS} words ({})", facts.words));
    }
    if !facts.has_install {
        problems.push("no install/getting-started section".to_owned());
    }
    if !facts.has_usage {
        problems.push("no usage/docs section".to_owned());
    }
    if !facts.has_license {
        problems.push("no License section heading".to_owned());
    }
    if !facts.has_contributing {
        problems.push("no Contributing section heading".to_owned());
    }
    let broken = facts
        .relative_links
        .iter()
        .filter(|target| !relative_target_exists(target, &context.inventory.files))
        .take(BROKEN_LINK_LIMIT)
        .cloned()
        .collect::<Vec<_>>();
    if !broken.is_empty() {
        problems.push(format!("broken relative links: {}", broken.join(", ")));
    }

    results.push(result(
        definition,
        if problems.is_empty() {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        path.clone(),
        if problems.is_empty() {
            format!("{} passes the README floor", path.display())
        } else {
            problems.join("; ")
        },
    ));
}
