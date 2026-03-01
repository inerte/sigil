//! Pattern AST nodes for pattern matching

use crate::SourceLocation;

/// Patterns used in match expressions and let bindings
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type"))]
pub enum Pattern {
    #[cfg_attr(feature = "serde", serde(rename = "LiteralPattern"))]
    Literal(LiteralPattern),
    #[cfg_attr(feature = "serde", serde(rename = "IdentifierPattern"))]
    Identifier(IdentifierPattern),
    #[cfg_attr(feature = "serde", serde(rename = "WildcardPattern"))]
    Wildcard(WildcardPattern),
    #[cfg_attr(feature = "serde", serde(rename = "ConstructorPattern"))]
    Constructor(ConstructorPattern),
    #[cfg_attr(feature = "serde", serde(rename = "ListPattern"))]
    List(ListPattern),
    #[cfg_attr(feature = "serde", serde(rename = "RecordPattern"))]
    Record(RecordPattern),
    #[cfg_attr(feature = "serde", serde(rename = "TuplePattern"))]
    Tuple(TuplePattern),
}

/// Literal pattern: 42, "hello", true
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LiteralPattern {
    #[cfg_attr(feature = "serde", serde(serialize_with = "serialize_pattern_literal_value"))]
    #[cfg_attr(feature = "serde", serde(deserialize_with = "deserialize_pattern_literal_value"))]
    pub value: PatternLiteralValue,
    #[cfg_attr(feature = "serde", serde(rename = "literalType"))]
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
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub enum PatternLiteralType {
    Int,
    Float,
    String,
    Char,
    Bool,
    Unit,
}

// Custom serialization for PatternLiteralValue to match TypeScript format
#[cfg(feature = "serde")]
fn serialize_pattern_literal_value<S>(value: &PatternLiteralValue, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::Serialize;
    match value {
        PatternLiteralValue::Int(n) => n.serialize(serializer),
        PatternLiteralValue::Float(f) => f.serialize(serializer),
        PatternLiteralValue::String(s) => s.serialize(serializer),
        PatternLiteralValue::Char(c) => c.to_string().serialize(serializer),
        PatternLiteralValue::Bool(b) => b.serialize(serializer),
        PatternLiteralValue::Unit => serializer.serialize_none(),
    }
}

#[cfg(feature = "serde")]
fn deserialize_pattern_literal_value<'de, D>(_deserializer: D) -> Result<PatternLiteralValue, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // Simplified deserializer - in practice this would need to handle all value types
    Ok(PatternLiteralValue::Unit)
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
