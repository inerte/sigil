//! Canonical branching recursion validation
//!
//! Rejects the narrow non-canonical pattern where sibling self-calls reduce the
//! same parameter while keeping all other arguments identical.

use crate::error::ValidationError;
use sigil_ast::*;

#[derive(Debug, Clone, PartialEq)]
struct ReducedCall<'a> {
    reduced_param: String,
    reduction_amount: i64,
    non_reduced_args: Vec<&'a Expr>,
}

pub fn validate_branching_recursion(program: &Program) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    for declaration in &program.declarations {
        if let Declaration::Function(function) = declaration {
            if contains_nested_branching_shape(&function.body, &function.name)
                || has_direct_branching_shape(&function.body, &function.name)
            {
                errors.push(ValidationError::BranchingSelfRecursion {
                    function_name: function.name.clone(),
                    location: function.location,
                });
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn has_direct_branching_shape(expr: &Expr, function_name: &str) -> bool {
    let sibling_calls = collect_sibling_self_calls(expr, function_name);
    if contains_forbidden_branching_group(&sibling_calls) {
        return true;
    }

    match expr {
        Expr::Literal(_) | Expr::Identifier(_) | Expr::MemberAccess(_) => false,
        Expr::Lambda(lambda) => has_direct_branching_shape(&lambda.body, function_name),
        Expr::Application(application) => {
            has_direct_branching_shape(&application.func, function_name)
                || application
                    .args
                    .iter()
                    .any(|arg| has_direct_branching_shape(arg, function_name))
        }
        Expr::Binary(binary) => {
            has_direct_branching_shape(&binary.left, function_name)
                || has_direct_branching_shape(&binary.right, function_name)
        }
        Expr::Unary(unary) => has_direct_branching_shape(&unary.operand, function_name),
        Expr::Match(match_expr) => {
            has_direct_branching_shape(&match_expr.scrutinee, function_name)
                || match_expr.arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .map(|guard| has_direct_branching_shape(guard, function_name))
                        .unwrap_or(false)
                        || has_direct_branching_shape(&arm.body, function_name)
                })
        }
        Expr::Let(let_expr) => {
            has_direct_branching_shape(&let_expr.value, function_name)
                || has_direct_branching_shape(&let_expr.body, function_name)
        }
        Expr::Using(using_expr) => {
            has_direct_branching_shape(&using_expr.value, function_name)
                || has_direct_branching_shape(&using_expr.body, function_name)
        }
        Expr::If(if_expr) => {
            has_direct_branching_shape(&if_expr.condition, function_name)
                || has_direct_branching_shape(&if_expr.then_branch, function_name)
                || if_expr
                    .else_branch
                    .as_ref()
                    .map(|else_branch| has_direct_branching_shape(else_branch, function_name))
                    .unwrap_or(false)
        }
        Expr::List(list) => list
            .elements
            .iter()
            .any(|element| has_direct_branching_shape(element, function_name)),
        Expr::Record(record) => record
            .fields
            .iter()
            .any(|field| has_direct_branching_shape(&field.value, function_name)),
        Expr::MapLiteral(map) => map.entries.iter().any(|entry| {
            has_direct_branching_shape(&entry.key, function_name)
                || has_direct_branching_shape(&entry.value, function_name)
        }),
        Expr::Tuple(tuple) => tuple
            .elements
            .iter()
            .any(|element| has_direct_branching_shape(element, function_name)),
        Expr::FieldAccess(field_access) => {
            has_direct_branching_shape(&field_access.object, function_name)
        }
        Expr::Index(index) => {
            has_direct_branching_shape(&index.object, function_name)
                || has_direct_branching_shape(&index.index, function_name)
        }
        Expr::Pipeline(pipeline) => {
            has_direct_branching_shape(&pipeline.left, function_name)
                || has_direct_branching_shape(&pipeline.right, function_name)
        }
        Expr::Map(map) => {
            has_direct_branching_shape(&map.list, function_name)
                || has_direct_branching_shape(&map.func, function_name)
        }
        Expr::Filter(filter) => {
            has_direct_branching_shape(&filter.list, function_name)
                || has_direct_branching_shape(&filter.predicate, function_name)
        }
        Expr::Fold(fold) => {
            has_direct_branching_shape(&fold.list, function_name)
                || has_direct_branching_shape(&fold.init, function_name)
                || has_direct_branching_shape(&fold.func, function_name)
        }
        Expr::Concurrent(concurrent) => concurrent.steps.iter().any(|step| match step {
            ConcurrentStep::Spawn(spawn) => has_direct_branching_shape(&spawn.expr, function_name),
            ConcurrentStep::SpawnEach(spawn_each) => {
                has_direct_branching_shape(&spawn_each.list, function_name)
                    || has_direct_branching_shape(&spawn_each.func, function_name)
            }
        }),
        Expr::TypeAscription(type_ascription) => {
            has_direct_branching_shape(&type_ascription.expr, function_name)
        }
    }
}

fn contains_nested_branching_shape(expr: &Expr, function_name: &str) -> bool {
    match expr {
        Expr::Application(application) => {
            if is_self_call(&application.func, function_name)
                && application
                    .args
                    .iter()
                    .any(|arg| has_direct_branching_shape(arg, function_name))
            {
                return true;
            }

            contains_nested_branching_shape(&application.func, function_name)
                || application
                    .args
                    .iter()
                    .any(|arg| contains_nested_branching_shape(arg, function_name))
        }
        Expr::Binary(binary) => {
            contains_nested_branching_shape(&binary.left, function_name)
                || contains_nested_branching_shape(&binary.right, function_name)
        }
        Expr::Unary(unary) => contains_nested_branching_shape(&unary.operand, function_name),
        Expr::Match(match_expr) => {
            contains_nested_branching_shape(&match_expr.scrutinee, function_name)
                || match_expr.arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .map(|guard| contains_nested_branching_shape(guard, function_name))
                        .unwrap_or(false)
                        || contains_nested_branching_shape(&arm.body, function_name)
                })
        }
        Expr::Let(let_expr) => {
            contains_nested_branching_shape(&let_expr.value, function_name)
                || contains_nested_branching_shape(&let_expr.body, function_name)
        }
        Expr::Using(using_expr) => {
            contains_nested_branching_shape(&using_expr.value, function_name)
                || contains_nested_branching_shape(&using_expr.body, function_name)
        }
        Expr::If(if_expr) => {
            contains_nested_branching_shape(&if_expr.condition, function_name)
                || contains_nested_branching_shape(&if_expr.then_branch, function_name)
                || if_expr
                    .else_branch
                    .as_ref()
                    .map(|else_branch| contains_nested_branching_shape(else_branch, function_name))
                    .unwrap_or(false)
        }
        Expr::List(list) => list
            .elements
            .iter()
            .any(|element| contains_nested_branching_shape(element, function_name)),
        Expr::Record(record) => record
            .fields
            .iter()
            .any(|field| contains_nested_branching_shape(&field.value, function_name)),
        Expr::MapLiteral(map) => map.entries.iter().any(|entry| {
            contains_nested_branching_shape(&entry.key, function_name)
                || contains_nested_branching_shape(&entry.value, function_name)
        }),
        Expr::Tuple(tuple) => tuple
            .elements
            .iter()
            .any(|element| contains_nested_branching_shape(element, function_name)),
        Expr::FieldAccess(field_access) => {
            contains_nested_branching_shape(&field_access.object, function_name)
        }
        Expr::Index(index) => {
            contains_nested_branching_shape(&index.object, function_name)
                || contains_nested_branching_shape(&index.index, function_name)
        }
        Expr::Pipeline(pipeline) => {
            contains_nested_branching_shape(&pipeline.left, function_name)
                || contains_nested_branching_shape(&pipeline.right, function_name)
        }
        Expr::Map(map) => {
            contains_nested_branching_shape(&map.list, function_name)
                || contains_nested_branching_shape(&map.func, function_name)
        }
        Expr::Filter(filter) => {
            contains_nested_branching_shape(&filter.list, function_name)
                || contains_nested_branching_shape(&filter.predicate, function_name)
        }
        Expr::Fold(fold) => {
            contains_nested_branching_shape(&fold.list, function_name)
                || contains_nested_branching_shape(&fold.init, function_name)
                || contains_nested_branching_shape(&fold.func, function_name)
        }
        Expr::Concurrent(concurrent) => concurrent.steps.iter().any(|step| match step {
            ConcurrentStep::Spawn(spawn) => {
                contains_nested_branching_shape(&spawn.expr, function_name)
            }
            ConcurrentStep::SpawnEach(spawn_each) => {
                contains_nested_branching_shape(&spawn_each.list, function_name)
                    || contains_nested_branching_shape(&spawn_each.func, function_name)
            }
        }),
        Expr::Lambda(lambda) => contains_nested_branching_shape(&lambda.body, function_name),
        Expr::TypeAscription(type_ascription) => {
            contains_nested_branching_shape(&type_ascription.expr, function_name)
        }
        Expr::Literal(_) | Expr::Identifier(_) | Expr::MemberAccess(_) => false,
    }
}

fn collect_sibling_self_calls<'a>(expr: &'a Expr, function_name: &str) -> Vec<ReducedCall<'a>> {
    match expr {
        Expr::Application(application) => match extract_reduced_call(application, function_name) {
            Some(call) => vec![call],
            None => Vec::new(),
        },
        Expr::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Add
                    | BinaryOperator::Multiply
                    | BinaryOperator::And
                    | BinaryOperator::Or
                    | BinaryOperator::Append
                    | BinaryOperator::ListAppend
            ) =>
        {
            let mut calls = collect_sibling_self_calls(&binary.left, function_name);
            calls.extend(collect_sibling_self_calls(&binary.right, function_name));
            calls
        }
        _ => Vec::new(),
    }
}

fn extract_reduced_call<'a>(
    application: &'a ApplicationExpr,
    function_name: &str,
) -> Option<ReducedCall<'a>> {
    if !is_self_call(&application.func, function_name) {
        return None;
    }

    let mut reduced_param = None;
    let mut reduction_amount = None;
    let mut non_reduced_args = Vec::with_capacity(application.args.len());

    for argument in &application.args {
        if let Some((param_name, amount)) = direct_positive_reduction(argument) {
            match (&reduced_param, &reduction_amount) {
                (None, None) => {
                    reduced_param = Some(param_name);
                    reduction_amount = Some(amount);
                }
                (Some(existing_param), Some(existing_amount))
                    if existing_param == &param_name && *existing_amount == amount =>
                {
                    non_reduced_args.push(argument);
                }
                _ => return None,
            }
        } else {
            non_reduced_args.push(argument);
        }
    }

    Some(ReducedCall {
        reduced_param: reduced_param?,
        reduction_amount: reduction_amount?,
        non_reduced_args,
    })
}

fn direct_positive_reduction(expr: &Expr) -> Option<(String, i64)> {
    let Expr::Binary(binary) = expr else {
        return None;
    };
    if binary.operator != BinaryOperator::Subtract {
        return None;
    }
    let Expr::Identifier(identifier) = &binary.left else {
        return None;
    };
    let Expr::Literal(literal) = &binary.right else {
        return None;
    };
    let LiteralValue::Int(amount) = literal.value else {
        return None;
    };
    if amount <= 0 {
        return None;
    }
    Some((identifier.name.clone(), amount))
}

fn contains_forbidden_branching_group(calls: &[ReducedCall<'_>]) -> bool {
    if calls.len() < 2 {
        return false;
    }

    let first = &calls[0];
    calls.iter().skip(1).all(|call| {
        call.reduced_param == first.reduced_param && call.non_reduced_args == first.non_reduced_args
    })
}

fn is_self_call(func: &Expr, function_name: &str) -> bool {
    matches!(func, Expr::Identifier(identifier) if identifier.name == function_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_branching_recursion;
    use sigil_lexer::tokenize;
    use sigil_parser::parse;

    fn validate(source: &str) -> Result<(), Vec<ValidationError>> {
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        validate_branching_recursion(&program)
    }

    #[test]
    fn test_direct_branching_self_recursion_rejected() {
        let source = r#"λfib(n:Int)=>Int match n{
  0=>0|
  1=>1|
  value=>fib(value-1)+fib(value-2)
}"#;
        let result = validate(source);
        assert!(matches!(
            result,
            Err(errors) if errors.iter().any(|error| matches!(error, ValidationError::BranchingSelfRecursion { .. }))
        ));
    }

    #[test]
    fn test_tribonacci_branching_self_recursion_rejected() {
        let source = r#"λtrib(n:Int)=>Int match n{
  0=>0|
  1=>1|
  2=>1|
  value=>trib(value-1)+trib(value-2)+trib(value-3)
}"#;
        let result = validate(source);
        assert!(matches!(
            result,
            Err(errors) if errors.iter().any(|error| matches!(error, ValidationError::BranchingSelfRecursion { .. }))
        ));
    }

    #[test]
    fn test_duplicate_same_reduction_rejected() {
        let source = r#"λbad(n:Int)=>Int match n{
  0=>0|
  value=>bad(value-1)+bad(value-1)
}"#;
        let result = validate(source);
        assert!(matches!(
            result,
            Err(errors) if errors.iter().any(|error| matches!(error, ValidationError::BranchingSelfRecursion { .. }))
        ));
    }

    #[test]
    fn test_nested_branching_self_recursion_rejected() {
        let source = r#"λbad(n:Int)=>Int match n{
  0=>0|
  1=>1|
  value=>bad(bad(value-1)+bad(value-2))
}"#;
        let result = validate(source);
        assert!(matches!(
            result,
            Err(errors) if errors.iter().any(|error| matches!(error, ValidationError::BranchingSelfRecursion { .. }))
        ));
    }

    #[test]
    fn test_single_recursive_call_allowed() {
        let source = r#"λcountdown(n:Int)=>Int match n{
  0=>0|
  value=>countdown(value-1)
}"#;
        assert!(validate(source).is_ok());
    }

    #[test]
    fn test_wrapper_plus_helper_allowed() {
        let source = r#"λfib(n:Int)=>Int=fibHelper(0,1,n)

λfibHelper(a:Int,b:Int,n:Int)=>Int match n{
  0=>a|
  count=>fibHelper(b,a+b,count-1)
}"#;
        assert!(validate(source).is_ok());
    }

    #[test]
    fn test_hanoi_style_partitioned_recursion_allowed() {
        let source = r#"λhanoi(from:String,aux:String,count:Int,to:String)=>String match count{
  1=>"Move from "++from++" to "++to|
  n=>hanoi(from,to,n-1,aux)++"..."++hanoi(aux,from,n-1,to)
}"#;
        assert!(validate(source).is_ok());
    }

    #[test]
    fn test_different_non_reduced_arguments_allowed() {
        let source = r#"λwalk(left:Int,n:Int,right:Int)=>Int match n{
  0=>left+right|
  value=>walk(left+1,value-1,right)+walk(left,value-2,right+1)
}"#;
        assert!(validate(source).is_ok());
    }

    #[test]
    fn test_tree_recursion_allowed() {
        let source = r#"t Tree=Leaf(Int)|Node(Tree,Tree)

λheight(tree:Tree)=>Int match tree{
  Leaf(value)=>1|
  Node(left,right)=>1+height(left)+height(right)
}"#;
        assert!(validate(source).is_ok());
    }
}
