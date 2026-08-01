use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use entl_codebase::{TestRetrySignal, tool_profile, tool_profiles};

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "test-retry-masking",
    default_severity: Severity::Required,
    category: CheckCategory::CiSafety,
    scope: CheckScope::Repository,
    instructions: "Do not configure test commands or standard Rust and TypeScript test-runner configuration to rerun failures until they pass.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let mut hits = BTreeSet::new();
    for task in context.inventory.github.task_invocations() {
        let Some(retry) = tool_profile(task.tool.as_str()).and_then(|tool| tool.test_retry) else {
            continue;
        };
        if task.arguments.iter().any(|argument| {
            retry.arguments.iter().any(|candidate| {
                argument == candidate || argument.starts_with(&format!("{candidate}="))
            })
        }) {
            hits.insert(format!("{}:{}", task.workflow.display(), task.job));
        }
    }

    for tool in tool_profiles() {
        let Some(retry) = tool.test_retry else {
            continue;
        };
        for configuration in retry.configurations {
            for relative in &context.inventory.files {
                if !configuration
                    .paths
                    .iter()
                    .any(|candidate| relative.ends_with(candidate))
                {
                    continue;
                }
                let Ok(text) = fs::read_to_string(context.root.join(relative)) else {
                    continue;
                };
                if configuration
                    .signals
                    .iter()
                    .any(|signal| retry_signal_present(&text, *signal))
                {
                    hits.insert(relative.display().to_string());
                }
            }
        }
    }

    results.push(if hits.is_empty() {
        result(
            definition,
            CheckStatus::Pass,
            Path::new(".").into(),
            "no rerun-until-green test retry is configured",
        )
    } else {
        result(
            definition,
            CheckStatus::Fail,
            Path::new(".").into(),
            format!(
                "test retries can mask intermittent failures: {}",
                hits.into_iter().collect::<Vec<_>>().join(", ")
            ),
        )
    });
}

fn retry_signal_present(text: &str, signal: TestRetrySignal) -> bool {
    match signal {
        TestRetrySignal::JavascriptProperty(key) => javascript_positive_value(text, key, ':'),
        TestRetrySignal::JavascriptCall(key) => javascript_positive_value(text, key, '('),
        TestRetrySignal::TomlPositiveInteger(key) => toml::from_str::<toml::Value>(text)
            .is_ok_and(|value| toml_has_positive_integer(&value, key)),
    }
}

fn javascript_positive_value(text: &str, key: &str, separator: char) -> bool {
    let compact = text
        .chars()
        .filter(|character| !character.is_whitespace() && !matches!(character, '\'' | '"'))
        .collect::<String>();
    let needle = format!("{key}{separator}");
    compact.match_indices(&needle).any(|(index, _)| {
        let before = compact[..index].chars().next_back();
        if before.is_some_and(|character| character.is_ascii_alphanumeric() || character == '_') {
            return false;
        }
        compact[index + needle.len()..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse::<u64>()
            .is_ok_and(|value| value > 0)
    })
}

fn toml_has_positive_integer(value: &toml::Value, key: &str) -> bool {
    match value {
        toml::Value::Table(table) => table.iter().any(|(candidate, value)| {
            (candidate == key && value.as_integer().is_some_and(|value| value > 0))
                || toml_has_positive_integer(value, key)
        }),
        toml::Value::Array(values) => values
            .iter()
            .any(|value| toml_has_positive_integer(value, key)),
        _ => false,
    }
}
