use sigil_ast::SourceLocation;
use sigil_typechecker::typed_ir::{
    MethodSelector, TypedConcurrentStep, TypedDeclaration, TypedExpr, TypedExprKind, TypedProgram,
};

pub const SPAN_MAP_FORMAT_VERSION: usize = 1;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugSourcePoint {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugSourceSpan {
    pub file: String,
    pub start: DebugSourcePoint,
    pub end: DebugSourcePoint,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugSpanKind {
    FunctionDecl,
    ConstDecl,
    JsonCodecDecl,
    TestDecl,
    TestWorldBinding,
    MatchArm,
    ExprLiteral,
    ExprIdentifier,
    ExprNamespaceMember,
    ExprLambda,
    ExprCall,
    ExprConstructorCall,
    ExprExternCall,
    ExprMethodCall,
    ExprBinary,
    ExprUnary,
    ExprMatch,
    ExprLet,
    ExprIf,
    ExprList,
    ExprTuple,
    ExprRecord,
    ExprMapLiteral,
    ExprFieldAccess,
    ExprIndex,
    ExprMap,
    ExprFilter,
    ExprFold,
    ExprConcurrent,
    ExprPipeline,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedLineRange {
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugSpanRecord {
    pub span_id: String,
    pub module_id: String,
    pub source_file: String,
    pub kind: DebugSpanKind,
    pub location: DebugSourceSpan,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_range: Option<GeneratedLineRange>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleSpanMap {
    pub format_version: usize,
    pub module_id: String,
    pub source_file: String,
    pub output_file: String,
    pub spans: Vec<DebugSpanRecord>,
}

impl ModuleSpanMap {
    pub fn annotate_generated_range(&mut self, span_id: &str, start_line: usize, end_line: usize) {
        if let Some(span) = self.spans.iter_mut().find(|span| span.span_id == span_id) {
            span.generated_range = Some(GeneratedLineRange {
                start_line,
                end_line,
            });
        }
    }
}

#[derive(Debug, Clone)]
pub struct CollectedModuleSpanMap {
    pub span_map: ModuleSpanMap,
    pub declaration_span_ids: Vec<Option<String>>,
}

pub fn collect_module_span_map(
    module_id: &str,
    source_file: &str,
    output_file: &str,
    program: &TypedProgram,
) -> CollectedModuleSpanMap {
    let mut collector = SpanCollector::new(module_id, source_file, output_file);
    let declaration_span_ids = program
        .declarations
        .iter()
        .map(|decl| collector.collect_declaration(decl))
        .collect();
    CollectedModuleSpanMap {
        span_map: ModuleSpanMap {
            format_version: SPAN_MAP_FORMAT_VERSION,
            module_id: module_id.to_string(),
            source_file: source_file.to_string(),
            output_file: output_file.to_string(),
            spans: collector.spans,
        },
        declaration_span_ids,
    }
}

struct SpanCollector {
    module_id: String,
    source_file: String,
    next_span_id: usize,
    spans: Vec<DebugSpanRecord>,
}

impl SpanCollector {
    fn new(module_id: &str, source_file: &str, _output_file: &str) -> Self {
        Self {
            module_id: module_id.to_string(),
            source_file: source_file.to_string(),
            next_span_id: 1,
            spans: Vec::new(),
        }
    }

    fn collect_declaration(&mut self, declaration: &TypedDeclaration) -> Option<String> {
        match declaration {
            TypedDeclaration::Function(function) => {
                let span_id = self.push_span(
                    DebugSpanKind::FunctionDecl,
                    function.location,
                    None,
                    Some(function.name.clone()),
                );
                self.collect_expr(&function.body, Some(span_id.clone()));
                Some(span_id)
            }
            TypedDeclaration::Const(const_decl) => {
                let span_id = self.push_span(
                    DebugSpanKind::ConstDecl,
                    const_decl.location,
                    None,
                    Some(const_decl.name.clone()),
                );
                self.collect_expr(&const_decl.value, Some(span_id.clone()));
                Some(span_id)
            }
            TypedDeclaration::Test(test_decl) => {
                let span_id = self.push_span(
                    DebugSpanKind::TestDecl,
                    test_decl.location,
                    None,
                    Some(test_decl.description.clone()),
                );
                for binding in &test_decl.world_bindings {
                    let binding_span_id = self.push_span(
                        DebugSpanKind::TestWorldBinding,
                        binding.location,
                        Some(span_id.clone()),
                        Some(binding.name.clone()),
                    );
                    self.collect_expr(&binding.value, Some(binding_span_id));
                }
                self.collect_expr(&test_decl.body, Some(span_id.clone()));
                Some(span_id)
            }
            TypedDeclaration::JsonCodec(codec_decl) => {
                let span_id = self.push_span(
                    DebugSpanKind::JsonCodecDecl,
                    codec_decl.location,
                    None,
                    Some(codec_decl.target_name.clone()),
                );
                for named_type in &codec_decl.named_types {
                    if let Some(constraint) = &named_type.constraint {
                        self.collect_expr(&constraint.predicate, Some(span_id.clone()));
                    }
                }
                Some(span_id)
            }
            TypedDeclaration::Type(_) | TypedDeclaration::Extern(_) => None,
        }
    }

    fn collect_expr(&mut self, expr: &TypedExpr, parent_span_id: Option<String>) -> String {
        let span_id = self.push_span(expr_kind(&expr.kind), expr.location, parent_span_id, None);
        match &expr.kind {
            TypedExprKind::Literal(_)
            | TypedExprKind::Identifier(_)
            | TypedExprKind::NamespaceMember { .. } => {}
            TypedExprKind::Lambda(lambda) => {
                self.collect_expr(&lambda.body, Some(span_id.clone()));
            }
            TypedExprKind::Call(call) => {
                self.collect_expr(&call.func, Some(span_id.clone()));
                for arg in &call.args {
                    self.collect_expr(arg, Some(span_id.clone()));
                }
            }
            TypedExprKind::ConstructorCall(call) => {
                for arg in &call.args {
                    self.collect_expr(arg, Some(span_id.clone()));
                }
            }
            TypedExprKind::ExternCall(call) => {
                for arg in &call.args {
                    self.collect_expr(arg, Some(span_id.clone()));
                }
            }
            TypedExprKind::MethodCall(call) => {
                self.collect_expr(&call.receiver, Some(span_id.clone()));
                if let MethodSelector::Index(index) = &call.selector {
                    self.collect_expr(index, Some(span_id.clone()));
                }
                for arg in &call.args {
                    self.collect_expr(arg, Some(span_id.clone()));
                }
            }
            TypedExprKind::Binary(binary) => {
                self.collect_expr(&binary.left, Some(span_id.clone()));
                self.collect_expr(&binary.right, Some(span_id.clone()));
            }
            TypedExprKind::Unary(unary) => {
                self.collect_expr(&unary.operand, Some(span_id.clone()));
            }
            TypedExprKind::Match(match_expr) => {
                self.collect_expr(&match_expr.scrutinee, Some(span_id.clone()));
                for arm in &match_expr.arms {
                    let arm_span_id = self.push_span(
                        DebugSpanKind::MatchArm,
                        arm.location,
                        Some(span_id.clone()),
                        None,
                    );
                    if let Some(guard) = &arm.guard {
                        self.collect_expr(guard, Some(arm_span_id.clone()));
                    }
                    self.collect_expr(&arm.body, Some(arm_span_id));
                }
            }
            TypedExprKind::Let(let_expr) => {
                self.collect_expr(&let_expr.value, Some(span_id.clone()));
                self.collect_expr(&let_expr.body, Some(span_id.clone()));
            }
            TypedExprKind::Using(using_expr) => {
                self.collect_expr(&using_expr.value, Some(span_id.clone()));
                self.collect_expr(&using_expr.body, Some(span_id.clone()));
            }
            TypedExprKind::If(if_expr) => {
                self.collect_expr(&if_expr.condition, Some(span_id.clone()));
                self.collect_expr(&if_expr.then_branch, Some(span_id.clone()));
                if let Some(else_branch) = &if_expr.else_branch {
                    self.collect_expr(else_branch, Some(span_id.clone()));
                }
            }
            TypedExprKind::List(list) => {
                for element in &list.elements {
                    self.collect_expr(element, Some(span_id.clone()));
                }
            }
            TypedExprKind::Tuple(tuple) => {
                for element in &tuple.elements {
                    self.collect_expr(element, Some(span_id.clone()));
                }
            }
            TypedExprKind::Record(record) => {
                for field in &record.fields {
                    self.collect_expr(&field.value, Some(span_id.clone()));
                }
            }
            TypedExprKind::MapLiteral(map) => {
                for entry in &map.entries {
                    self.collect_expr(&entry.key, Some(span_id.clone()));
                    self.collect_expr(&entry.value, Some(span_id.clone()));
                }
            }
            TypedExprKind::FieldAccess(field_access) => {
                self.collect_expr(&field_access.object, Some(span_id.clone()));
            }
            TypedExprKind::Index(index) => {
                self.collect_expr(&index.object, Some(span_id.clone()));
                self.collect_expr(&index.index, Some(span_id.clone()));
            }
            TypedExprKind::Map(map) => {
                self.collect_expr(&map.list, Some(span_id.clone()));
                self.collect_expr(&map.func, Some(span_id.clone()));
            }
            TypedExprKind::Filter(filter) => {
                self.collect_expr(&filter.list, Some(span_id.clone()));
                self.collect_expr(&filter.predicate, Some(span_id.clone()));
            }
            TypedExprKind::Fold(fold) => {
                self.collect_expr(&fold.list, Some(span_id.clone()));
                self.collect_expr(&fold.func, Some(span_id.clone()));
                self.collect_expr(&fold.init, Some(span_id.clone()));
            }
            TypedExprKind::Concurrent(concurrent) => {
                self.collect_expr(&concurrent.config.width, Some(span_id.clone()));
                if let Some(jitter_ms) = &concurrent.config.jitter_ms {
                    self.collect_expr(jitter_ms, Some(span_id.clone()));
                }
                if let Some(stop_on) = &concurrent.config.stop_on {
                    self.collect_expr(stop_on, Some(span_id.clone()));
                }
                if let Some(window_ms) = &concurrent.config.window_ms {
                    self.collect_expr(window_ms, Some(span_id.clone()));
                }
                for step in &concurrent.steps {
                    match step {
                        TypedConcurrentStep::Spawn(spawn) => {
                            self.collect_expr(&spawn.expr, Some(span_id.clone()));
                        }
                        TypedConcurrentStep::SpawnEach(spawn_each) => {
                            self.collect_expr(&spawn_each.func, Some(span_id.clone()));
                            self.collect_expr(&spawn_each.list, Some(span_id.clone()));
                        }
                    }
                }
            }
            TypedExprKind::Pipeline(pipeline) => {
                self.collect_expr(&pipeline.left, Some(span_id.clone()));
                self.collect_expr(&pipeline.right, Some(span_id.clone()));
            }
        }
        span_id
    }

    fn push_span(
        &mut self,
        kind: DebugSpanKind,
        location: SourceLocation,
        parent_span_id: Option<String>,
        label: Option<String>,
    ) -> String {
        let span_id = format!("s{}", self.next_span_id);
        self.next_span_id += 1;
        self.spans.push(DebugSpanRecord {
            span_id: span_id.clone(),
            module_id: self.module_id.clone(),
            source_file: self.source_file.clone(),
            kind,
            location: to_debug_span(&self.source_file, location),
            parent_span_id,
            label,
            generated_range: None,
        });
        span_id
    }
}

fn to_debug_span(source_file: &str, location: SourceLocation) -> DebugSourceSpan {
    DebugSourceSpan {
        file: source_file.to_string(),
        start: DebugSourcePoint {
            line: location.start.line,
            column: location.start.column,
            offset: location.start.offset,
        },
        end: DebugSourcePoint {
            line: location.end.line,
            column: location.end.column,
            offset: location.end.offset,
        },
    }
}

fn expr_kind(kind: &TypedExprKind) -> DebugSpanKind {
    match kind {
        TypedExprKind::Literal(_) => DebugSpanKind::ExprLiteral,
        TypedExprKind::Identifier(_) => DebugSpanKind::ExprIdentifier,
        TypedExprKind::NamespaceMember { .. } => DebugSpanKind::ExprNamespaceMember,
        TypedExprKind::Lambda(_) => DebugSpanKind::ExprLambda,
        TypedExprKind::Call(_) => DebugSpanKind::ExprCall,
        TypedExprKind::ConstructorCall(_) => DebugSpanKind::ExprConstructorCall,
        TypedExprKind::ExternCall(_) => DebugSpanKind::ExprExternCall,
        TypedExprKind::MethodCall(_) => DebugSpanKind::ExprMethodCall,
        TypedExprKind::Binary(_) => DebugSpanKind::ExprBinary,
        TypedExprKind::Unary(_) => DebugSpanKind::ExprUnary,
        TypedExprKind::Match(_) => DebugSpanKind::ExprMatch,
        TypedExprKind::Let(_) => DebugSpanKind::ExprLet,
        TypedExprKind::Using(_) => DebugSpanKind::ExprLet,
        TypedExprKind::If(_) => DebugSpanKind::ExprIf,
        TypedExprKind::List(_) => DebugSpanKind::ExprList,
        TypedExprKind::Tuple(_) => DebugSpanKind::ExprTuple,
        TypedExprKind::Record(_) => DebugSpanKind::ExprRecord,
        TypedExprKind::MapLiteral(_) => DebugSpanKind::ExprMapLiteral,
        TypedExprKind::FieldAccess(_) => DebugSpanKind::ExprFieldAccess,
        TypedExprKind::Index(_) => DebugSpanKind::ExprIndex,
        TypedExprKind::Map(_) => DebugSpanKind::ExprMap,
        TypedExprKind::Filter(_) => DebugSpanKind::ExprFilter,
        TypedExprKind::Fold(_) => DebugSpanKind::ExprFold,
        TypedExprKind::Concurrent(_) => DebugSpanKind::ExprConcurrent,
        TypedExprKind::Pipeline(_) => DebugSpanKind::ExprPipeline,
    }
}
