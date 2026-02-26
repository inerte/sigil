//! Type syntax AST nodes
use crate::SourceLocation;



/// Type expressions in Sigil
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type"))]
pub enum Type {
    Primitive(PrimitiveType),
    List(Box<ListType>),
    Map(Box<MapType>),
    Function(Box<FunctionType>),
    Constructor(TypeConstructor),
    Variable(TypeVariable),
    Tuple(TupleType),
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
    pub element_type: Type,
    pub location: SourceLocation,
}

/// Map type: Map[K, V]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MapType {
    pub key_type: Type,
    pub value_type: Type,
    pub location: SourceLocation,
}

/// Function type: (T1, T2) → R ! [Effect1, Effect2]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FunctionType {
    pub param_types: Vec<Type>,
    pub effects: Vec<String>,  // Effect annotations: ['IO', 'Network', 'Async', 'Error', 'Mut']
    pub return_type: Type,
    pub location: SourceLocation,
}

/// Type constructor: Result[T, E] or Option[T]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TypeConstructor {
    pub name: String,
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
    pub module_path: Vec<String>,  // ['src', 'types'] from "src⋅types"
    pub type_name: String,          // 'ArticleMeta' from "src⋅types.ArticleMeta"
    pub type_args: Vec<Type>,       // [T, E] for generic types like "Result[T, E]"
    pub location: SourceLocation,
}
