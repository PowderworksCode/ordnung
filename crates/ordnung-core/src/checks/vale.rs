use std::path::{Component, Path, PathBuf};

use entl_codebase::VALE;

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "vale",
    default_severity: Severity::Required,
    category: CheckCategory::Documentation,
    instructions: "Keep a root .vale.ini with an existing relative StylesPath when declared, and run Vale from a push or pull-request workflow.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let config = PathBuf::from(".vale.ini");
    if !context.inventory.files.contains(&config) {
        results.push(result(
            definition,
            CheckStatus::Fail,
            config,
            "no root .vale.ini",
        ));
        return;
    }
    let text = match std::fs::read_to_string(context.root.join(&config)) {
        Ok(text) => text,
        Err(error) => {
            results.push(result(
                definition,
                CheckStatus::Error,
                config,
                format!("could not read .vale.ini: {error}"),
            ));
            return;
        }
    };
    let styles = styles_path(&text);
    if let Some(styles) = styles.as_deref().filter(|styles| !styles.is_empty()) {
        let path = Path::new(styles);
        if path.is_absolute()
            || path
                .components()
                .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
        {
            results.push(result(
                definition,
                CheckStatus::Fail,
                config,
                format!("StylesPath is not a safe relative path: {styles}"),
            ));
            return;
        }
        if !context
            .inventory
            .files
            .iter()
            .any(|candidate| candidate == path || candidate.starts_with(path))
        {
            results.push(result(
                definition,
                CheckStatus::Fail,
                path.to_path_buf(),
                format!("StylesPath does not exist: {styles}"),
            ));
            return;
        }
    }
    let invocation = context
        .inventory
        .github
        .tool_invocations
        .iter()
        .find(|invocation| invocation.runs_on_changes && invocation.tool.as_str() == VALE.id);
    results.push(result(
        definition,
        if invocation.is_some() {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        invocation.map_or_else(
            || PathBuf::from(".github/workflows"),
            |invocation| invocation.workflow.clone(),
        ),
        if invocation.is_some() {
            "Vale configuration and change-triggered workflow are present"
        } else {
            ".vale.ini is present but no push or pull-request workflow runs Vale"
        },
    ));
}

fn styles_path(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .take_while(|line| !line.starts_with('['))
        .find_map(|line| {
            let (key, value) = line.split_once('=')?;
            key.trim()
                .eq_ignore_ascii_case("StylesPath")
                .then(|| value.trim().to_owned())
        })
}
