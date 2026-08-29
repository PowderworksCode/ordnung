use std::path::{Component, Path, PathBuf};

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag};

use crate::check::{
    CheckCategory, CheckDefinition, CheckRegistration, CheckResult, CheckScope, CheckStatus,
    RepositoryCheckContext, Severity, registry, result,
};

pub(crate) static CHECK: CheckDefinition = CheckDefinition {
    id: "readme",
    default_severity: Severity::Required,
    category: CheckCategory::Documentation,
    scope: CheckScope::Repository,
    instructions: "Keep a root README that opens with an H1 title in its first ten nonblank lines.",
    repository_runner: Some(run),
    github_runner: None,
};

registry::submit! { CheckRegistration(&CHECK) }

fn run(
    definition: &'static CheckDefinition,
    context: &RepositoryCheckContext<'_>,
    results: &mut Vec<CheckResult>,
) {
    let Some(path) = root_readme(&context.inventory.files) else {
        results.push(result(
            definition,
            CheckStatus::Fail,
            PathBuf::from("README.md"),
            "no root README found",
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
    results.push(if facts.has_early_title {
        result(
            definition,
            CheckStatus::Pass,
            path.clone(),
            format!("{} opens with an H1 title", path.display()),
        )
    } else {
        result(
            definition,
            CheckStatus::Fail,
            path.clone(),
            "no H1 title heading in the first 10 nonblank lines",
        )
    });
}

pub(super) struct ReadmeFacts {
    pub(super) words: usize,
    pub(super) has_early_title: bool,
    pub(super) has_install: bool,
    pub(super) has_usage: bool,
    pub(super) has_license: bool,
    pub(super) has_contributing: bool,
    pub(super) relative_links: Vec<String>,
}

struct Heading {
    level: HeadingLevel,
    start: usize,
    text: String,
}

pub(super) fn inspect(text: &str) -> ReadmeFacts {
    let mut headings = Vec::new();
    let mut current_heading: Option<Heading> = None;
    let mut relative_links = Vec::new();
    for (event, range) in Parser::new_ext(text, Options::ENABLE_GFM).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current_heading = Some(Heading {
                    level,
                    start: range.start,
                    text: String::new(),
                });
            }
            Event::End(pulldown_cmark::TagEnd::Heading(_)) => {
                if let Some(heading) = current_heading.take() {
                    headings.push(heading);
                }
            }
            Event::Text(value) | Event::Code(value) => {
                if let Some(heading) = &mut current_heading {
                    if !heading.text.is_empty() {
                        heading.text.push(' ');
                    }
                    heading.text.push_str(&value);
                }
            }
            Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) => {
                let target = dest_url.trim();
                if is_repository_relative(target) {
                    relative_links.push(target.to_owned());
                }
            }
            _ => {}
        }
    }
    relative_links.sort();
    relative_links.dedup();
    let early_cutoff = first_nonblank_lines_end(text, 10);
    let heading_text = headings
        .iter()
        .map(|heading| heading.text.to_ascii_lowercase())
        .collect::<Vec<_>>();
    ReadmeFacts {
        words: text.split_whitespace().count(),
        has_early_title: headings
            .iter()
            .any(|heading| heading.level == HeadingLevel::H1 && heading.start < early_cutoff),
        has_install: heading_text.iter().any(|heading| {
            contains_word(heading, "install")
                || contains_word(heading, "setup")
                || heading.contains("getting started")
                || heading.contains("quick start")
                || heading.contains("quickstart")
        }),
        has_usage: heading_text.iter().any(|heading| {
            [
                "usage",
                "use",
                "example",
                "examples",
                "how",
                "docs",
                "documentation",
            ]
            .iter()
            .any(|word| contains_word(heading, word))
        }),
        has_license: heading_text.iter().any(|heading| {
            words(heading).any(|word| word.starts_with("license") || word.starts_with("licenc"))
        }),
        has_contributing: heading_text
            .iter()
            .any(|heading| words(heading).any(|word| word.starts_with("contribut"))),
        relative_links,
    }
}

pub(super) fn root_readme(files: &std::collections::BTreeSet<PathBuf>) -> Option<&PathBuf> {
    files.iter().find(|path| {
        path.parent()
            .is_some_and(|parent| parent.as_os_str().is_empty())
            && path
                .file_stem()
                .is_some_and(|stem| stem.to_string_lossy().eq_ignore_ascii_case("readme"))
    })
}

fn first_nonblank_lines_end(text: &str, limit: usize) -> usize {
    let mut nonblank = 0;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        offset += line.len();
        if !line.trim().is_empty() {
            nonblank += 1;
            if nonblank == limit {
                return offset;
            }
        }
    }
    text.len().saturating_add(1)
}

fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
}

fn contains_word(text: &str, expected: &str) -> bool {
    words(text).any(|word| word == expected)
}

fn is_repository_relative(target: &str) -> bool {
    if target.is_empty() || target.starts_with('#') || target.starts_with("//") {
        return false;
    }
    !target.split_once(':').is_some_and(|(scheme, _)| {
        !scheme.is_empty()
            && scheme.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
            })
    })
}

pub(super) fn relative_target_exists(
    target: &str,
    files: &std::collections::BTreeSet<PathBuf>,
) -> bool {
    let target = target.split(['#', '?']).next().unwrap_or_default().trim();
    if target.is_empty() {
        return true;
    }
    let path = Path::new(target);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return false;
    }
    let normalized = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect::<PathBuf>();
    files.contains(&normalized)
        || files
            .iter()
            .any(|candidate| candidate.starts_with(&normalized))
}
