//! Bidirectional Type Checking for Sigil
//!
//! Uses two complementary modes:
//! - Synthesis (⇒): Infer type from expression structure (bottom-up)
//! - Checking (⇐): Verify expression matches expected type (top-down)
//!
//! This is simpler than Hindley-Milner because Sigil requires mandatory
//! type annotations everywhere, making the inference burden much lighter.

use crate::environment::{BindingMeta, TypeEnvironment, TypeInfo};
use crate::errors::{format_type, TypeError};
use crate::types::{ast_type_to_inference_type, types_equal, InferenceType, TFunction, TPrimitive};
use crate::TypeCheckOptions;
use sigil_ast::{
    BinaryOperator, Declaration, Expr, FunctionDecl, LiteralType, LiteralValue, PrimitiveName,
    Program,
};
use std::collections::HashMap;

/// Type check a Sigil program
///
/// Returns a map of function names to their inferred types
pub fn type_check(
    program: &Program,
    _source_code: &str,
    options: TypeCheckOptions,
) -> Result<HashMap<String, InferenceType>, TypeError> {
    let mut env = TypeEnvironment::create_initial();
    let mut types = HashMap::new();

    // Register imported type registries
    if let Some(imported_type_registries) = options.imported_type_registries {
        for (module_id, type_registry) in imported_type_registries {
            env.register_imported_types(module_id, type_registry);
        }
    }

    // First pass: Register all function signatures and types
    for decl in &program.declarations {
        match decl {
            Declaration::Type(type_decl) => {
                // Register the type in the type registry
                env.register_type(
                    type_decl.name.clone(),
                    TypeInfo {
                        type_params: type_decl.type_params.clone(),
                        definition: type_decl.definition.clone(),
                    },
                );

                // TODO: Register constructor functions for sum types
            }

            Declaration::Function(func_decl) => {
                // Extract function type from signature
                let params: Vec<InferenceType> = func_decl
                    .params
                    .iter()
                    .map(|p| {
                        p.type_annotation
                            .as_ref()
                            .map(ast_type_to_inference_type)
                            .unwrap_or(InferenceType::Any)
                    })
                    .collect();

                let return_type = func_decl
                    .return_type
                    .as_ref()
                    .map(ast_type_to_inference_type)
                    .unwrap_or(InferenceType::Any);

                let effects = if func_decl.effects.is_empty() {
                    None
                } else {
                    Some(func_decl.effects.iter().cloned().collect())
                };

                let func_type = InferenceType::Function(Box::new(TFunction {
                    params,
                    return_type,
                    effects,
                }));

                if func_decl.is_mockable {
                    env.bind_with_meta(
                        func_decl.name.clone(),
                        func_type.clone(),
                        BindingMeta {
                            is_mockable_function: true,
                            is_extern_namespace: false,
                        },
                    );
                } else {
                    env.bind(func_decl.name.clone(), func_type.clone());
                }

                types.insert(func_decl.name.clone(), func_type);
            }

            Declaration::Const(const_decl) => {
                // Register constant type
                let const_type = const_decl
                    .type_annotation
                    .as_ref()
                    .map(ast_type_to_inference_type)
                    .unwrap_or(InferenceType::Any);

                env.bind(const_decl.name.clone(), const_type.clone());
                types.insert(const_decl.name.clone(), const_type);
            }

            Declaration::Extern(extern_decl) => {
                let namespace_name = extern_decl.module_path.join("⋅");

                if let Some(_members) = &extern_decl.members {
                    // TODO: Create record type with typed members
                    env.bind_with_meta(
                        namespace_name,
                        InferenceType::Any,
                        BindingMeta {
                            is_mockable_function: false,
                            is_extern_namespace: true,
                        },
                    );
                } else {
                    // Untyped extern: trust mode
                    env.bind_with_meta(
                        namespace_name,
                        InferenceType::Any,
                        BindingMeta {
                            is_mockable_function: false,
                            is_extern_namespace: true,
                        },
                    );
                }
            }

            Declaration::Import(import_decl) => {
                let namespace_name = import_decl.module_path.join("⋅");
                let imported_type = options
                    .imported_namespaces
                    .as_ref()
                    .and_then(|ns| ns.get(&namespace_name))
                    .cloned()
                    .unwrap_or(InferenceType::Any);

                env.bind(namespace_name, imported_type);
            }

            Declaration::Test(_) => {
                // TODO: Check test declarations
            }
        }
    }

    // Second pass: Type check function bodies
    for decl in &program.declarations {
        if let Declaration::Function(func_decl) = decl {
            check_function_decl(&env, func_decl)?;
        } else if let Declaration::Const(const_decl) = decl {
            // Type check constant value
            let value_type = synthesize(&env, &const_decl.value)?;
            if let Some(ref annotation) = const_decl.type_annotation {
                let expected_type = ast_type_to_inference_type(annotation);
                if !types_equal(&value_type, &expected_type) {
                    return Err(TypeError::mismatch(
                        format!(
                            "Constant '{}' type mismatch",
                            const_decl.name
                        ),
                        Some(const_decl.location),
                        expected_type,
                        value_type,
                    ));
                }
            }
        }
    }

    Ok(types)
}

/// Type check a function declaration
fn check_function_decl(env: &TypeEnvironment, func_decl: &FunctionDecl) -> Result<(), TypeError> {
    // Create environment with parameter bindings
    let mut func_env = env.extend(None);

    for param in &func_decl.params {
        let param_type = param
            .type_annotation
            .as_ref()
            .map(ast_type_to_inference_type)
            .unwrap_or(InferenceType::Any);
        func_env.bind(param.name.clone(), param_type);
    }

    // Get expected return type
    let expected_return_type = func_decl
        .return_type
        .as_ref()
        .map(ast_type_to_inference_type)
        .unwrap_or(InferenceType::Any);

    // Type check body
    check(&func_env, &func_decl.body, &expected_return_type)?;

    Ok(())
}

// ============================================================================
// SYNTHESIS (⇒) - Infer type from expression
// ============================================================================

/// Synthesize (infer) type from expression
/// Returns the inferred type
fn synthesize(env: &TypeEnvironment, expr: &Expr) -> Result<InferenceType, TypeError> {
    match expr {
        Expr::Literal(lit) => Ok(synthesize_literal(lit)),

        Expr::Identifier(id) => {
            env.lookup(&id.name).ok_or_else(|| {
                TypeError::new(
                    format!("Unbound variable: {}", id.name),
                    Some(id.location),
                )
            })
        }

        Expr::Binary(bin) => synthesize_binary(env, bin),

        Expr::Unary(un) => synthesize_unary(env, un),

        Expr::Application(app) => synthesize_application(env, app),

        Expr::List(list) => synthesize_list(env, list),

        Expr::If(if_expr) => synthesize_if(env, if_expr),

        Expr::Let(let_expr) => synthesize_let(env, let_expr),

        _ => Err(TypeError::new(
            format!("Synthesis not yet implemented for expression type"),
            None, // TODO: extract location from specific expression variant
        )),
    }
}

fn synthesize_literal(lit: &sigil_ast::LiteralExpr) -> InferenceType {
    let prim_name = match lit.literal_type {
        LiteralType::Int => PrimitiveName::Int,
        LiteralType::Float => PrimitiveName::Float,
        LiteralType::String => PrimitiveName::String,
        LiteralType::Char => PrimitiveName::Char,
        LiteralType::Unit => PrimitiveName::Unit,
        LiteralType::Bool => PrimitiveName::Bool,
    };

    InferenceType::Primitive(TPrimitive { name: prim_name })
}

fn synthesize_binary(
    env: &TypeEnvironment,
    bin: &sigil_ast::BinaryExpr,
) -> Result<InferenceType, TypeError> {
    let left_type = synthesize(env, &bin.left)?;
    let right_type = synthesize(env, &bin.right)?;

    let int_type = InferenceType::Primitive(TPrimitive {
        name: PrimitiveName::Int,
    });
    let bool_type = InferenceType::Primitive(TPrimitive {
        name: PrimitiveName::Bool,
    });
    let string_type = InferenceType::Primitive(TPrimitive {
        name: PrimitiveName::String,
    });

    match bin.operator {
        // Arithmetic operators: ℤ → ℤ → ℤ
        BinaryOperator::Add
        | BinaryOperator::Subtract
        | BinaryOperator::Multiply
        | BinaryOperator::Divide
        | BinaryOperator::Modulo => {
            // Special case: + with string operands does concatenation
            if bin.operator == BinaryOperator::Add
                && (matches!(left_type, InferenceType::Primitive(ref p) if p.name == PrimitiveName::String)
                    || matches!(right_type, InferenceType::Primitive(ref p) if p.name == PrimitiveName::String))
            {
                return Ok(string_type);
            }

            // Otherwise require both operands to be integers
            check(env, &bin.left, &int_type)?;
            check(env, &bin.right, &int_type)?;
            Ok(int_type)
        }

        // Comparison operators: ℤ → ℤ → 𝔹
        BinaryOperator::Less
        | BinaryOperator::Greater
        | BinaryOperator::LessEq
        | BinaryOperator::GreaterEq => {
            check(env, &bin.left, &int_type)?;
            check(env, &bin.right, &int_type)?;
            Ok(bool_type)
        }

        // Equality operators: T → T → 𝔹 (polymorphic)
        BinaryOperator::Equal | BinaryOperator::NotEqual => {
            if !types_equal(&left_type, &right_type) {
                return Err(TypeError::new(
                    format!(
                        "Cannot compare {} with {}",
                        format_type(&left_type),
                        format_type(&right_type)
                    ),
                    Some(bin.location),
                ));
            }
            Ok(bool_type)
        }

        // Logical operators: 𝔹 → 𝔹 → 𝔹
        BinaryOperator::And | BinaryOperator::Or => {
            check(env, &bin.left, &bool_type)?;
            check(env, &bin.right, &bool_type)?;
            Ok(bool_type)
        }

        // String concatenation: 𝕊 → 𝕊 → 𝕊
        BinaryOperator::Append => {
            check(env, &bin.left, &string_type)?;
            check(env, &bin.right, &string_type)?;
            Ok(string_type)
        }

        // List append: [T] → [T] → [T]
        BinaryOperator::ListAppend => {
            if !matches!(left_type, InferenceType::List(_)) {
                return Err(TypeError::new(
                    format!("List append requires list operands, got {}", format_type(&left_type)),
                    Some(bin.location),
                ));
            }
            if !types_equal(&left_type, &right_type) {
                return Err(TypeError::new(
                    format!(
                        "Cannot concatenate lists of different types: {} and {}",
                        format_type(&left_type),
                        format_type(&right_type)
                    ),
                    Some(bin.location),
                ));
            }
            Ok(left_type)
        }

        _ => Err(TypeError::new(
            format!("Operator {:?} not yet implemented", bin.operator),
            Some(bin.location),
        )),
    }
}

fn synthesize_unary(
    env: &TypeEnvironment,
    un: &sigil_ast::UnaryExpr,
) -> Result<InferenceType, TypeError> {
    let int_type = InferenceType::Primitive(TPrimitive {
        name: PrimitiveName::Int,
    });
    let bool_type = InferenceType::Primitive(TPrimitive {
        name: PrimitiveName::Bool,
    });

    match un.operator {
        sigil_ast::UnaryOperator::Negate => {
            check(env, &un.operand, &int_type)?;
            Ok(int_type)
        }
        sigil_ast::UnaryOperator::Not => {
            check(env, &un.operand, &bool_type)?;
            Ok(bool_type)
        }
        sigil_ast::UnaryOperator::Length => {
            // Length operator # - works on strings and lists
            let operand_type = synthesize(env, &un.operand)?;
            match operand_type {
                InferenceType::Primitive(ref p) if p.name == PrimitiveName::String => Ok(int_type),
                InferenceType::List(_) => Ok(int_type),
                _ => Err(TypeError::new(
                    format!(
                        "Length operator # requires string or list, got {}",
                        format_type(&operand_type)
                    ),
                    Some(un.location),
                )),
            }
        }
    }
}

fn synthesize_application(
    env: &TypeEnvironment,
    app: &sigil_ast::ApplicationExpr,
) -> Result<InferenceType, TypeError> {
    let fn_type = synthesize(env, &app.func)?;

    // Special case: applying 'any' type (FFI function call)
    if matches!(fn_type, InferenceType::Any) {
        return Ok(InferenceType::Any);
    }

    match fn_type {
        InferenceType::Function(ref tfunc) => {
            // Check argument count
            if app.args.len() != tfunc.params.len() {
                return Err(TypeError::new(
                    format!(
                        "Function expects {} arguments, got {}",
                        tfunc.params.len(),
                        app.args.len()
                    ),
                    Some(app.location),
                ));
            }

            // Check each argument against parameter type
            for (arg, param_type) in app.args.iter().zip(&tfunc.params) {
                check(env, arg, param_type)?;
            }

            Ok(tfunc.return_type.clone())
        }
        _ => Err(TypeError::new(
            format!("Expected function type, got {}", format_type(&fn_type)),
            Some(app.location),
        )),
    }
}

fn synthesize_list(
    env: &TypeEnvironment,
    list: &sigil_ast::ListExpr,
) -> Result<InferenceType, TypeError> {
    if list.elements.is_empty() {
        // Empty list - cannot infer element type
        // In checked position this would be ok, but in synthesis we need a type
        // Return a list of Any for now
        return Ok(InferenceType::List(Box::new(crate::types::TList {
            element_type: InferenceType::Any,
        })));
    }

    // Infer type from first element
    let first_type = synthesize(env, &list.elements[0])?;

    // Check remaining elements match
    for elem in &list.elements[1..] {
        check(env, elem, &first_type)?;
    }

    Ok(InferenceType::List(Box::new(crate::types::TList {
        element_type: first_type,
    })))
}

fn synthesize_if(
    env: &TypeEnvironment,
    if_expr: &sigil_ast::IfExpr,
) -> Result<InferenceType, TypeError> {
    // Check condition is boolean
    let bool_type = InferenceType::Primitive(TPrimitive {
        name: PrimitiveName::Bool,
    });
    check(env, &if_expr.condition, &bool_type)?;

    // Synthesize then branch
    let then_type = synthesize(env, &if_expr.then_branch)?;

    // If no else branch, then branch must be Unit
    if if_expr.else_branch.is_none() {
        let unit_type = InferenceType::Primitive(TPrimitive {
            name: PrimitiveName::Unit,
        });
        if !types_equal(&then_type, &unit_type) {
            return Err(TypeError::new(
                format!(
                    "If expression without else must have Unit type, got {}",
                    format_type(&then_type)
                ),
                Some(if_expr.location),
            ));
        }
        return Ok(then_type);
    }

    // Synthesize else branch
    let else_type = synthesize(env, if_expr.else_branch.as_ref().unwrap())?;

    // Both branches must have same type
    if !types_equal(&then_type, &else_type) {
        return Err(TypeError::new(
            format!(
                "If branches have different types: then is {}, else is {}",
                format_type(&then_type),
                format_type(&else_type)
            ),
            Some(if_expr.location),
        ));
    }

    Ok(then_type)
}

fn synthesize_let(
    env: &TypeEnvironment,
    let_expr: &sigil_ast::LetExpr,
) -> Result<InferenceType, TypeError> {
    use sigil_ast::Pattern;

    // Synthesize binding value type
    let value_type = synthesize(env, &let_expr.value)?;

    // Check pattern and get bindings
    // For now, only support simple identifier patterns
    // TODO: Full pattern matching support
    let mut bindings = HashMap::new();
    match &let_expr.pattern {
        Pattern::Identifier(id_pattern) => {
            bindings.insert(id_pattern.name.clone(), value_type);
        }
        _ => {
            return Err(TypeError::new(
                "Let expression pattern matching not yet fully implemented".to_string(),
                Some(let_expr.location),
            ));
        }
    }

    // Extend environment and synthesize body
    let body_env = env.extend(Some(bindings));
    synthesize(&body_env, &let_expr.body)
}

// ============================================================================
// CHECKING (⇐) - Verify expression matches expected type
// ============================================================================

/// Check expression against expected type
/// Returns error if expression doesn't match
fn check(
    env: &TypeEnvironment,
    expr: &Expr,
    expected_type: &InferenceType,
) -> Result<(), TypeError> {
    // Special case: checking against 'any' type always succeeds (FFI trust mode)
    if matches!(expected_type, InferenceType::Any) {
        return Ok(());
    }

    // For most expressions: synthesize then verify equality
    let actual_type = synthesize(env, expr)?;

    // Special case: 'any' type matches anything (FFI trust mode)
    if matches!(actual_type, InferenceType::Any) {
        return Ok(());
    }

    if !types_equal(&actual_type, expected_type) {
        return Err(TypeError::mismatch(
            format!(
                "Type mismatch: expected {}, got {}",
                format_type(expected_type),
                format_type(&actual_type)
            ),
            None, // TODO: extract location from expression variant
            expected_type.clone(),
            actual_type,
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sigil_lexer::tokenize;
    use sigil_parser::parse;

    #[test]
    fn test_simple_integer_function() {
        let source = "λadd(x:ℤ,y:ℤ)→ℤ=x+y";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());

        let types = result.unwrap();
        assert_eq!(types.len(), 1);
        assert!(types.contains_key("add"));
    }

    #[test]
    fn test_type_mismatch() {
        let source = "λbad(x:ℤ)→𝕊=x";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_literal_types() {
        let source = "λf()→ℤ=42";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_function_application() {
        let source = "λadd(x:ℤ,y:ℤ)→ℤ=x+y\nλmain()→ℤ=add(1,2)";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();

        let result = type_check(&program, source, TypeCheckOptions::default());
        assert!(result.is_ok());
    }

    // TODO: Add If/Let expression tests when full syntax support is confirmed
    // The type checking logic is implemented, but needs matching lexer/parser support
}
