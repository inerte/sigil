//! Expression AST nodes

use crate::{Param, Pattern, SourceLocation, Type};

/// Expressions in Sigil
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type"))]
pub enum Expr {
    Literal(LiteralExpr),
    Identifier(IdentifierExpr),
    Lambda(Box<LambdaExpr>),
    Application(Box<ApplicationExpr>),
    Binary(Box<BinaryExpr>),
    Unary(Box<UnaryExpr>),
    Match(Box<MatchExpr>),
    Let(Box<LetExpr>),
    If(Box<IfExpr>),
    List(ListExpr),
    Record(RecordExpr),
    Tuple(TupleExpr),
    FieldAccess(Box<FieldAccessExpr>),
    Index(Box<IndexExpr>),
    Pipeline(Box<PipelineExpr>),
    Map(Box<MapExpr>),
    Filter(Box<FilterExpr>),
    Fold(Box<FoldExpr>),
    MemberAccess(MemberAccessExpr),
    WithMock(Box<WithMockExpr>),
}

/// Literal expression: 42, 3.14, "hello", 'c', true, false, ()
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LiteralExpr {
    pub value: LiteralValue,
    pub literal_type: LiteralType,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LiteralValue {
    Int(i64),
    Float(f64),
    String(String),
    Char(char),
    Bool(bool),
    Unit,
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
    Add,       // +
    Subtract,  // -
    Multiply,  // *
    Divide,    // /
    Modulo,    // %
    Power,     // ^
    // Comparison
    Equal,     // =
    NotEqual,  // ≠
    Less,      // <
    Greater,   // >
    LessEq,    // ≤
    GreaterEq, // ≥
    // Logical
    And,       // ∧
    Or,        // ∨
    // Pipeline
    Pipe,      // |>
    ComposeFwd, // >>
    ComposeBwd, // <<
    // Concatenation
    Append,    // ++
    ListAppend, // ⧺
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
    Negate,  // -
    Not,     // ¬
    Length,  // # (list length)
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
    pub then_branch: Expr,
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
    Pipe,       // |>
    ComposeFwd, // >>
    ComposeBwd, // <<
}

/// Map operation: list ↦ fn
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MapExpr {
    pub list: Expr,
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
