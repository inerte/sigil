use crate::types::{EffectSet, InferenceType, TypeScheme};
use sigil_ast::{
    BinaryOperator, ExternDecl, IdentifierExpr, ImportDecl, LiteralExpr, Param, Pattern,
    PipelineOperator, SourceLocation, TypeDecl, UnaryOperator,
};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct TypeCheckResult {
    pub declaration_types: HashMap<String, InferenceType>,
    pub declaration_schemes: HashMap<String, TypeScheme>,
    pub typed_program: TypedProgram,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedProgram {
    pub declarations: Vec<TypedDeclaration>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedDeclaration {
    Function(TypedFunctionDecl),
    Type(TypedTypeDecl),
    Import(TypedImportDecl),
    Const(TypedConstDecl),
    Test(TypedTestDecl),
    Extern(TypedExternDecl),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedFunctionDecl {
    pub name: String,
    pub type_params: Vec<String>,
    pub params: Vec<Param>,
    pub return_type: InferenceType,
    pub effects: Option<EffectSet>,
    pub body: TypedExpr,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedTypeDecl {
    pub ast: TypeDecl,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedImportDecl {
    pub ast: ImportDecl,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedConstDecl {
    pub name: String,
    pub type_annotation: Option<sigil_ast::Type>,
    pub typ: InferenceType,
    pub value: TypedExpr,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedTestDecl {
    pub description: String,
    pub effects: Option<EffectSet>,
    pub body: TypedExpr,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedExternDecl {
    pub ast: ExternDecl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PurityClass {
    Pure,
    Effectful,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StrictnessClass {
    Strict,
    Deferred,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedExpr {
    pub kind: TypedExprKind,
    pub typ: InferenceType,
    pub effects: EffectSet,
    pub purity: PurityClass,
    pub strictness: StrictnessClass,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MethodSelector {
    Field(String),
    Index(Box<TypedExpr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedExprKind {
    Literal(LiteralExpr),
    Identifier(IdentifierExpr),
    NamespaceMember {
        namespace: Vec<String>,
        member: String,
    },
    Lambda(TypedLambdaExpr),
    Call(TypedCallExpr),
    ConstructorCall(TypedConstructorCallExpr),
    ExternCall(TypedExternCallExpr),
    MethodCall(TypedMethodCallExpr),
    Binary(TypedBinaryExpr),
    Unary(TypedUnaryExpr),
    Match(TypedMatchExpr),
    Let(TypedLetExpr),
    If(TypedIfExpr),
    List(TypedListExpr),
    Tuple(TypedTupleExpr),
    Record(TypedRecordExpr),
    MapLiteral(TypedMapLiteralExpr),
    FieldAccess(TypedFieldAccessExpr),
    Index(TypedIndexExpr),
    Map(TypedMapExpr),
    Filter(TypedFilterExpr),
    Fold(TypedFoldExpr),
    Concurrent(TypedConcurrentExpr),
    Pipeline(TypedPipelineExpr),
    WithMock(TypedWithMockExpr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedLambdaExpr {
    pub params: Vec<Param>,
    pub effects: Option<EffectSet>,
    pub return_type: InferenceType,
    pub body: Box<TypedExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedCallExpr {
    pub func: Box<TypedExpr>,
    pub args: Vec<TypedExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedConstructorCallExpr {
    pub module_path: Option<Vec<String>>,
    pub constructor: String,
    pub args: Vec<TypedExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedExternCallExpr {
    pub namespace: Vec<String>,
    pub member: String,
    pub mock_key: String,
    pub args: Vec<TypedExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedMethodCallExpr {
    pub receiver: Box<TypedExpr>,
    pub selector: MethodSelector,
    pub args: Vec<TypedExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedBinaryExpr {
    pub left: Box<TypedExpr>,
    pub operator: BinaryOperator,
    pub right: Box<TypedExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedUnaryExpr {
    pub operand: Box<TypedExpr>,
    pub operator: UnaryOperator,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedMatchArm {
    pub pattern: Pattern,
    pub guard: Option<Box<TypedExpr>>,
    pub body: Box<TypedExpr>,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedMatchExpr {
    pub scrutinee: Box<TypedExpr>,
    pub arms: Vec<TypedMatchArm>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedLetExpr {
    pub pattern: Pattern,
    pub value: Box<TypedExpr>,
    pub body: Box<TypedExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedIfExpr {
    pub condition: Box<TypedExpr>,
    pub then_branch: Box<TypedExpr>,
    pub else_branch: Option<Box<TypedExpr>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedListExpr {
    pub elements: Vec<TypedExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedTupleExpr {
    pub elements: Vec<TypedExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedRecordField {
    pub name: String,
    pub value: TypedExpr,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedRecordExpr {
    pub fields: Vec<TypedRecordField>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedMapEntryExpr {
    pub key: TypedExpr,
    pub value: TypedExpr,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedMapLiteralExpr {
    pub entries: Vec<TypedMapEntryExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedFieldAccessExpr {
    pub object: Box<TypedExpr>,
    pub field: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedIndexExpr {
    pub object: Box<TypedExpr>,
    pub index: Box<TypedExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedMapExpr {
    pub list: Box<TypedExpr>,
    pub func: Box<TypedExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedFilterExpr {
    pub list: Box<TypedExpr>,
    pub predicate: Box<TypedExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedFoldExpr {
    pub list: Box<TypedExpr>,
    pub func: Box<TypedExpr>,
    pub init: Box<TypedExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedConcurrentExpr {
    pub config: TypedConcurrentConfig,
    pub name: String,
    pub steps: Vec<TypedConcurrentStep>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedConcurrentConfig {
    pub concurrency: Box<TypedExpr>,
    pub jitter_ms: Box<TypedExpr>,
    pub stop_on: Box<TypedExpr>,
    pub window_ms: Box<TypedExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedConcurrentStep {
    Spawn(TypedSpawnStep),
    SpawnEach(TypedSpawnEachStep),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedSpawnStep {
    pub expr: Box<TypedExpr>,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedSpawnEachStep {
    pub func: Box<TypedExpr>,
    pub list: Box<TypedExpr>,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedPipelineExpr {
    pub left: Box<TypedExpr>,
    pub operator: PipelineOperator,
    pub right: Box<TypedExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WithMockTarget {
    LocalFunction(String),
    ExternMember {
        namespace: Vec<String>,
        member: String,
        mock_key: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedWithMockExpr {
    pub target: WithMockTarget,
    pub replacement: Box<TypedExpr>,
    pub body: Box<TypedExpr>,
}
