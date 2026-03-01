//! Type syntax AST nodes
use crate::SourceLocation;

/// Type expressions in Sigil
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type"))]
pub enum Type {
    #[cfg_attr(feature = "serde", serde(rename = "PrimitiveType"))]
    Primitive(PrimitiveType),
    #[cfg_attr(feature = "serde", serde(rename = "ListType"))]
    List(Box<ListType>),
    #[cfg_attr(feature = "serde", serde(rename = "MapType"))]
    Map(Box<MapType>),
    #[cfg_attr(feature = "serde", serde(rename = "FunctionType"))]
    Function(Box<FunctionType>),
    #[cfg_attr(feature = "serde", serde(rename = "TypeConstructor"))]
    Constructor(TypeConstructor),
    #[cfg_attr(feature = "serde", serde(rename = "TypeVariable"))]
    Variable(TypeVariable),
    #[cfg_attr(feature = "serde", serde(rename = "TupleType"))]
    Tuple(TupleType),
    #[cfg_attr(feature = "serde", serde(rename = "QualifiedType"))]
    Qualified(QualifiedType),
}

/// Primitive type: Int, Float, Bool, String, Char, Unit
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PrimitiveType {
    pub name: PrimitiveName,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub enum PrimitiveName {
    Int,
    Float,
    Bool,
    String,
    Char,
    Unit,
}

impl std::fmt::Display for PrimitiveName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrimitiveName::Int => write!(f, "Int"),
            PrimitiveName::Float => write!(f, "Float"),
            PrimitiveName::Bool => write!(f, "Bool"),
            PrimitiveName::String => write!(f, "String"),
            PrimitiveName::Char => write!(f, "Char"),
            PrimitiveName::Unit => write!(f, "Unit"),
        }
    }
}

/// List type: [T]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListType {
    #[cfg_attr(feature = "serde", serde(rename = "elementType"))]
    pub element_type: Type,
    pub location: SourceLocation,
}

/// Map type: Map[K, V]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MapType {
    #[cfg_attr(feature = "serde", serde(rename = "keyType"))]
    pub key_type: Type,
    #[cfg_attr(feature = "serde", serde(rename = "valueType"))]
    pub value_type: Type,
    pub location: SourceLocation,
}

/// Function type: (T1, T2) → R ! [Effect1, Effect2]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FunctionType {
    #[cfg_attr(feature = "serde", serde(rename = "paramTypes"))]
    pub param_types: Vec<Type>,
    pub effects: Vec<String>,  // Effect annotations: ['IO', 'Network', 'Async', 'Error', 'Mut']
    #[cfg_attr(feature = "serde", serde(rename = "returnType"))]
    pub return_type: Type,
    pub location: SourceLocation,
}

/// Type constructor: Result[T, E] or Option[T]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TypeConstructor {
    pub name: String,
    #[cfg_attr(feature = "serde", serde(rename = "typeArgs"))]
    pub type_args: Vec<Type>,
    pub location: SourceLocation,
}

/// Type variable: α, β, T, E
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TypeVariable {
    pub name: String,
    pub location: SourceLocation,
}

/// Tuple type: (T1, T2, T3)
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TupleType {
    pub types: Vec<Type>,
    pub location: SourceLocation,
}

/// Qualified type: src⋅types.ArticleMeta[T, E]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QualifiedType {
    #[cfg_attr(feature = "serde", serde(rename = "modulePath"))]
    pub module_path: Vec<String>,  // ['src', 'types'] from "src⋅types"
    #[cfg_attr(feature = "serde", serde(rename = "typeName"))]
    pub type_name: String,          // 'ArticleMeta' from "src⋅types.ArticleMeta"
    #[cfg_attr(feature = "serde", serde(rename = "typeArgs"))]
    pub type_args: Vec<Type>,       // [T, E] for generic types like "Result[T, E]"
    pub location: SourceLocation,
}
