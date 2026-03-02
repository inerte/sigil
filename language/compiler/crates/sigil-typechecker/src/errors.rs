//! Sigil Type Checker - Error Reporting
//!
//! Type error messages optimized for clarity (both for LLMs and humans)

use crate::types::{prune, InferenceType, TPrimitive, TVar};
use sigil_ast::PrimitiveName;
use sigil_diagnostics::{codes, Diagnostic, SigilPhase, SourcePoint, SourceSpan};
use sigil_lexer::SourceLocation;
use thiserror::Error;

/// Type error with source location information
#[derive(Debug, Error)]
#[error("{message}")]
pub struct TypeError {
    pub message: String,
    pub location: Option<SourceLocation>,
    pub expected: Option<InferenceType>,
    pub actual: Option<InferenceType>,
}

impl TypeError {
    /// Create a new type error
    pub fn new(message: String, location: Option<SourceLocation>) -> Self {
        Self {
            message,
            location,
            expected: None,
            actual: None,
        }
    }

    /// Create a type error with expected/actual types
    pub fn mismatch(
        message: String,
        location: Option<SourceLocation>,
        expected: InferenceType,
        actual: InferenceType,
    ) -> Self {
        Self {
            message,
            location,
            expected: Some(expected),
            actual: Some(actual),
        }
    }

    /// Format error message with source context
    pub fn format(&self, source_code: Option<&str>) -> String {
        let mut output = format!("Type Error: {}\n", self.message);

        // Show source location context
        if let (Some(loc), Some(code)) = (&self.location, source_code) {
            let lines: Vec<&str> = code.split('\n').collect();
            if let Some(line) = lines.get(loc.start.line - 1) {
                output.push('\n');
                output.push_str(&format!("  {} | {}\n", loc.start.line, line));

                // Add caret pointing to error location
                let line_num_str = loc.start.line.to_string();
                let padding = " ".repeat(line_num_str.len() + 3 + loc.start.column);
                output.push_str(&format!("  {} | {}^\n", " ".repeat(line_num_str.len()), padding));
            }
        }

        // Show expected vs actual types
        if let (Some(ref exp), Some(ref act)) = (&self.expected, &self.actual) {
            output.push('\n');
            output.push_str(&format!("Expected: {}\n", format_type(exp)));
            output.push_str(&format!("Actual:   {}\n", format_type(act)));
        }

        output
    }
}

/// Format a type for display in error messages
///
/// Uses Sigil Unicode symbols (ℤ, 𝔹, 𝕊) for readability
pub fn format_type(typ: &InferenceType) -> String {
    // Follow instances (dereferencing)
    let typ = prune(typ);

    match &typ {
        InferenceType::Primitive(p) => {
            // Use Sigil Unicode symbols
            match p.name {
                PrimitiveName::Int => "ℤ".to_string(),
                PrimitiveName::Float => "ℝ".to_string(),
                PrimitiveName::Bool => "𝔹".to_string(),
                PrimitiveName::String => "𝕊".to_string(),
                PrimitiveName::Char => "ℂ".to_string(),
                PrimitiveName::Unit => "𝕌".to_string(),
            }
        }

        InferenceType::Var(tvar) => {
            // Use Greek letters for type variables
            tvar.name.clone().unwrap_or_else(|| format!("α{}", tvar.id))
        }

        InferenceType::Function(tfunc) => {
            let params = tfunc
                .params
                .iter()
                .map(format_type)
                .collect::<Vec<_>>()
                .join(", ");
            let ret = format_type(&tfunc.return_type);

            // Use Sigil syntax: (T1, T2) → R
            if let Some(ref effects) = tfunc.effects {
                let effect_str = effects
                    .iter()
                    .map(|e| format!("!{}", e))
                    .collect::<Vec<_>>()
                    .join("");
                format!("({}) →{} {}", params, effect_str, ret)
            } else {
                format!("({}) → {}", params, ret)
            }
        }

        InferenceType::List(tlist) => {
            format!("[{}]", format_type(&tlist.element_type))
        }

        InferenceType::Tuple(ttuple) => {
            let types = ttuple
                .types
                .iter()
                .map(format_type)
                .collect::<Vec<_>>()
                .join(", ");
            format!("({})", types)
        }

        InferenceType::Record(trec) => {
            if let Some(ref name) = trec.name {
                name.clone()
            } else {
                let fields = trec
                    .fields
                    .iter()
                    .map(|(name, typ)| format!("{}: {}", name, format_type(typ)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{{}}}", fields)
            }
        }

        InferenceType::Constructor(tcons) => {
            if tcons.type_args.is_empty() {
                tcons.name.clone()
            } else {
                let args = tcons
                    .type_args
                    .iter()
                    .map(format_type)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}[{}]", tcons.name, args)
            }
        }

        InferenceType::Any => "Any".to_string(),
    }
}

/// Convert SourceLocation from lexer to SourceSpan for diagnostics
fn source_location_to_span(file: String, loc: SourceLocation) -> SourceSpan {
    SourceSpan::with_end(
        file,
        SourcePoint::with_offset(loc.start.line, loc.start.column, loc.start.offset),
        SourcePoint::with_offset(loc.end.line, loc.end.column, loc.end.offset),
    )
}

impl From<TypeError> for Diagnostic {
    fn from(error: TypeError) -> Self {
        let code = codes::typecheck::ERROR;
        let mut diag = Diagnostic::new(code, SigilPhase::Typecheck, error.message.clone());

        if let Some(loc) = error.location {
            diag = diag.with_location(source_location_to_span("<unknown>".into(), loc));
        }

        if let (Some(exp), Some(act)) = (error.expected, error.actual) {
            diag = diag.with_found_expected(format_type(&act), format_type(&exp));
        }

        diag
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_primitive() {
        let int_type = InferenceType::Primitive(TPrimitive {
            name: PrimitiveName::Int,
        });
        assert_eq!(format_type(&int_type), "ℤ");

        let bool_type = InferenceType::Primitive(TPrimitive {
            name: PrimitiveName::Bool,
        });
        assert_eq!(format_type(&bool_type), "𝔹");
    }

    #[test]
    fn test_format_list() {
        let int_type = InferenceType::Primitive(TPrimitive {
            name: PrimitiveName::Int,
        });
        let list_type = InferenceType::List(Box::new(crate::types::TList {
            element_type: int_type,
        }));
        assert_eq!(format_type(&list_type), "[ℤ]");
    }
}
