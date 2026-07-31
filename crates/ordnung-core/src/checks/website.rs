use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::Duration;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckStatus,
    GithubCheckContext, Severity, registry, result,
};

const TIMEOUT: Duration = Duration::from_secs(10);

static HTTP: LazyLock<ureq::Agent> = LazyLock::new(|| {
    ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(concat!(
            "ordnung/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/PowderworksCode/ordnung)"
        ))
        .build()
        .into()
});

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "website",
    default_severity: Severity::Required,
    category: CheckCategory::Documentation,
    instructions: "Keep the repository's GitHub homepage setting pointed at its reachable HTTP(S) website.",
    repository_runner: None,
    github_runner: Some(run_github),
};

registry::submit! { CheckRegistration(&CHECK) }

fn run_github(
    definition: &'static CheckDefinition,
    facts: &GithubCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let Some(homepage) = facts
        .homepage
        .as_deref()
        .map(str::trim)
        .filter(|homepage| !homepage.is_empty())
    else {
        results.push(result(
            definition,
            CheckStatus::Fail,
            PathBuf::new(),
            "GitHub homepage is not set",
        ));
        return;
    };

    let (status, message) = match probe(homepage) {
        Probe::Reachable => (
            CheckStatus::Pass,
            format!("GitHub homepage is reachable: {homepage}"),
        ),
        Probe::HttpFailure(code) => (
            CheckStatus::Fail,
            format!("GitHub homepage {homepage} returned HTTP {code}"),
        ),
        Probe::Invalid(reason) => (
            CheckStatus::Fail,
            format!("GitHub homepage {homepage} is invalid: {reason}"),
        ),
        Probe::Unavailable(reason) => (
            CheckStatus::Error,
            format!("could not check GitHub homepage {homepage}: {reason}"),
        ),
    };
    results.push(result(definition, status, PathBuf::new(), message));
}

fn is_http_url(target: &str) -> bool {
    target
        .get(..7)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("http://"))
        || target
            .get(..8)
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"))
}

enum Probe {
    Reachable,
    HttpFailure(u16),
    Invalid(String),
    Unavailable(String),
}

fn probe(url: &str) -> Probe {
    if !is_http_url(url) {
        return Probe::Invalid("expected an HTTP(S) URL".into());
    }
    match HTTP.get(url).call() {
        Ok(response) if response.status().is_success() => Probe::Reachable,
        Ok(response) => Probe::HttpFailure(response.status().as_u16()),
        Err(ureq::Error::StatusCode(status)) => Probe::HttpFailure(status),
        Err(error @ (ureq::Error::BadUri(_) | ureq::Error::Http(_))) => {
            Probe::Invalid(error.to_string())
        }
        Err(error) => Probe::Unavailable(error.to_string()),
    }
}
