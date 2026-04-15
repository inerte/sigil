#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DocKind {
    Guide,
    Docs,
    Spec,
    Article,
}

impl DocKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Guide => "guide",
            Self::Docs => "docs",
            Self::Spec => "spec",
            Self::Article => "article",
        }
    }

    pub fn search_rank(self) -> u8 {
        match self {
            Self::Guide => 0,
            Self::Docs => 1,
            Self::Spec => 2,
            Self::Article => 3,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorpusSource {
    pub kind: DocKind,
    pub relative_path: String,
    pub absolute_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessedLine {
    pub line: usize,
    pub text: String,
    pub section: Option<String>,
}

pub fn collect_corpus_sources(repo_root: &Path) -> Result<Vec<CorpusSource>, String> {
    let mut sources = Vec::new();

    sources.push(source_for(repo_root, DocKind::Guide, "README.md")?);
    sources.push(source_for(repo_root, DocKind::Guide, "language/README.md")?);

    for entry in sorted_dir_entries(&repo_root.join("language/docs"))? {
        if is_markdown(&entry) {
            sources.push(source_for(
                repo_root,
                DocKind::Docs,
                &relative_path_from_repo_root(repo_root, &entry)?,
            )?);
        }
    }

    for entry in sorted_dir_entries(&repo_root.join("language/spec"))? {
        if is_markdown(&entry) {
            sources.push(source_for(
                repo_root,
                DocKind::Spec,
                &relative_path_from_repo_root(repo_root, &entry)?,
            )?);
        }
    }

    sources.push(source_for(
        repo_root,
        DocKind::Spec,
        "language/spec/grammar.ebnf",
    )?);

    for entry in sorted_dir_entries(&repo_root.join("website/articles"))? {
        if !is_markdown(&entry) {
            continue;
        }
        if entry.file_name().and_then(|name| name.to_str()) == Some("README.md") {
            continue;
        }
        sources.push(source_for(
            repo_root,
            DocKind::Article,
            &relative_path_from_repo_root(repo_root, &entry)?,
        )?);
    }

    Ok(sources)
}

pub fn derive_doc_id(kind: DocKind, relative_path: &str) -> Result<String, String> {
    match (kind, relative_path) {
        (DocKind::Guide, "README.md") => Ok("guide/root-readme".to_string()),
        (DocKind::Guide, "language/README.md") => Ok("guide/language-readme".to_string()),
        (DocKind::Spec, "language/spec/grammar.ebnf") => Ok("spec/grammar".to_string()),
        (DocKind::Docs, path) | (DocKind::Spec, path) => {
            let stem = stem_for(path)?;
            Ok(format!("{}/{}", kind.as_str(), normalize_stem(stem)))
        }
        (DocKind::Article, path) => {
            let stem = stem_for(path)?;
            Ok(format!(
                "article/{}",
                normalize_stem(strip_numeric_prefix(stem))
            ))
        }
        (DocKind::Guide, path) => Err(format!("unsupported guide path `{path}`")),
    }
}

pub fn extract_title(kind: DocKind, relative_path: &str, contents: &str) -> String {
    if matches!(kind, DocKind::Article) {
        if let Some(title) = extract_frontmatter_title(contents) {
            return title;
        }
    }

    if is_markdown_path(relative_path) {
        if let Some(title) = first_markdown_heading(contents) {
            return title;
        }
    } else if relative_path.ends_with("grammar.ebnf") {
        if let Some(title) = first_grammar_comment(contents) {
            return title;
        }
    }

    fallback_title_from_path(relative_path)
}

pub fn extract_description(kind: DocKind, contents: &str, fallback_title: &str) -> String {
    match kind {
        DocKind::Guide | DocKind::Docs | DocKind::Spec | DocKind::Article => {
            if let Some(paragraph) = first_markdown_paragraph(contents) {
                return paragraph;
            }
            if let Some(comment) = first_grammar_comment(contents) {
                return comment;
            }
            fallback_title.to_string()
        }
    }
}

pub fn split_lines_with_sections(kind: DocKind, contents: &str) -> Vec<ProcessedLine> {
    if matches!(kind, DocKind::Spec) && contents.starts_with("(*") {
        return raw_lines(contents)
            .into_iter()
            .enumerate()
            .map(|(index, text)| ProcessedLine {
                line: index + 1,
                text,
                section: None,
            })
            .collect();
    }

    let mut lines = Vec::new();
    let mut current_section: Option<String> = None;
    let mut in_frontmatter = starts_with_frontmatter(contents);
    let mut frontmatter_complete = !in_frontmatter;
    let mut in_code_fence = false;

    for (index, raw_line) in raw_lines(contents).into_iter().enumerate() {
        let trimmed = raw_line.trim();

        if in_frontmatter {
            if index != 0 && trimmed == "---" {
                in_frontmatter = false;
                frontmatter_complete = true;
            }
            lines.push(ProcessedLine {
                line: index + 1,
                text: raw_line,
                section: None,
            });
            continue;
        }

        if frontmatter_complete && trimmed.starts_with("```") {
            in_code_fence = !in_code_fence;
        }

        if !in_code_fence {
            if let Some(section) = markdown_heading_text(trimmed) {
                current_section = Some(section.to_string());
            }
        }

        lines.push(ProcessedLine {
            line: index + 1,
            text: raw_line,
            section: current_section.clone(),
        });
    }

    lines
}

fn source_for(repo_root: &Path, kind: DocKind, relative_path: &str) -> Result<CorpusSource, String> {
    let absolute_path = repo_root.join(relative_path);
    if !absolute_path.exists() {
        return Err(format!("missing corpus source `{}`", absolute_path.display()));
    }
    Ok(CorpusSource {
        kind,
        relative_path: relative_path.to_string(),
        absolute_path,
    })
}

fn sorted_dir_entries(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut entries = fs::read_dir(dir)
        .map_err(|error| format!("failed to read `{}`: {error}", dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to list `{}`: {error}", dir.display()))?
        .into_iter()
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.file_name().cmp(&right.file_name()));
    Ok(entries)
}

fn normalize_relative_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn relative_path_from_repo_root(repo_root: &Path, path: &Path) -> Result<String, String> {
    let relative = path
        .strip_prefix(repo_root)
        .map_err(|error| format!("failed to relativize `{}`: {error}", path.display()))?;
    Ok(normalize_relative_path(relative))
}

fn is_markdown(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("md")
}

fn is_markdown_path(path: &str) -> bool {
    path.ends_with(".md")
}

fn stem_for(path: &str) -> Result<&str, String> {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| format!("failed to derive file stem from `{path}`"))
}

fn strip_numeric_prefix(stem: &str) -> &str {
    let bytes = stem.as_bytes();
    let digits = bytes.iter().take_while(|byte| byte.is_ascii_digit()).count();
    if digits > 0 && bytes.get(digits) == Some(&b'-') {
        &stem[(digits + 1)..]
    } else {
        stem
    }
}

fn normalize_stem(stem: &str) -> String {
    stem.chars()
        .map(|ch| match ch {
            '_' => '-',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

fn fallback_title_from_path(path: &str) -> String {
    let stem = stem_for(path).unwrap_or(path);
    strip_numeric_prefix(stem)
        .split(['-', '_'])
        .filter(|segment| !segment.is_empty())
        .map(capitalize_word)
        .collect::<Vec<_>>()
        .join(" ")
}

fn capitalize_word(word: &str) -> String {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut result = String::new();
    result.push(first.to_ascii_uppercase());
    for ch in chars {
        result.push(ch.to_ascii_lowercase());
    }
    result
}

fn extract_frontmatter_title(contents: &str) -> Option<String> {
    let mut lines = contents.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        if let Some(value) = trimmed.strip_prefix("title:") {
            let title = value.trim().trim_matches('"').trim_matches('\'');
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
    }
    None
}

fn first_markdown_heading(contents: &str) -> Option<String> {
    let mut in_frontmatter = starts_with_frontmatter(contents);
    let mut in_code_fence = false;

    for (index, line) in raw_lines(contents).into_iter().enumerate() {
        let trimmed = line.trim();
        if in_frontmatter {
            if index != 0 && trimmed == "---" {
                in_frontmatter = false;
            }
            continue;
        }
        if trimmed.starts_with("```") {
            in_code_fence = !in_code_fence;
            continue;
        }
        if in_code_fence {
            continue;
        }
        if let Some(heading) = markdown_heading_text(trimmed) {
            return Some(heading.to_string());
        }
    }
    None
}

fn first_markdown_paragraph(contents: &str) -> Option<String> {
    let mut in_frontmatter = starts_with_frontmatter(contents);
    let mut in_code_fence = false;
    let mut paragraph = Vec::new();

    for (index, line) in raw_lines(contents).into_iter().enumerate() {
        let trimmed = line.trim();
        if in_frontmatter {
            if index != 0 && trimmed == "---" {
                in_frontmatter = false;
            }
            continue;
        }
        if trimmed.starts_with("```") {
            in_code_fence = !in_code_fence;
            if !paragraph.is_empty() {
                break;
            }
            continue;
        }
        if in_code_fence {
            continue;
        }
        if trimmed.is_empty() {
            if !paragraph.is_empty() {
                break;
            }
            continue;
        }
        if markdown_heading_text(trimmed).is_some() {
            if !paragraph.is_empty() {
                break;
            }
            continue;
        }

        paragraph.push(trimmed.to_string());
    }

    if paragraph.is_empty() {
        None
    } else {
        Some(paragraph.join(" "))
    }
}

fn first_grammar_comment(contents: &str) -> Option<String> {
    for line in raw_lines(contents) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(comment) = trimmed
            .strip_prefix("(*")
            .and_then(|rest| rest.strip_suffix("*)"))
        {
            let comment = comment.trim();
            if !comment.is_empty() {
                return Some(comment.to_string());
            }
        }
        return Some(trimmed.to_string());
    }
    None
}

fn markdown_heading_text(line: &str) -> Option<&str> {
    let heading = line.trim_start_matches('#');
    if heading.len() == line.len() {
        return None;
    }
    let heading = heading.trim_start();
    if heading.is_empty() {
        return None;
    }
    Some(heading)
}

fn starts_with_frontmatter(contents: &str) -> bool {
    raw_lines(contents)
        .first()
        .map(|line| line.trim() == "---")
        .unwrap_or(false)
}

fn raw_lines(contents: &str) -> Vec<String> {
    if contents.is_empty() {
        return Vec::new();
    }

    contents
        .split_terminator('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        collect_corpus_sources, derive_doc_id, extract_title, split_lines_with_sections, DocKind,
    };
    use std::path::PathBuf;

    #[test]
    fn derives_doc_ids_from_paths() {
        assert_eq!(
            derive_doc_id(DocKind::Guide, "README.md").unwrap(),
            "guide/root-readme"
        );
        assert_eq!(
            derive_doc_id(DocKind::Guide, "language/README.md").unwrap(),
            "guide/language-readme"
        );
        assert_eq!(
            derive_doc_id(DocKind::Docs, "language/docs/CANONICAL_ENFORCEMENT.md").unwrap(),
            "docs/canonical-enforcement"
        );
        assert_eq!(
            derive_doc_id(DocKind::Spec, "language/spec/stdlib-spec.md").unwrap(),
            "spec/stdlib-spec"
        );
        assert_eq!(
            derive_doc_id(
                DocKind::Article,
                "website/articles/038-packages-use-npm-as-transport.md",
            )
            .unwrap(),
            "article/packages-use-npm-as-transport"
        );
    }

    #[test]
    fn extracts_title_from_article_frontmatter() {
        let contents = r#"---
title: Packages Use npm as Transport, Not as the Semantic Model
date: 2026-04-05
---

# Different Heading
"#;
        assert_eq!(
            extract_title(
                DocKind::Article,
                "website/articles/038-packages-use-npm-as-transport.md",
                contents,
            ),
            "Packages Use npm as Transport, Not as the Semantic Model"
        );
    }

    #[test]
    fn strips_numeric_article_prefixes() {
        assert_eq!(
            derive_doc_id(
                DocKind::Article,
                "website/articles/035-machine-first-debugging.md",
            )
            .unwrap(),
            "article/machine-first-debugging"
        );
    }

    #[test]
    fn assigns_sections_from_nearest_heading() {
        let lines = split_lines_with_sections(
            DocKind::Docs,
            "# Title\n\nIntro line\n## Details\nMore detail\n```text\n# Not a heading\n```\n",
        );
        assert_eq!(lines[0].section.as_deref(), Some("Title"));
        assert_eq!(lines[2].section.as_deref(), Some("Title"));
        assert_eq!(lines[3].section.as_deref(), Some("Details"));
        assert_eq!(lines[4].section.as_deref(), Some("Details"));
        assert_eq!(lines[6].section.as_deref(), Some("Details"));
    }

    #[test]
    fn collects_corpus_sources_in_stable_order() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(4)
            .unwrap()
            .to_path_buf();

        let sources = collect_corpus_sources(&repo_root).unwrap();
        assert_eq!(sources[0].relative_path, "README.md");
        assert_eq!(sources[1].relative_path, "language/README.md");
        assert!(sources
            .iter()
            .position(|source| source.relative_path == "language/docs/syntax-reference.md")
            .unwrap()
            < sources
                .iter()
                .position(|source| source.relative_path == "language/spec/cli-json.md")
                .unwrap());
        assert!(sources
            .iter()
            .position(|source| source.relative_path == "language/spec/grammar.ebnf")
            .unwrap()
            < sources
                .iter()
                .position(|source| {
                    source.relative_path
                        == "website/articles/001-canonical-length-operator.md"
                })
                .unwrap());
    }
}
