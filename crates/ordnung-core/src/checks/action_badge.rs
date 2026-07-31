use std::path::PathBuf;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckStatus,
    GithubCheckContext, Severity, registry, result,
};
use crate::github::GithubValue;

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "action-badge",
    default_severity: Severity::Required,
    category: CheckCategory::Documentation,
    instructions: "For a public repository that publishes a root GitHub Action, link its exact Marketplace listing from the root README.",
    repository_runner: None,
    github_runner: Some(run),
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &GithubCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    if context.visibility != "public" {
        results.push(result(
            definition,
            CheckStatus::Skip,
            PathBuf::new(),
            "repository is not public, so it cannot publish to GitHub Marketplace",
        ));
        return;
    }
    let publication = match &context.action_publication {
        GithubValue::Known { value: Some(value) } => value,
        GithubValue::Known { value: None } => {
            results.push(result(
                definition,
                CheckStatus::Skip,
                PathBuf::new(),
                "repository does not publish a root GitHub Action",
            ));
            return;
        }
        GithubValue::Unavailable { reason } => {
            results.push(result(
                definition,
                CheckStatus::Error,
                PathBuf::new(),
                format!("could not inspect action publication: {reason}"),
            ));
            return;
        }
    };
    let scope = publication
        .readme_path
        .clone()
        .unwrap_or_else(|| PathBuf::from("README"));
    results.push(result(
        definition,
        if publication.marketplace_linked {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        scope,
        if publication.marketplace_linked {
            format!(
                "README links the Marketplace listing for {}",
                publication.name
            )
        } else if publication.readme_path.is_none() {
            format!(
                "{} publishes a GitHub Action but has no root README linking {}",
                publication.manifest_path.display(),
                publication.marketplace_url
            )
        } else {
            format!(
                "README does not link the Marketplace listing {}",
                publication.marketplace_url
            )
        },
    ));
}
