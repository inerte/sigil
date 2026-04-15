use super::legacy::CliError;
use super::shared::{output_json_error, output_json_value};
use crate::docs_support::{split_lines_with_sections, DocKind, ProcessedLine};
use serde_json::json;
use sigil_diagnostics::codes;
use std::collections::HashMap;
use std::sync::OnceLock;

const LIST_COMMAND: &str = "sigil docs list";
const SEARCH_COMMAND: &str = "sigil docs search";
const SHOW_COMMAND: &str = "sigil docs show";
const CONTEXT_COMMAND: &str = "sigil docs context";

#[derive(Debug)]
enum DocsError {
    BlankQuery,
    DocNotFound {
        doc_id: String,
    },
    ContextNotFound {
        context_id: String,
    },
    InvalidLineRange {
        doc_id: String,
        line_count: usize,
        start_line: Option<usize>,
        end_line: Option<usize>,
        reason: String,
    },
    InvalidContextRequest,
    InvalidContextReference {
        context_id: &'static str,
        doc_id: &'static str,
    },
}

struct EmbeddedDocSpec {
    doc_id: &'static str,
    kind: DocKind,
    title: &'static str,
    path: &'static str,
    description: &'static str,
    text: &'static str,
}

struct EmbeddedDocument {
    doc_id: &'static str,
    kind: DocKind,
    title: &'static str,
    path: &'static str,
    description: &'static str,
    lines: Vec<ProcessedLine>,
}

struct DocsCorpus {
    documents: Vec<EmbeddedDocument>,
    index_by_id: HashMap<&'static str, usize>,
}

struct DocsContextSpec {
    id: &'static str,
    title: &'static str,
    description: &'static str,
    included_docs: &'static [&'static str],
}

static CORPUS: OnceLock<DocsCorpus> = OnceLock::new();

static CONTEXTS: &[DocsContextSpec] = &[
    DocsContextSpec {
        id: "overview",
        title: "Overview",
        description: "Start here for the project model, installation flow, and the main language reference surfaces.",
        included_docs: &[
            "guide/language-readme",
            "guide/root-readme",
            "docs/embedded-docs",
            "docs/syntax-reference",
            "docs/stdlib",
        ],
    },
    DocsContextSpec {
        id: "syntax",
        title: "Syntax",
        description: "Canonical surface syntax, canonical printing rules, and the high-level grammar sketch.",
        included_docs: &[
            "docs/syntax-reference",
            "docs/canonical-forms",
            "docs/canonical-enforcement",
            "spec/grammar",
        ],
    },
    DocsContextSpec {
        id: "type-system",
        title: "Type System",
        description: "Type system overview plus the formal type-system and semantics references.",
        included_docs: &["docs/type-system", "spec/type-system", "spec/semantics"],
    },
    DocsContextSpec {
        id: "stdlib",
        title: "Stdlib",
        description: "The operational standard library surface and its canonical reference spec.",
        included_docs: &["docs/stdlib", "spec/stdlib-spec"],
    },
    DocsContextSpec {
        id: "testing",
        title: "Testing",
        description: "How Sigil tests work, how debugging overlaps with testing, and the CLI/testing specs.",
        included_docs: &[
            "docs/testing",
            "docs/debugging",
            "spec/testing",
            "spec/cli-json",
        ],
    },
    DocsContextSpec {
        id: "packages",
        title: "Packages",
        description: "Package semantics, package CLI behavior, and the npm-transport rationale.",
        included_docs: &[
            "docs/packages",
            "spec/packages",
            "article/packages-use-npm-as-transport",
        ],
    },
    DocsContextSpec {
        id: "topology",
        title: "Topology",
        description: "Topology-aware project structure, runtime world inspection, and the rationale for topology as runtime truth.",
        included_docs: &[
            "docs/topology",
            "spec/topology",
            "article/topology-is-runtime-truth",
            "article/topology-vs-config",
        ],
    },
    DocsContextSpec {
        id: "ffi",
        title: "FFI",
        description: "Canonical FFI usage plus the declaration-ordering rationale behind the current surface.",
        included_docs: &[
            "docs/ffi",
            "article/typed-ffi-and-declaration-ordering",
        ],
    },
    DocsContextSpec {
        id: "debugging",
        title: "Debugging",
        description: "Machine-first debugging behavior, artifacts, and the canonical CLI JSON surface.",
        included_docs: &[
            "docs/debugging",
            "article/machine-first-debugging",
            "spec/cli-json",
        ],
    },
    DocsContextSpec {
        id: "feature-flags",
        title: "Feature Flags",
        description: "Where feature flags live, how packages expose them, and how config supplies live values.",
        included_docs: &[
            "guide/language-readme",
            "docs/syntax-reference",
            "article/feature-flags-live-in-packages",
        ],
    },
];

include!(concat!(env!("OUT_DIR"), "/embedded_docs.rs"));

pub fn docs_list_command() -> Result<(), CliError> {
    output_docs_success(
        LIST_COMMAND,
        json!({
            "documents": corpus()
                .documents
                .iter()
                .map(document_summary_json)
                .collect::<Vec<_>>()
        }),
    );
    Ok(())
}

pub fn docs_search_command(query: &str) -> Result<(), CliError> {
    match search_documents(query) {
        Ok(results) => {
            output_docs_success(
                SEARCH_COMMAND,
                json!({
                    "query": query.trim(),
                    "results": results
                }),
            );
            Ok(())
        }
        Err(error) => {
            output_docs_error(SEARCH_COMMAND, &error);
            Err(CliError::Reported(1))
        }
    }
}

pub fn docs_show_command(
    doc_id: &str,
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> Result<(), CliError> {
    match show_document(doc_id, start_line, end_line) {
        Ok(document) => {
            output_docs_success(SHOW_COMMAND, document);
            Ok(())
        }
        Err(error) => {
            output_docs_error(SHOW_COMMAND, &error);
            Err(CliError::Reported(1))
        }
    }
}

pub fn docs_context_command(list: bool, id: Option<&str>) -> Result<(), CliError> {
    let result = if list {
        Ok(json!({
            "contexts": CONTEXTS.iter().map(|context| {
                json!({
                    "id": context.id,
                    "title": context.title,
                    "description": context.description,
                    "includedDocs": context.included_docs,
                })
            }).collect::<Vec<_>>()
        }))
    } else if let Some(id) = id {
        context_json(id)
    } else {
        Err(DocsError::InvalidContextRequest)
    };

    match result {
        Ok(data) => {
            output_docs_success(CONTEXT_COMMAND, data);
            Ok(())
        }
        Err(error) => {
            output_docs_error(CONTEXT_COMMAND, &error);
            Err(CliError::Reported(1))
        }
    }
}

fn corpus() -> &'static DocsCorpus {
    CORPUS.get_or_init(|| {
        let documents = EMBEDDED_DOC_SPECS
            .iter()
            .map(|spec| EmbeddedDocument {
                doc_id: spec.doc_id,
                kind: spec.kind,
                title: spec.title,
                path: spec.path,
                description: spec.description,
                lines: split_lines_with_sections(spec.kind, spec.text),
            })
            .collect::<Vec<_>>();

        let index_by_id = documents
            .iter()
            .enumerate()
            .map(|(index, document)| (document.doc_id, index))
            .collect::<HashMap<_, _>>();

        DocsCorpus {
            documents,
            index_by_id,
        }
    })
}

fn search_documents(query: &str) -> Result<Vec<serde_json::Value>, DocsError> {
    let normalized_query = query.trim().to_lowercase();
    let terms = normalized_query
        .split_ascii_whitespace()
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();

    if terms.is_empty() {
        return Err(DocsError::BlankQuery);
    }

    let mut results = Vec::new();

    for document in &corpus().documents {
        for (line_index, line) in document.lines.iter().enumerate() {
            let line_index: usize = line_index;
            let normalized_line = line.text.to_lowercase();
            if !terms.iter().all(|term| normalized_line.contains(term)) {
                continue;
            }

            let before_start = line_index.saturating_sub(2);
            let before = document.lines[before_start..line_index]
                .iter()
                .map(window_line_json)
                .collect::<Vec<_>>();
            let after_end = usize::min(line_index + 3, document.lines.len());
            let after = document.lines[(line_index + 1)..after_end]
                .iter()
                .map(window_line_json)
                .collect::<Vec<_>>();
            let is_exact_phrase = normalized_line.contains(&normalized_query);

            results.push(json!({
                "docId": document.doc_id,
                "kind": document.kind.as_str(),
                "title": document.title,
                "path": document.path,
                "section": line.section,
                "line": line.line,
                "before": before,
                "match": [window_line_json(line)],
                "after": after,
                "isExactPhrase": is_exact_phrase,
            }));
        }
    }

    results.sort_by(|left, right| {
        let left_exact = left["isExactPhrase"].as_bool().unwrap_or(false);
        let right_exact = right["isExactPhrase"].as_bool().unwrap_or(false);
        right_exact
            .cmp(&left_exact)
            .then_with(|| kind_rank(left["kind"].as_str()).cmp(&kind_rank(right["kind"].as_str())))
            .then_with(|| left["docId"].as_str().cmp(&right["docId"].as_str()))
            .then_with(|| left["line"].as_u64().cmp(&right["line"].as_u64()))
    });

    Ok(results)
}

fn show_document(
    doc_id: &str,
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> Result<serde_json::Value, DocsError> {
    let document = document_by_id(doc_id)?;
    let line_count = document.lines.len();

    if matches!(start_line, Some(0)) || matches!(end_line, Some(0)) {
        return Err(DocsError::InvalidLineRange {
            doc_id: doc_id.to_string(),
            line_count,
            start_line,
            end_line,
            reason: "line numbers are 1-based".to_string(),
        });
    }

    let start = start_line.unwrap_or(1);
    if start > line_count {
        return Err(DocsError::InvalidLineRange {
            doc_id: doc_id.to_string(),
            line_count,
            start_line,
            end_line,
            reason: format!("start line {start} exceeds document length {line_count}"),
        });
    }

    let end = usize::min(end_line.unwrap_or(line_count), line_count);
    if start > end {
        return Err(DocsError::InvalidLineRange {
            doc_id: doc_id.to_string(),
            line_count,
            start_line,
            end_line,
            reason: format!("start line {start} exceeds end line {end}"),
        });
    }

    Ok(json!({
        "document": {
            "docId": document.doc_id,
            "kind": document.kind.as_str(),
            "title": document.title,
            "path": document.path,
            "description": document.description,
            "lineCount": line_count,
            "lines": document.lines[(start - 1)..end]
                .iter()
                .map(|line| {
                    json!({
                        "line": line.line,
                        "text": line.text,
                        "section": line.section
                    })
                })
                .collect::<Vec<_>>()
        },
        "range": {
            "startLine": start,
            "endLine": end
        }
    }))
}

fn context_json(id: &str) -> Result<serde_json::Value, DocsError> {
    let context = CONTEXTS
        .iter()
        .find(|context| context.id == id)
        .ok_or_else(|| DocsError::ContextNotFound {
            context_id: id.to_string(),
        })?;

    let documents = context
        .included_docs
        .iter()
        .map(|doc_id| {
            let document = document_by_id(doc_id).map_err(|_| DocsError::InvalidContextReference {
                context_id: context.id,
                doc_id,
            })?;
            Ok(json!({
                "docId": document.doc_id,
                "kind": document.kind.as_str(),
                "title": document.title,
                "path": document.path,
                "description": document.description,
                "lineCount": document.lines.len(),
            }))
        })
        .collect::<Result<Vec<_>, DocsError>>()?;

    Ok(json!({
        "context": {
            "id": context.id,
            "title": context.title,
            "description": context.description,
            "includedDocs": documents
        }
    }))
}

fn document_by_id(doc_id: &str) -> Result<&'static EmbeddedDocument, DocsError> {
    corpus()
        .index_by_id
        .get(doc_id)
        .and_then(|index| corpus().documents.get(*index))
        .ok_or_else(|| DocsError::DocNotFound {
            doc_id: doc_id.to_string(),
        })
}

fn document_summary_json(document: &EmbeddedDocument) -> serde_json::Value {
    json!({
        "docId": document.doc_id,
        "kind": document.kind.as_str(),
        "title": document.title,
        "path": document.path,
        "description": document.description,
        "lineCount": document.lines.len(),
    })
}

fn window_line_json(line: &ProcessedLine) -> serde_json::Value {
    json!({
        "line": line.line,
        "text": line.text,
    })
}

fn kind_rank(kind: Option<&str>) -> u8 {
    match kind {
        Some("guide") => DocKind::Guide.search_rank(),
        Some("docs") => DocKind::Docs.search_rank(),
        Some("spec") => DocKind::Spec.search_rank(),
        Some("article") => DocKind::Article.search_rank(),
        _ => u8::MAX,
    }
}

fn output_docs_success(command: &str, data: serde_json::Value) {
    output_json_value(
        &json!({
            "formatVersion": 1,
            "command": command,
            "ok": true,
            "phase": "docs",
            "data": data,
        }),
        false,
    );
}

fn output_docs_error(command: &str, error: &DocsError) {
    match error {
        DocsError::BlankQuery => output_json_error(
            command,
            "cli",
            codes::cli::USAGE,
            "sigil docs search requires a non-blank query",
            json!({ "query": "" }),
        ),
        DocsError::DocNotFound { doc_id } => output_json_error(
            command,
            "cli",
            codes::cli::DOC_NOT_FOUND,
            &format!("embedded docs do not contain `{doc_id}`"),
            json!({ "docId": doc_id }),
        ),
        DocsError::ContextNotFound { context_id } => output_json_error(
            command,
            "cli",
            codes::cli::DOC_CONTEXT_NOT_FOUND,
            &format!("embedded docs do not contain context `{context_id}`"),
            json!({ "contextId": context_id }),
        ),
        DocsError::InvalidLineRange {
            doc_id,
            line_count,
            start_line,
            end_line,
            reason,
        } => output_json_error(
            command,
            "cli",
            codes::cli::DOC_INVALID_LINE_RANGE,
            &format!("invalid line range for `{doc_id}`: {reason}"),
            json!({
                "docId": doc_id,
                "lineCount": line_count,
                "startLine": start_line,
                "endLine": end_line,
            }),
        ),
        DocsError::InvalidContextRequest => output_json_error(
            command,
            "cli",
            codes::cli::USAGE,
            "sigil docs context expects either --list or one context id",
            json!({}),
        ),
        DocsError::InvalidContextReference { context_id, doc_id } => output_json_error(
            command,
            "cli",
            codes::cli::UNEXPECTED,
            &format!(
                "embedded docs context `{context_id}` references missing doc `{doc_id}`"
            ),
            json!({
                "contextId": context_id,
                "docId": doc_id,
            }),
        ),
    }
}
