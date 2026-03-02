//! Expression AST nodes

use crate::{Param, Pattern, SourceLocation, Type};

/// Expressions in Sigil
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type"))]
pub enum Expr {
    #[cfg_attr(feature = "serde", serde(rename = "LiteralExpr"))]
    Literal(LiteralExpr),
    #[cfg_attr(feature = "serde", serde(rename = "IdentifierExpr"))]
    Identifier(IdentifierExpr),
    #[cfg_attr(feature = "serde", serde(rename = "LambdaExpr"))]
    Lambda(Box<LambdaExpr>),
    #[cfg_attr(feature = "serde", serde(rename = "ApplicationExpr"))]
    Application(Box<ApplicationExpr>),
    #[cfg_attr(feature = "serde", serde(rename = "BinaryExpr"))]
    Binary(Box<BinaryExpr>),
    #[cfg_attr(feature = "serde", serde(rename = "UnaryExpr"))]
    Unary(Box<UnaryExpr>),
    #[cfg_attr(feature = "serde", serde(rename = "MatchExpr"))]
    Match(Box<MatchExpr>),
    #[cfg_attr(feature = "serde", serde(rename = "LetExpr"))]
    Let(Box<LetExpr>),
    #[cfg_attr(feature = "serde", serde(rename = "IfExpr"))]
    If(Box<IfExpr>),
    #[cfg_attr(feature = "serde", serde(rename = "ListExpr"))]
    List(ListExpr),
    #[cfg_attr(feature = "serde", serde(rename = "RecordExpr"))]
    Record(RecordExpr),
    #[cfg_attr(feature = "serde", serde(rename = "TupleExpr"))]
    Tuple(TupleExpr),
    #[cfg_attr(feature = "serde", serde(rename = "FieldAccessExpr"))]
    FieldAccess(Box<FieldAccessExpr>),
    #[cfg_attr(feature = "serde", serde(rename = "IndexExpr"))]
    Index(Box<IndexExpr>),
    #[cfg_attr(feature = "serde", serde(rename = "PipelineExpr"))]
    Pipeline(Box<PipelineExpr>),
    #[cfg_attr(feature = "serde", serde(rename = "MapExpr"))]
    Map(Box<MapExpr>),
    #[cfg_attr(feature = "serde", serde(rename = "FilterExpr"))]
    Filter(Box<FilterExpr>),
    #[cfg_attr(feature = "serde", serde(rename = "FoldExpr"))]
    Fold(Box<FoldExpr>),
    #[cfg_attr(feature = "serde", serde(rename = "MemberAccessExpr"))]
    MemberAccess(MemberAccessExpr),
    #[cfg_attr(feature = "serde", serde(rename = "WithMockExpr"))]
    WithMock(Box<WithMockExpr>),
    #[cfg_attr(feature = "serde", serde(rename = "TypeAscriptionExpr"))]
    TypeAscription(Box<TypeAscriptionExpr>),
}

/// Literal expression: 42, 3.14, "hello", 'c', true, false, ()
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LiteralExpr {
    #[cfg_attr(feature = "serde", serde(serialize_with = "serialize_literal_value"))]
    #[cfg_attr(feature = "serde", serde(deserialize_with = "deserialize_literal_value"))]
    pub value: LiteralValue,
    #[cfg_attr(feature = "serde", serde(rename = "literalType"))]
    pub literal_type: LiteralType,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LiteralValue {
    Int(i64),
    Float(f64),
    String(String),
    Char(char),
    Bool(bool),
    Unit,
}

#[cfg(feature = "serde")]
fn serialize_literal_value<S>(value: &LiteralValue, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match value {
        LiteralValue::Int(i) => serializer.serialize_i64(*i),
        LiteralValue::Float(f) => serializer.serialize_f64(*f),
        LiteralValue::String(s) => serializer.serialize_str(s),
        LiteralValue::Char(c) => serializer.serialize_str(&c.to_string()),
        LiteralValue::Bool(b) => serializer.serialize_bool(*b),
        LiteralValue::Unit => serializer.serialize_none(),
    }
}

#[cfg(feature = "serde")]
fn deserialize_literal_value<'de, D>(deserializer: D) -> Result<LiteralValue, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Visitor;
    use std::fmt;

    struct LiteralValueVisitor;

    impl<'de> Visitor<'de> for LiteralValueVisitor {
        type Value = LiteralValue;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a number, string, boolean, or null")
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(LiteralValue::Int(value))
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(LiteralValue::Int(value as i64))
        }

        fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(LiteralValue::Float(value))
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if value.len() == 1 {
                Ok(LiteralValue::Char(value.chars().next().unwrap()))
            } else {
                Ok(LiteralValue::String(value.to_string()))
            }
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if value.len() == 1 {
                Ok(LiteralValue::Char(value.chars().next().unwrap()))
            } else {
                Ok(LiteralValue::String(value))
            }
        }

        fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(LiteralValue::Bool(value))
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(LiteralValue::Unit)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(LiteralValue::Unit)
        }
    }

    deserializer.deserialize_any(LiteralValueVisitor)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LiteralType {
    Int,
    Float,
    String,
    Char,
    Bool,
    Unit,
}

/// Identifier expression: x, foo, _temp
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IdentifierExpr {
    pub name: String,
    pub location: SourceLocation,
}

/// Lambda expression: λ (x: Int) → Int { x + 1 }
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LambdaExpr {
    pub params: Vec<Param>,
    pub effects: Vec<String>,     // Effect annotations: ['IO', 'Network', 'Async', 'Error', 'Mut']
    #[cfg_attr(feature = "serde", serde(rename = "returnType"))]
    pub return_type: Type,         // Mandatory (canonical form)
    pub body: Expr,
    pub location: SourceLocation,
}

/// Function application: f(x, y)
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ApplicationExpr {
    pub func: Expr,
    pub args: Vec<Expr>,
    pub location: SourceLocation,
}

/// Binary expression: x + y, a && b
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BinaryExpr {
    pub left: Expr,
    pub operator: BinaryOperator,
    pub right: Expr,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BinaryOperator {
    // Arithmetic
    #[cfg_attr(feature = "serde", serde(rename = "+"))]
    Add,
    #[cfg_attr(feature = "serde", serde(rename = "-"))]
    Subtract,
    #[cfg_attr(feature = "serde", serde(rename = "*"))]
    Multiply,
    #[cfg_attr(feature = "serde", serde(rename = "/"))]
    Divide,
    #[cfg_attr(feature = "serde", serde(rename = "%"))]
    Modulo,
    #[cfg_attr(feature = "serde", serde(rename = "^"))]
    Power,
    // Comparison
    #[cfg_attr(feature = "serde", serde(rename = "="))]
    Equal,
    #[cfg_attr(feature = "serde", serde(rename = "≠"))]
    NotEqual,
    #[cfg_attr(feature = "serde", serde(rename = "<"))]
    Less,
    #[cfg_attr(feature = "serde", serde(rename = ">"))]
    Greater,
    #[cfg_attr(feature = "serde", serde(rename = "≤"))]
    LessEq,
    #[cfg_attr(feature = "serde", serde(rename = "≥"))]
    GreaterEq,
    // Logical
    #[cfg_attr(feature = "serde", serde(rename = "∧"))]
    And,
    #[cfg_attr(feature = "serde", serde(rename = "∨"))]
    Or,
    // Pipeline
    #[cfg_attr(feature = "serde", serde(rename = "|>"))]
    Pipe,
    #[cfg_attr(feature = "serde", serde(rename = ">>"))]
    ComposeFwd,
    #[cfg_attr(feature = "serde", serde(rename = "<<"))]
    ComposeBwd,
    // Concatenation
    #[cfg_attr(feature = "serde", serde(rename = "++"))]
    Append,
    #[cfg_attr(feature = "serde", serde(rename = "⧺"))]
    ListAppend,
}

impl std::fmt::Display for BinaryOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            BinaryOperator::Add => "+",
            BinaryOperator::Subtract => "-",
            BinaryOperator::Multiply => "*",
            BinaryOperator::Divide => "/",
            BinaryOperator::Modulo => "%",
            BinaryOperator::Power => "^",
            BinaryOperator::Equal => "=",
            BinaryOperator::NotEqual => "≠",
            BinaryOperator::Less => "<",
            BinaryOperator::Greater => ">",
            BinaryOperator::LessEq => "≤",
            BinaryOperator::GreaterEq => "≥",
            BinaryOperator::And => "∧",
            BinaryOperator::Or => "∨",
            BinaryOperator::Pipe => "|>",
            BinaryOperator::ComposeFwd => ">>",
            BinaryOperator::ComposeBwd => "<<",
            BinaryOperator::Append => "++",
            BinaryOperator::ListAppend => "⧺",
        };
        write!(f, "{}", s)
    }
}

/// Unary expression: -x, ¬b, #list
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnaryExpr {
    pub operator: UnaryOperator,
    pub operand: Expr,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum UnaryOperator {
    #[cfg_attr(feature = "serde", serde(rename = "-"))]
    Negate,
    #[cfg_attr(feature = "serde", serde(rename = "¬"))]
    Not,
    #[cfg_attr(feature = "serde", serde(rename = "#"))]
    Length,
}

impl std::fmt::Display for UnaryOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            UnaryOperator::Negate => "-",
            UnaryOperator::Not => "¬",
            UnaryOperator::Length => "#",
        };
        write!(f, "{}", s)
    }
}

/// Match expression: value ≡ Some x → x | None → 0
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MatchExpr {
    pub scrutinee: Expr,
    pub arms: Vec<MatchArm>,
    pub location: SourceLocation,
}

/// Match arm: pattern → body or pattern when guard → body
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,  // Optional pattern guard: when boolean_expr
    pub body: Expr,
    pub location: SourceLocation,
}

/// Let binding: l x = 5 { x + 10 }
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LetExpr {
    pub pattern: Pattern,
    pub value: Expr,
    pub body: Expr,
    pub location: SourceLocation,
}

/// If expression: x > 0 ? x : -x
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IfExpr {
    pub condition: Expr,
    #[cfg_attr(feature = "serde", serde(rename = "thenBranch"))]
    pub then_branch: Expr,
    #[cfg_attr(feature = "serde", serde(rename = "elseBranch"))]
    pub else_branch: Option<Expr>,
    pub location: SourceLocation,
}

/// List expression: [1, 2, 3]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListExpr {
    pub elements: Vec<Expr>,
    pub location: SourceLocation,
}

/// Record expression: { x: 10, y: 20 }
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RecordExpr {
    pub fields: Vec<RecordField>,
    pub location: SourceLocation,
}

/// Record field: name: value
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RecordField {
    pub name: String,
    pub value: Expr,
    pub location: SourceLocation,
}

/// Tuple expression: (1, "hello", true)
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TupleExpr {
    pub elements: Vec<Expr>,
    pub location: SourceLocation,
}

/// Field access: record.field
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FieldAccessExpr {
    pub object: Expr,
    pub field: String,
    pub location: SourceLocation,
}

/// Index expression: list[0]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IndexExpr {
    pub object: Expr,
    pub index: Expr,
    pub location: SourceLocation,
}

/// Pipeline expression: x |> f or f >> g
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PipelineExpr {
    pub left: Expr,
    pub operator: PipelineOperator,
    pub right: Expr,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PipelineOperator {
    #[cfg_attr(feature = "serde", serde(rename = "|>"))]
    Pipe,
    #[cfg_attr(feature = "serde", serde(rename = ">>"))]
    ComposeFwd,
    #[cfg_attr(feature = "serde", serde(rename = "<<"))]
    ComposeBwd,
}

/// Map operation: list ↦ fn
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MapExpr {
    pub list: Expr,
    #[cfg_attr(feature = "serde", serde(rename = "fn"))]
    pub func: Expr,
    pub location: SourceLocation,
}

/// Filter operation: list ⊳ predicate
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FilterExpr {
    pub list: Expr,
    pub predicate: Expr,
    pub location: SourceLocation,
}

/// Fold operation: list ⊕ fn init
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FoldExpr {
    pub list: Expr,
    #[cfg_attr(feature = "serde", serde(rename = "fn"))]
    pub func: Expr,
    pub init: Expr,
    pub location: SourceLocation,
}

/// Member access (FFI): fs⋅promises.readFile
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MemberAccessExpr {
    pub namespace: Vec<String>,    // ['fs', 'promises'] or ['axios'] (Sigil syntax: fs⋅promises)
    pub member: String,             // 'readFile' or 'get'
    pub location: SourceLocation,
}

/// With mock expression: with_mock target replacement { body }
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WithMockExpr {
    pub target: Expr,
    pub replacement: Expr,
    pub body: Expr,
    pub location: SourceLocation,
}

/// Type ascription expression: (expr:Type)
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TypeAscriptionExpr {
    pub expr: Expr,
    #[cfg_attr(feature = "serde", serde(rename = "ascribedType"))]
    pub ascribed_type: Type,
    pub location: SourceLocation,
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;
    use crate::{Type, PrimitiveType, PrimitiveName};
    use sigil_lexer::{Position, SourceLocation};

    fn test_loc() -> SourceLocation {
        SourceLocation::new(
            Position::new(1, 1, 0),
            Position::new(1, 10, 10),
        )
    }

    fn int_type() -> Type {
        Type::Primitive(PrimitiveType {
            name: PrimitiveName::Int,
            location: test_loc(),
        })
    }

    #[test]
    fn test_literal_expr_json_format() {
        // Test integer literal
        let int_expr = LiteralExpr {
            value: LiteralValue::Int(42),
            literal_type: LiteralType::Int,
            location: test_loc(),
        };

        let json = serde_json::to_value(&int_expr).unwrap();
        assert_eq!(json["value"], 42);
        assert_eq!(json["literalType"], "Int");

        // Test string literal
        let str_expr = LiteralExpr {
            value: LiteralValue::String("hello".to_string()),
            literal_type: LiteralType::String,
            location: test_loc(),
        };

        let json = serde_json::to_value(&str_expr).unwrap();
        assert_eq!(json["value"], "hello");
        assert_eq!(json["literalType"], "String");

        // Test boolean literal
        let bool_expr = LiteralExpr {
            value: LiteralValue::Bool(true),
            literal_type: LiteralType::Bool,
            location: test_loc(),
        };

        let json = serde_json::to_value(&bool_expr).unwrap();
        assert_eq!(json["value"], true);
        assert_eq!(json["literalType"], "Bool");

        // Test unit literal
        let unit_expr = LiteralExpr {
            value: LiteralValue::Unit,
            literal_type: LiteralType::Unit,
            location: test_loc(),
        };

        let json = serde_json::to_value(&unit_expr).unwrap();
        assert!(json["value"].is_null());
        assert_eq!(json["literalType"], "Unit");
    }

    #[test]
    fn test_expr_variant_names() {
        // Test that Expr enum variants serialize with correct type names
        let literal = Expr::Literal(LiteralExpr {
            value: LiteralValue::Int(42),
            literal_type: LiteralType::Int,
            location: test_loc(),
        });

        let json = serde_json::to_value(&literal).unwrap();
        assert_eq!(json["type"], "LiteralExpr");

        let identifier = Expr::Identifier(IdentifierExpr {
            name: "x".to_string(),
            location: test_loc(),
        });

        let json = serde_json::to_value(&identifier).unwrap();
        assert_eq!(json["type"], "IdentifierExpr");
    }

    #[test]
    fn test_binary_operator_serialization() {
        // Test that operators serialize as symbols, not variant names
        let add_expr = BinaryExpr {
            left: Expr::Literal(LiteralExpr {
                value: LiteralValue::Int(1),
                literal_type: LiteralType::Int,
                location: test_loc(),
            }),
            operator: BinaryOperator::Add,
            right: Expr::Literal(LiteralExpr {
                value: LiteralValue::Int(2),
                literal_type: LiteralType::Int,
                location: test_loc(),
            }),
            location: test_loc(),
        };

        let json = serde_json::to_value(&add_expr).unwrap();
        assert_eq!(json["operator"], "+");
    }

    #[test]
    fn test_camel_case_field_names() {
        // Test that snake_case Rust fields serialize as camelCase JSON
        let lambda = LambdaExpr {
            params: vec![],
            effects: vec![],
            return_type: int_type(),
            body: Expr::Literal(LiteralExpr {
                value: LiteralValue::Int(42),
                literal_type: LiteralType::Int,
                location: test_loc(),
            }),
            location: test_loc(),
        };

        let json = serde_json::to_value(&lambda).unwrap();
        assert!(json.get("returnType").is_some(), "Field should be 'returnType' not 'return_type'");
        assert!(json.get("return_type").is_none(), "Field should not be 'return_type'");

        let if_expr = IfExpr {
            condition: Expr::Literal(LiteralExpr {
                value: LiteralValue::Bool(true),
                literal_type: LiteralType::Bool,
                location: test_loc(),
            }),
            then_branch: Expr::Literal(LiteralExpr {
                value: LiteralValue::Int(1),
                literal_type: LiteralType::Int,
                location: test_loc(),
            }),
            else_branch: Some(Expr::Literal(LiteralExpr {
                value: LiteralValue::Int(2),
                literal_type: LiteralType::Int,
                location: test_loc(),
            })),
            location: test_loc(),
        };

        let json = serde_json::to_value(&if_expr).unwrap();
        assert!(json.get("thenBranch").is_some(), "Field should be 'thenBranch' not 'then_branch'");
        assert!(json.get("elseBranch").is_some(), "Field should be 'elseBranch' not 'else_branch'");
    }

    #[test]
    fn test_map_expr_fn_field() {
        // Test that MapExpr.func serializes as "fn" in JSON (matching TypeScript)
        let map_expr = MapExpr {
            list: Expr::Identifier(IdentifierExpr {
                name: "list".to_string(),
                location: test_loc(),
            }),
            func: Expr::Identifier(IdentifierExpr {
                name: "fn".to_string(),
                location: test_loc(),
            }),
            location: test_loc(),
        };

        let json = serde_json::to_value(&map_expr).unwrap();
        assert!(json.get("fn").is_some(), "Field should be 'fn' not 'func'");
        assert!(json.get("func").is_none(), "Field should not be 'func'");
    }
}
