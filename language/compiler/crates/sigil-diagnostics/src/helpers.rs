use crate::types::{Diagnostic, Fixit, SourcePoint, SourceSpan, Suggestion, SymbolTarget};

/// Create a basic diagnostic with code, phase, and message
pub fn diagnostic(
    code: impl Into<String>,
    phase: crate::types::SigilPhase,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::new(code, phase, message)
}

/// Helper to create a SourcePoint
pub fn source_point(line: usize, column: usize) -> SourcePoint {
    SourcePoint::new(line, column)
}

/// Helper to create a SourcePoint with offset
pub fn source_point_with_offset(line: usize, column: usize, offset: usize) -> SourcePoint {
    SourcePoint::with_offset(line, column, offset)
}

/// Helper to create a SourceSpan
pub fn source_span(file: impl Into<String>, start: SourcePoint) -> SourceSpan {
    SourceSpan::new(file.into(), start)
}

/// Helper to create a SourceSpan with end
pub fn source_span_with_end(
    file: impl Into<String>,
    start: SourcePoint,
    end: SourcePoint,
) -> SourceSpan {
    SourceSpan::with_end(file.into(), start, end)
}

/// Create a "replace symbol" suggestion
pub fn suggest_replace_symbol(
    message: impl Into<String>,
    replacement: impl Into<String>,
    target: Option<SymbolTarget>,
) -> Suggestion {
    Suggestion::ReplaceSymbol {
        message: message.into(),
        replacement: replacement.into(),
        target,
    }
}

/// Create an "export member" suggestion
pub fn suggest_export_member(
    message: impl Into<String>,
    member: Option<String>,
    target_file: Option<String>,
) -> Suggestion {
    Suggestion::ExportMember {
        message: message.into(),
        target_file,
        member,
    }
}

/// Create a "use operator" suggestion
pub fn suggest_use_operator(
    message: impl Into<String>,
    operator: impl Into<String>,
    replaces: Option<String>,
) -> Suggestion {
    Suggestion::UseOperator {
        message: message.into(),
        operator: operator.into(),
        replaces,
    }
}

/// Create a "reorder declaration" suggestion
pub fn suggest_reorder_declaration(
    message: impl Into<String>,
    category: Option<String>,
    name: Option<String>,
    before: Option<String>,
) -> Suggestion {
    Suggestion::ReorderDeclaration {
        message: message.into(),
        category,
        name,
        before,
    }
}

/// Create a generic suggestion
pub fn suggest_generic(message: impl Into<String>, action: Option<String>) -> Suggestion {
    Suggestion::Generic {
        message: message.into(),
        action,
    }
}

/// Create a replace fixit
pub fn fixit_replace(range: SourceSpan, text: impl Into<String>) -> Fixit {
    Fixit::Replace {
        range,
        text: Some(text.into()),
    }
}

/// Create an insert fixit
pub fn fixit_insert(range: SourceSpan, text: impl Into<String>) -> Fixit {
    Fixit::Insert {
        range,
        text: Some(text.into()),
    }
}

/// Create a delete fixit
pub fn fixit_delete(range: SourceSpan) -> Fixit {
    Fixit::Delete { range }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SigilPhase;

    #[test]
    fn test_diagnostic_creation() {
        let diag = diagnostic("TEST-CODE", SigilPhase::Lexer, "test message");
        assert_eq!(diag.code, "TEST-CODE");
        assert_eq!(diag.phase, SigilPhase::Lexer);
        assert_eq!(diag.message, "test message");
    }

    #[test]
    fn test_source_span_format_location() {
        let span = source_span_with_end(
            "test.sigil",
            source_point(10, 5),
            source_point(10, 15),
        );
        assert_eq!(span.format_location(), "test.sigil:10:5");
    }

    #[test]
    fn test_diagnostic_format_human() {
        let diag = diagnostic("SIGIL-TEST", SigilPhase::Parser, "test error")
            .with_location(source_span("file.sigil", source_point(1, 1)))
            .with_found_expected("x", "y");

        let formatted = diag.format_human();
        assert!(formatted.contains("SIGIL-TEST"));
        assert!(formatted.contains("file.sigil:1:1"));
        assert!(formatted.contains("test error"));
        assert!(formatted.contains("found"));
        assert!(formatted.contains("expected"));
    }

    #[test]
    fn test_suggestions() {
        let _s1 = suggest_replace_symbol("use dot", "⋅", None);
        let _s2 = suggest_export_member("export this", Some("foo".into()), None);
        let _s3 = suggest_use_operator("use operator", "+", None);
        let _s4 = suggest_reorder_declaration("reorder", None, None, None);
        let _s5 = suggest_generic("fix this", None);
    }

    #[test]
    fn test_fixits() {
        let span = source_span("test.sigil", source_point(1, 1));
        let _f1 = fixit_replace(span.clone(), "new text");
        let _f2 = fixit_insert(span.clone(), "inserted");
        let _f3 = fixit_delete(span);
    }
}
