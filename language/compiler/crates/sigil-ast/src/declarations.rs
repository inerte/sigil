//! Declaration AST nodes

use crate::{Expr, SourceLocation, Type};

/// Top-level declarations in a Sigil program
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type"))]
pub enum Declaration {
    #[cfg_attr(feature = "serde", serde(rename = "FunctionDecl"))]
    Function(FunctionDecl),
    #[cfg_attr(feature = "serde", serde(rename = "TransformDecl"))]
    Transform(TransformDecl),
    #[cfg_attr(feature = "serde", serde(rename = "TypeDecl"))]
    Type(TypeDecl),
    #[cfg_attr(feature = "serde", serde(rename = "ProtocolDecl"))]
    Protocol(ProtocolDecl),
    #[cfg_attr(feature = "serde", serde(rename = "LabelDecl"))]
    Label(LabelDecl),
    #[cfg_attr(feature = "serde", serde(rename = "RuleDecl"))]
    Rule(RuleDecl),
    #[cfg_attr(feature = "serde", serde(rename = "EffectDecl"))]
    Effect(EffectDecl),
    #[cfg_attr(feature = "serde", serde(rename = "FeatureFlagDecl"))]
    FeatureFlag(FeatureFlagDecl),
    #[cfg_attr(feature = "serde", serde(rename = "ConstDecl"))]
    Const(ConstDecl),
    #[cfg_attr(feature = "serde", serde(rename = "TestDecl"))]
    Test(TestDecl),
    #[cfg_attr(feature = "serde", serde(rename = "ExternDecl"))]
    Extern(ExternDecl),
}

/// Protocol declaration: compile-time state machine enforcement for a handle type
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProtocolDecl {
    pub name: String,
    pub transitions: Vec<ProtocolTransition>,
    pub initial: String,
    pub terminal: String,
    pub location: SourceLocation,
}

/// A single state transition in a protocol declaration
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProtocolTransition {
    pub from: String,
    pub to: String,
    pub via: Vec<String>,
    pub location: SourceLocation,
}

/// Function declaration
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FunctionDecl {
    pub name: String,
    #[cfg_attr(feature = "serde", serde(rename = "typeParams"))]
    pub type_params: Vec<String>,
    pub params: Vec<Param>,
    pub effects: Vec<String>, // Effect annotations: ['IO', 'Network', 'Error', 'Mut']
    #[cfg_attr(feature = "serde", serde(rename = "returnType"))]
    pub return_type: Option<Type>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub requires: Option<Expr>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub decreases: Option<Expr>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub ensures: Option<Expr>,
    pub body: Expr,
    pub location: SourceLocation,
}

/// Function or lambda parameter
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Param {
    pub name: String,
    #[cfg_attr(feature = "serde", serde(rename = "typeAnnotation"))]
    pub type_annotation: Option<Type>,
    #[cfg_attr(feature = "serde", serde(rename = "isMutable"))]
    pub is_mutable: bool, // tracks if parameter is mutable
    pub location: SourceLocation,
}

/// Type declaration
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TypeDecl {
    pub name: String,
    #[cfg_attr(feature = "serde", serde(rename = "typeParams"))]
    pub type_params: Vec<String>,
    pub definition: TypeDef,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub constraint: Option<Expr>,
    #[cfg_attr(feature = "serde", serde(default))]
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Vec::is_empty"))]
    pub labels: Vec<LabelRef>,
    pub location: SourceLocation,
}

/// Label declaration: label Pii combines [Sensitive,CustomerData]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelDecl {
    pub name: String,
    #[cfg_attr(feature = "serde", serde(default))]
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Vec::is_empty"))]
    pub combines: Vec<LabelRef>,
    pub location: SourceLocation,
}

/// Rule declaration: rule [•types.Pii] for •topology.auditLog=Allow()
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RuleDecl {
    pub labels: Vec<LabelRef>,
    pub boundary: MemberRef,
    pub action: RuleAction,
    pub location: SourceLocation,
}

/// Policy transform declaration: transform λredact(...)
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TransformDecl {
    pub function: FunctionDecl,
}

/// Boundary rule action.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type"))]
pub enum RuleAction {
    #[cfg_attr(feature = "serde", serde(rename = "AllowRuleAction"))]
    Allow { location: SourceLocation },
    #[cfg_attr(feature = "serde", serde(rename = "BlockRuleAction"))]
    Block { location: SourceLocation },
    #[cfg_attr(feature = "serde", serde(rename = "ThroughRuleAction"))]
    Through {
        transform: MemberRef,
        location: SourceLocation,
    },
}

/// Reference to a label declaration.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelRef {
    #[cfg_attr(feature = "serde", serde(default))]
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Vec::is_empty"))]
    pub module_path: Vec<String>,
    pub name: String,
    pub location: SourceLocation,
}

/// Reference to a module member such as •topology.auditLog or •policies.redactPii.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MemberRef {
    pub module_path: Vec<String>,
    pub member: String,
    pub location: SourceLocation,
}

/// Type definition (sum type, product type, or alias)
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type"))]
pub enum TypeDef {
    #[cfg_attr(feature = "serde", serde(rename = "SumType"))]
    Sum(SumType),
    #[cfg_attr(feature = "serde", serde(rename = "ProductType"))]
    Product(ProductType),
    #[cfg_attr(feature = "serde", serde(rename = "TypeAlias"))]
    Alias(TypeAlias),
}

/// Sum type (tagged union): Maybe[T] = Some T | None
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SumType {
    pub variants: Vec<Variant>,
    pub location: SourceLocation,
}

/// Variant in a sum type
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Variant {
    pub name: String,
    pub types: Vec<Type>,
    pub location: SourceLocation,
}

/// Product type (record): { field1: Type1, field2: Type2 }
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProductType {
    pub fields: Vec<Field>,
    pub location: SourceLocation,
}

/// Field in a product type
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Field {
    pub name: String,
    #[cfg_attr(feature = "serde", serde(rename = "fieldType"))]
    pub field_type: Type,
    pub location: SourceLocation,
}

/// Type alias: type MyInt = Int
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TypeAlias {
    #[cfg_attr(feature = "serde", serde(rename = "aliasedType"))]
    pub aliased_type: Type,
    pub location: SourceLocation,
}

/// Effect declaration: effect AppIo=!Fs!Log!Process
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EffectDecl {
    pub name: String,
    pub effects: Vec<String>,
    pub location: SourceLocation,
}

/// Feature flag declaration: featureFlag NewCheckout:Bool createdAt "..." default false
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FeatureFlagDecl {
    pub name: String,
    #[cfg_attr(feature = "serde", serde(rename = "flagType"))]
    pub flag_type: Type,
    #[cfg_attr(feature = "serde", serde(rename = "createdAt"))]
    pub created_at: String,
    #[cfg_attr(feature = "serde", serde(rename = "createdAtLocation"))]
    pub created_at_location: SourceLocation,
    pub default: Expr,
    pub location: SourceLocation,
}

/// Const declaration: c PI = 3.14159
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConstDecl {
    pub name: String,
    #[cfg_attr(feature = "serde", serde(rename = "typeAnnotation"))]
    pub type_annotation: Option<Type>,
    pub value: Expr,
    pub location: SourceLocation,
}

/// Test declaration: "should add numbers correctly" { ... }
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TestDecl {
    pub description: String,
    pub effects: Vec<String>,
    #[cfg_attr(feature = "serde", serde(rename = "worldBindings"))]
    pub world_bindings: Vec<ConstDecl>,
    pub body: Expr,
    pub location: SourceLocation,
}

/// External FFI declaration: e fs::promises { readFile: ... }
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExternDecl {
    #[cfg_attr(feature = "serde", serde(rename = "modulePath"))]
    pub module_path: Vec<String>, // ['fs', 'promises'] or ['axios'] (Sigil syntax: fs::promises)
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub members: Option<Vec<ExternMember>>, // Optional typed members for FFI type checking
    pub location: SourceLocation,
}

/// External member with type signature
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExternMember {
    pub name: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub kind: ExternMemberKind,
    #[cfg_attr(feature = "serde", serde(rename = "memberType"))]
    pub member_type: Type, // Function type or primitive type
    pub location: SourceLocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ExternMemberKind {
    #[default]
    Value,
    Subscription,
}
