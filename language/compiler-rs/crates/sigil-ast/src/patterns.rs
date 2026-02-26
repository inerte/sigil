//! Pattern AST nodes for pattern matching

use crate::SourceLocation;

/// Patterns used in match expressions and let bindings
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type"))]
pub enum Pattern {
    Literal(LiteralPattern),
    Identifier(IdentifierPattern),
    Wildcard(WildcardPattern),
    Constructor(ConstructorPattern),
    List(ListPattern),
    Record(RecordPattern),
    Tuple(TuplePattern),
}

/// Literal pattern: 42, "hello", true
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LiteralPattern {
    pub value: PatternLiteralValue,
    pub literal_type: PatternLiteralType,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PatternLiteralValue {
    Int(i64),
    Float(f64),
    String(String),
    Char(char),
    Bool(bool),
    Unit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PatternLiteralType {
    Int,
    Float,
    String,
    Char,
    Bool,
    Unit,
}

/// Identifier pattern: x, result
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IdentifierPattern {
    pub name: String,
    pub location: SourceLocation,
}

/// Wildcard pattern: _
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WildcardPattern {
    pub location: SourceLocation,
}

/// Constructor pattern: Some x, Ok (value, message)
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConstructorPattern {
    pub name: String,
    pub patterns: Vec<Pattern>,
    pub location: SourceLocation,
}

/// List pattern: [x, y, .rest] or []
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListPattern {
    pub patterns: Vec<Pattern>,
    pub rest: Option<String>,  // For [x, .xs] pattern, rest = Some("xs")
    pub location: SourceLocation,
}

/// Record pattern: { x, y: value }
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RecordPattern {
    pub fields: Vec<RecordPatternField>,
    pub location: SourceLocation,
}

/// Record pattern field: name or name: pattern
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RecordPatternField {
    pub name: String,
    pub pattern: Option<Pattern>,  // None means just bind the field name
    pub location: SourceLocation,
}

/// Tuple pattern: (x, y, z)
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TuplePattern {
    pub patterns: Vec<Pattern>,
    pub location: SourceLocation,
}
