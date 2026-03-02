//! Declaration AST nodes

use crate::{Expr, SourceLocation, Type};

/// Top-level declarations in a Sigil program
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type"))]
pub enum Declaration {
    #[cfg_attr(feature = "serde", serde(rename = "FunctionDecl"))]
    Function(FunctionDecl),
    #[cfg_attr(feature = "serde", serde(rename = "TypeDecl"))]
    Type(TypeDecl),
    #[cfg_attr(feature = "serde", serde(rename = "ImportDecl"))]
    Import(ImportDecl),
    #[cfg_attr(feature = "serde", serde(rename = "ConstDecl"))]
    Const(ConstDecl),
    #[cfg_attr(feature = "serde", serde(rename = "TestDecl"))]
    Test(TestDecl),
    #[cfg_attr(feature = "serde", serde(rename = "ExternDecl"))]
    Extern(ExternDecl),
}

/// Function declaration
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FunctionDecl {
    pub name: String,
    #[cfg_attr(feature = "serde", serde(rename = "isMockable"))]
    pub is_mockable: bool,
    pub params: Vec<Param>,
    pub effects: Vec<String>,     // Effect annotations: ['IO', 'Network', 'Async', 'Error', 'Mut']
    #[cfg_attr(feature = "serde", serde(rename = "returnType"))]
    pub return_type: Option<Type>,
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
    pub is_mutable: bool,          // tracks if parameter is mutable
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

/// Import declaration: i stdlib⋅list
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImportDecl {
    #[cfg_attr(feature = "serde", serde(rename = "modulePath"))]
    pub module_path: Vec<String>,  // No selective imports - works like FFI (use as namespace.member)
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
    pub body: Expr,
    pub location: SourceLocation,
}

/// External FFI declaration: e fs⋅promises { readFile: ... }
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExternDecl {
    #[cfg_attr(feature = "serde", serde(rename = "modulePath"))]
    pub module_path: Vec<String>,     // ['fs', 'promises'] or ['axios'] (Sigil syntax: fs⋅promises)
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub members: Option<Vec<ExternMember>>, // Optional typed members for FFI type checking
    pub location: SourceLocation,
}

/// External member with type signature
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExternMember {
    pub name: String,
    #[cfg_attr(feature = "serde", serde(rename = "memberType"))]
    pub member_type: Type,             // Function type or primitive type
    pub location: SourceLocation,
}
