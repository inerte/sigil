use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SigilPhase {
    Cli,
    Io,
    Surface,
    Lexer,
    Parser,
    Canonical,
    Typecheck,
    Mutability,
    Extern,
    Codegen,
    Runtime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePoint {
    pub line: usize,
    pub column: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,
}

impl SourcePoint {
    pub fn new(line: usize, column: usize) -> Self {
        Self {
            line,
            column,
            offset: None,
        }
    }

    pub fn with_offset(line: usize, column: usize, offset: usize) -> Self {
        Self {
            line,
            column,
            offset: Some(offset),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub file: String,
    pub start: SourcePoint,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<SourcePoint>,
}

impl SourceSpan {
    pub fn new(file: String, start: SourcePoint) -> Self {
        Self {
            file,
            start,
            end: None,
        }
    }

    pub fn with_end(file: String, start: SourcePoint, end: SourcePoint) -> Self {
        Self {
            file,
            start,
            end: Some(end),
        }
    }

    /// Format location as "file:line:column"
    pub fn format_location(&self) -> String {
        format!("{}:{}:{}", self.file, self.start.line, self.start.column)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Fixit {
    Replace {
        range: SourceSpan,
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    Insert {
        range: SourceSpan,
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    Delete {
        range: SourceSpan,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Suggestion {
    ReplaceSymbol {
        message: String,
        replacement: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        target: Option<SymbolTarget>,
    },
    ExportMember {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        target_file: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        member: Option<String>,
    },
    UseOperator {
        message: String,
        operator: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        replaces: Option<String>,
    },
    ReorderDeclaration {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        category: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        before: Option<String>,
    },
    Generic {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        action: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolTarget {
    NamespaceSeparator,
    LocalBindingKeyword,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub phase: SigilPhase,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<SourceSpan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub found: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixits: Option<Vec<Fixit>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestions: Option<Vec<Suggestion>>,
}

impl Diagnostic {
    /// Create a new diagnostic with code, phase, and message
    pub fn new(code: impl Into<String>, phase: SigilPhase, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            phase,
            message: message.into(),
            location: None,
            found: None,
            expected: None,
            details: None,
            fixits: None,
            suggestions: None,
        }
    }

    /// Set the location
    pub fn with_location(mut self, location: SourceSpan) -> Self {
        self.location = Some(location);
        self
    }

    /// Set found and expected values
    pub fn with_found_expected(
        mut self,
        found: impl Serialize,
        expected: impl Serialize,
    ) -> Self {
        self.found = serde_json::to_value(found).ok();
        self.expected = serde_json::to_value(expected).ok();
        self
    }

    /// Set found value only
    pub fn with_found(mut self, found: impl Serialize) -> Self {
        self.found = serde_json::to_value(found).ok();
        self
    }

    /// Set expected value only
    pub fn with_expected(mut self, expected: impl Serialize) -> Self {
        self.expected = serde_json::to_value(expected).ok();
        self
    }

    /// Add details
    pub fn with_details(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        let details = self.details.get_or_insert_with(HashMap::new);
        if let Ok(json_value) = serde_json::to_value(value) {
            details.insert(key.into(), json_value);
        }
        self
    }

    /// Add a suggestion
    pub fn with_suggestion(mut self, suggestion: Suggestion) -> Self {
        self.suggestions.get_or_insert_with(Vec::new).push(suggestion);
        self
    }

    /// Add a fixit
    pub fn with_fixit(mut self, fixit: Fixit) -> Self {
        self.fixits.get_or_insert_with(Vec::new).push(fixit);
        self
    }

    /// Format for human-readable output
    /// Format: "CODE file:line:col message (found X, expected Y)"
    pub fn format_human(&self) -> String {
        let mut parts = vec![self.code.clone()];

        if let Some(loc) = &self.location {
            parts.push(loc.format_location());
        }

        parts.push(self.message.clone());

        if self.found.is_some() || self.expected.is_some() {
            let found = self
                .found
                .as_ref()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "?".into());
            let expected = self
                .expected
                .as_ref()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "?".into());
            parts.push(format!("(found {}, expected {})", found, expected));
        }

        parts.join(" ")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandEnvelope<T = serde_json::Value> {
    #[serde(rename = "formatVersion")]
    pub format_version: u8,
    pub command: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<SigilPhase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Diagnostic>,
}

impl<T> CommandEnvelope<T> {
    pub fn success(command: impl Into<String>, data: T) -> Self {
        Self {
            format_version: 1,
            command: command.into(),
            ok: true,
            phase: None,
            data: Some(data),
            error: None,
        }
    }

    pub fn failure(command: impl Into<String>, error: Diagnostic) -> Self {
        Self {
            format_version: 1,
            command: command.into(),
            ok: false,
            phase: Some(error.phase),
            data: None,
            error: Some(error),
        }
    }

    pub fn format_human(&self) -> String {
        if self.ok {
            let mut parts = vec![format!("{} OK", self.command)];
            if let Some(phase) = self.phase {
                parts.push(format!("phase={:?}", phase).to_lowercase());
            }
            parts.join(" ")
        } else if let Some(err) = &self.error {
            err.format_human()
        } else {
            format!("{} FAIL", self.command)
        }
    }
}
