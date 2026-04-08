use sigil_ast::*;
use sigil_typechecker::EffectCatalog;

const INDENT: &str = "  ";

pub fn print_canonical_program(program: &Program) -> String {
    print_canonical_program_with_effects(program, None)
}

pub fn print_canonical_program_with_effects(
    program: &Program,
    effect_catalog: Option<&EffectCatalog>,
) -> String {
    let mut printer = Printer::new();
    printer.effect_catalog = effect_catalog.cloned();
    printer.program(program);
    printer.finish()
}

pub fn print_canonical_type(ty: &Type) -> String {
    print_canonical_type_with_effects(ty, None)
}

pub fn print_canonical_type_with_effects(
    ty: &Type,
    effect_catalog: Option<&EffectCatalog>,
) -> String {
    let mut printer = Printer::new();
    printer.effect_catalog = effect_catalog.cloned();
    printer.type_text(ty)
}

pub fn print_canonical_type_definition(type_def: &TypeDef) -> String {
    print_canonical_type_definition_with_effects(type_def, None)
}

pub fn print_canonical_type_definition_with_effects(
    type_def: &TypeDef,
    effect_catalog: Option<&EffectCatalog>,
) -> String {
    let mut printer = Printer::new();
    printer.effect_catalog = effect_catalog.cloned();
    printer.type_def_text(type_def)
}

pub fn print_canonical_expr(expr: &Expr) -> String {
    print_canonical_expr_with_effects(expr, None)
}

pub fn print_canonical_expr_with_effects(
    expr: &Expr,
    effect_catalog: Option<&EffectCatalog>,
) -> String {
    let mut printer = Printer::new();
    printer.effect_catalog = effect_catalog.cloned();
    printer.expr(expr, 0, 0)
}

struct Printer {
    out: String,
    effect_catalog: Option<EffectCatalog>,
}

impl Printer {
    fn new() -> Self {
        Self {
            out: String::new(),
            effect_catalog: None,
        }
    }

    fn finish(mut self) -> String {
        if !self.out.ends_with('\n') {
            self.out.push('\n');
        }
        self.out
    }

    fn push(&mut self, text: &str) {
        self.out.push_str(text);
    }

    fn newline(&mut self) {
        self.out.push('\n');
    }

    fn indent(&mut self, level: usize) {
        for _ in 0..level {
            self.push(INDENT);
        }
    }

    fn program(&mut self, program: &Program) {
        for (index, declaration) in program.declarations.iter().enumerate() {
            if index > 0 {
                self.newline();
                self.newline();
            }
            self.declaration(declaration, 0);
        }
    }

    fn declaration(&mut self, declaration: &Declaration, indent: usize) {
        match declaration {
            Declaration::Function(function) => self.function_decl(function, indent),
            Declaration::Transform(transform_decl) => self.transform_decl(transform_decl, indent),
            Declaration::Type(type_decl) => self.type_decl(type_decl, indent),
            Declaration::Label(label_decl) => self.label_decl(label_decl, indent),
            Declaration::Rule(rule_decl) => self.rule_decl(rule_decl, indent),
            Declaration::Effect(effect_decl) => self.effect_decl(effect_decl, indent),
            Declaration::Const(const_decl) => {
                self.indent(indent);
                self.push("c ");
                self.push(&const_decl.name);
                self.push("=");
                self.push(&self.const_value(const_decl));
            }
            Declaration::Test(test_decl) => self.test_decl(test_decl, indent),
            Declaration::Extern(extern_decl) => self.extern_decl(extern_decl, indent),
        }
    }

    fn function_decl(&mut self, function: &FunctionDecl, indent: usize) {
        self.indent(indent);
        self.push("λ");
        self.push(&function.name);
        self.type_params(&function.type_params);
        self.push("(");
        self.params(&function.params);
        self.push(")=>");
        self.effects(&function.effects, None);
        if let Some(return_type) = &function.return_type {
            self.push(&self.type_text(return_type));
        }

        if let Some(requires) = &function.requires {
            self.newline();
            self.indent(indent);
            self.push("requires ");
            self.push(&self.expr(requires, indent, 0));
        }

        if let Some(ensures) = &function.ensures {
            self.newline();
            self.indent(indent);
            self.push("ensures ");
            self.push(&self.expr(ensures, indent, 0));
        }

        if function.requires.is_some() || function.ensures.is_some() {
            self.newline();
            self.indent(indent);
        }

        match &function.body {
            Expr::Match(match_expr) => {
                if function.requires.is_none() && function.ensures.is_none() {
                    self.push(" ");
                }
                self.match_expr(match_expr, indent);
            }
            Expr::Let(let_expr) => {
                self.push("=");
                self.block_expr(let_expr, indent);
            }
            body => {
                self.push("=");
                self.push(&self.expr(body, indent, 0));
            }
        }
    }

    fn transform_decl(&mut self, transform_decl: &TransformDecl, indent: usize) {
        self.indent(indent);
        self.push("transform ");
        let before = self.out.len();
        self.function_decl(&transform_decl.function, 0);
        if self.out.len() == before {
            self.function_decl(&transform_decl.function, indent);
        }
    }

    fn test_decl(&mut self, test_decl: &TestDecl, indent: usize) {
        self.indent(indent);
        self.push("test ");
        self.push(&string_literal(&test_decl.description));
        if !test_decl.effects.is_empty() {
            self.push(" =>");
            self.effects(&test_decl.effects, None);
            if self.out.ends_with(' ') {
                self.out.pop();
            }
        }
        if !test_decl.world_bindings.is_empty() {
            self.push(" world {");
            for binding in &test_decl.world_bindings {
                self.newline();
                self.indent(indent + 1);
                self.push("c ");
                self.push(&binding.name);
                self.push("=");
                self.push(&self.const_value(binding));
            }
            self.newline();
            self.indent(indent);
            self.push("}");
        }
        self.push(" {");
        self.block_body(&test_decl.body, indent + 1);
        self.newline();
        self.indent(indent);
        self.push("}");
    }

    fn effect_decl(&mut self, effect_decl: &EffectDecl, indent: usize) {
        self.indent(indent);
        self.push("effect ");
        self.push(&effect_decl.name);
        self.push("=");
        self.effects(&effect_decl.effects, Some(effect_decl.name.as_str()));
        if self.out.ends_with(' ') {
            self.out.pop();
        }
    }

    fn label_decl(&mut self, label_decl: &LabelDecl, indent: usize) {
        self.indent(indent);
        self.push("label ");
        self.push(&label_decl.name);
        if !label_decl.combines.is_empty() {
            self.push(" combines ");
            self.push(&self.label_refs_text(&label_decl.combines));
        }
    }

    fn rule_decl(&mut self, rule_decl: &RuleDecl, indent: usize) {
        self.indent(indent);
        self.push("rule ");
        self.push(&self.label_refs_text(&rule_decl.labels));
        self.push(" for ");
        self.push(&self.member_ref_text(&rule_decl.boundary));
        self.push("=");
        self.push(&self.rule_action_text(&rule_decl.action));
    }

    fn extern_decl(&mut self, extern_decl: &ExternDecl, indent: usize) {
        self.indent(indent);
        self.push("e ");
        self.push(&extern_decl.module_path.join("::"));
        if let Some(members) = &extern_decl.members {
            self.push(":{");
            for (index, member) in members.iter().enumerate() {
                if index > 0 {
                    self.push(",");
                }
                self.push(&member.name);
                self.push(":");
                self.push(&self.type_text(&member.member_type));
            }
            self.push("}");
        }
    }

    fn type_decl(&mut self, type_decl: &TypeDecl, indent: usize) {
        self.indent(indent);
        self.push("t ");
        self.push(&type_decl.name);
        self.type_params(&type_decl.type_params);
        self.push("=");
        self.push(&self.type_def_text(&type_decl.definition));
        if let Some(constraint) = &type_decl.constraint {
            self.push(" where ");
            self.push(&self.expr(constraint, 0, 0));
        }
        if !type_decl.labels.is_empty() {
            self.push(" label ");
            self.push(&self.label_refs_text(&type_decl.labels));
        }
    }

    fn params(&mut self, params: &[Param]) {
        for (index, param) in params.iter().enumerate() {
            if index > 0 {
                self.push(",");
            }
            self.push(&param.name);
            self.push(":");
            if param.is_mutable {
                self.push("mut ");
            }
            if let Some(type_annotation) = &param.type_annotation {
                self.push(&self.type_text(type_annotation));
            }
        }
    }

    fn effects(&mut self, effects: &[String], exclude_alias: Option<&str>) {
        let surface_effects = self
            .effect_catalog
            .as_ref()
            .and_then(|catalog| catalog.canonicalize_names(effects, exclude_alias).ok())
            .unwrap_or_else(|| effects.to_vec());

        for effect in &surface_effects {
            self.push("!");
            self.push(effect);
        }
        if !surface_effects.is_empty() {
            self.push(" ");
        }
    }

    fn label_refs_text(&self, refs: &[LabelRef]) -> String {
        let mut items: Vec<String> = refs.iter().map(|label| self.label_ref_text(label)).collect();
        items.sort();
        match items.as_slice() {
            [] => "[]".to_string(),
            [single] => single.clone(),
            _ => format!("[{}]", items.join(",")),
        }
    }

    fn label_ref_text(&self, label: &LabelRef) -> String {
        if label.module_path.is_empty() {
            return label.name.clone();
        }
        format!("{}.{}", module_path_text(&label.module_path), label.name)
    }

    fn member_ref_text(&self, reference: &MemberRef) -> String {
        if reference.module_path.is_empty() {
            return reference.member.clone();
        }
        format!("{}.{}", module_path_text(&reference.module_path), reference.member)
    }

    fn rule_action_text(&self, action: &RuleAction) -> String {
        match action {
            RuleAction::Allow { .. } => "Allow()".to_string(),
            RuleAction::Block { .. } => "Block()".to_string(),
            RuleAction::Through { transform, .. } => {
                format!("Through({})", self.member_ref_text(transform))
            }
        }
    }

    fn type_params(&mut self, type_params: &[String]) {
        if type_params.is_empty() {
            return;
        }
        self.push("[");
        for (index, type_param) in type_params.iter().enumerate() {
            if index > 0 {
                self.push(",");
            }
            self.push(type_param);
        }
        self.push("]");
    }

    fn type_text(&self, ty: &Type) -> String {
        match ty {
            Type::Primitive(primitive) => primitive.name.to_string(),
            Type::List(list) => format!("[{}]", self.type_text(&list.element_type)),
            Type::Map(map) => format!(
                "{{{}↦{}}}",
                self.type_text(&map.key_type),
                self.type_text(&map.value_type)
            ),
            Type::Function(function) => {
                let params = function
                    .param_types
                    .iter()
                    .map(|ty| self.type_text(ty))
                    .collect::<Vec<_>>()
                    .join(",");
                let effects = function.effects.to_vec();
                let effect_text = self
                    .effect_catalog
                    .as_ref()
                    .and_then(|catalog| catalog.canonicalize_names(&effects, None).ok())
                    .unwrap_or(effects)
                    .into_iter()
                    .map(|effect| format!("!{}", effect))
                    .collect::<String>();
                if effect_text.is_empty() {
                    format!("λ({})=>{}", params, self.type_text(&function.return_type))
                } else {
                    format!(
                        "λ({})=>{} {}",
                        params,
                        effect_text,
                        self.type_text(&function.return_type)
                    )
                }
            }
            Type::Constructor(constructor) => {
                if constructor.type_args.is_empty() {
                    constructor.name.clone()
                } else {
                    let args = constructor
                        .type_args
                        .iter()
                        .map(|ty| self.type_text(ty))
                        .collect::<Vec<_>>()
                        .join(",");
                    format!("{}[{}]", constructor.name, args)
                }
            }
            Type::Variable(variable) => variable.name.clone(),
            Type::Tuple(tuple) => {
                let elements = tuple
                    .types
                    .iter()
                    .map(|ty| self.type_text(ty))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("({})", elements)
            }
            Type::Qualified(qualified) => {
                let args = if qualified.type_args.is_empty() {
                    String::new()
                } else {
                    format!(
                        "[{}]",
                        qualified
                            .type_args
                            .iter()
                            .map(|ty| self.type_text(ty))
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                };
                if is_project_types_module(&qualified.module_path) {
                    format!("µ{}{}", qualified.type_name, args)
                } else {
                    format!(
                        "{}.{}{}",
                        module_path_text(&qualified.module_path),
                        qualified.type_name,
                        args
                    )
                }
            }
        }
    }

    fn type_def_text(&self, type_def: &TypeDef) -> String {
        match type_def {
            TypeDef::Sum(sum) => sum
                .variants
                .iter()
                .map(|variant| {
                    if variant.types.is_empty() {
                        format!("{}()", variant.name)
                    } else {
                        format!(
                            "{}({})",
                            variant.name,
                            variant
                                .types
                                .iter()
                                .map(|ty| self.type_text(ty))
                                .collect::<Vec<_>>()
                                .join(",")
                        )
                    }
                })
                .collect::<Vec<_>>()
                .join("|"),
            TypeDef::Product(product) => format!(
                "{{{}}}",
                product
                    .fields
                    .iter()
                    .map(|field| format!("{}:{}", field.name, self.type_text(&field.field_type)))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            TypeDef::Alias(alias) => self.type_text(&alias.aliased_type),
        }
    }

    fn const_value(&self, const_decl: &ConstDecl) -> String {
        match &const_decl.type_annotation {
            Some(type_annotation) => {
                format!(
                    "({}:{})",
                    self.expr(&const_decl.value, 0, 0),
                    self.type_text(type_annotation)
                )
            }
            None => self.expr(&const_decl.value, 0, 0),
        }
    }

    fn block_expr(&mut self, let_expr: &LetExpr, indent: usize) {
        self.push("{");
        self.block_body(&Expr::Let(Box::new(let_expr.clone())), indent + 1);
        self.newline();
        self.indent(indent);
        self.push("}");
    }

    fn block_body(&mut self, expr: &Expr, indent: usize) {
        match expr {
            Expr::Let(let_expr) => {
                let (bindings, body) = flatten_lets(let_expr);
                for binding in bindings {
                    self.newline();
                    self.indent(indent);
                    self.push("l ");
                    self.push(&self.pattern_text(&binding.pattern));
                    self.push("=");
                    self.push(&self.expr(&binding.value, indent, 0));
                    self.push(";");
                }
                self.newline();
                self.indent(indent);
                self.push(&self.expr(body, indent, 0));
            }
            other => {
                self.newline();
                self.indent(indent);
                self.push(&self.expr(other, indent, 0));
            }
        }
    }

    fn expr(&self, expr: &Expr, indent: usize, parent_prec: u8) -> String {
        let prec = precedence(expr);
        let text = match expr {
            Expr::Literal(literal) => literal_text(literal),
            Expr::Identifier(identifier) => identifier.name.clone(),
            Expr::Lambda(lambda) => self.lambda_expr(lambda, indent),
            Expr::Application(application) => {
                let func = self.wrap_expr(&application.func, indent, precedence(expr));
                let args = application
                    .args
                    .iter()
                    .map(|arg| self.expr(arg, indent, 0))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("{}({})", func, args)
            }
            Expr::Binary(binary) => {
                let op_prec = precedence(expr);
                let left = self.wrap_expr(&binary.left, indent, op_prec);
                let right = self.wrap_expr(&binary.right, indent, op_prec.saturating_add(1));
                match binary.operator {
                    BinaryOperator::And | BinaryOperator::Or => {
                        format!("{} {} {}", left, binary.operator, right)
                    }
                    _ => format!("{}{}{}", left, binary.operator, right),
                }
            }
            Expr::Unary(unary) => format!(
                "{}{}",
                unary.operator,
                self.wrap_expr(&unary.operand, indent, precedence(expr))
            ),
            Expr::Match(match_expr) => self.match_text(match_expr, indent),
            Expr::Let(let_expr) => self.let_text(let_expr, indent),
            Expr::If(if_expr) => {
                let else_branch = if_expr
                    .else_branch
                    .as_ref()
                    .map(|branch| self.expr(branch, indent, 0))
                    .unwrap_or_else(|| "()".to_string());
                format!(
                    "{}?{}:{}",
                    self.wrap_expr(&if_expr.condition, indent, precedence(expr)),
                    self.expr(&if_expr.then_branch, indent, 0),
                    else_branch
                )
            }
            Expr::List(list) => {
                let elements = list
                    .elements
                    .iter()
                    .map(|element| self.expr(element, indent, 0))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("[{}]", elements)
            }
            Expr::Record(record) => {
                let fields = record
                    .fields
                    .iter()
                    .map(|field| format!("{}:{}", field.name, self.expr(&field.value, indent, 0)))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("{{{}}}", fields)
            }
            Expr::MapLiteral(map) => {
                if map.entries.is_empty() {
                    "{↦}".to_string()
                } else {
                    let entries = map
                        .entries
                        .iter()
                        .map(|entry| {
                            format!(
                                "{}↦{}",
                                self.expr(&entry.key, indent, 0),
                                self.expr(&entry.value, indent, 0)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(",");
                    format!("{{{}}}", entries)
                }
            }
            Expr::Tuple(tuple) => {
                let elements = tuple
                    .elements
                    .iter()
                    .map(|element| self.expr(element, indent, 0))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("({})", elements)
            }
            Expr::FieldAccess(access) => {
                format!(
                    "{}.{}",
                    self.wrap_expr(&access.object, indent, precedence(expr)),
                    access.field
                )
            }
            Expr::Index(index) => {
                format!(
                    "{}[{}]",
                    self.wrap_expr(&index.object, indent, precedence(expr)),
                    self.expr(&index.index, indent, 0)
                )
            }
            Expr::Pipeline(pipeline) => {
                let operator = match pipeline.operator {
                    PipelineOperator::Pipe => "|>",
                    PipelineOperator::ComposeFwd => ">>",
                    PipelineOperator::ComposeBwd => "<<",
                };
                format!(
                    "{}{}{}",
                    self.wrap_expr(&pipeline.left, indent, precedence(expr)),
                    operator,
                    self.wrap_expr(&pipeline.right, indent, precedence(expr).saturating_add(1))
                )
            }
            Expr::Map(map) => format!(
                "{} map {}",
                self.wrap_expr(&map.list, indent, precedence(expr)),
                self.wrap_expr(&map.func, indent, precedence(expr).saturating_add(1))
            ),
            Expr::Filter(filter) => format!(
                "{} filter {}",
                self.wrap_expr(&filter.list, indent, precedence(expr)),
                self.wrap_expr(
                    &filter.predicate,
                    indent,
                    precedence(expr).saturating_add(1)
                )
            ),
            Expr::Fold(fold) => format!(
                "{} reduce {} from {}",
                self.wrap_expr(&fold.list, indent, precedence(expr)),
                self.wrap_expr(&fold.func, indent, precedence(expr).saturating_add(1)),
                self.wrap_expr(&fold.init, indent, precedence(expr).saturating_add(1))
            ),
            Expr::Concurrent(concurrent) => self.concurrent_text(concurrent, indent),
            Expr::MemberAccess(member) => {
                if is_project_types_module(&member.namespace) {
                    format!("µ{}", member.member)
                } else {
                    format!("{}.{}", module_path_text(&member.namespace), member.member)
                }
            }
            Expr::TypeAscription(ascription) => {
                format!(
                    "({}:{})",
                    self.expr(&ascription.expr, indent, 0),
                    self.type_text(&ascription.ascribed_type)
                )
            }
        };

        if prec < parent_prec {
            format!("({})", text)
        } else {
            text
        }
    }

    fn wrap_expr(&self, expr: &Expr, indent: usize, parent_prec: u8) -> String {
        self.expr(expr, indent, parent_prec)
    }

    fn lambda_expr(&self, lambda: &LambdaExpr, indent: usize) -> String {
        let params = lambda
            .params
            .iter()
            .map(|param| {
                let type_annotation = param
                    .type_annotation
                    .as_ref()
                    .map(|ty| self.type_text(ty))
                    .unwrap_or_default();
                if param.is_mutable {
                    format!("{}:mut {}", param.name, type_annotation)
                } else {
                    format!("{}:{}", param.name, type_annotation)
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        let effects = lambda
            .effects
            .iter()
            .map(|effect| format!("!{}", effect))
            .collect::<String>();
        let head = if effects.is_empty() {
            format!("λ({})=>{}", params, self.type_text(&lambda.return_type))
        } else {
            format!(
                "λ({})=>{} {}",
                params,
                effects,
                self.type_text(&lambda.return_type)
            )
        };

        match &lambda.body {
            Expr::Match(match_expr) => format!("{} {}", head, self.match_text(match_expr, indent)),
            Expr::Let(let_expr) => format!("{}={}", head, self.let_text(let_expr, indent)),
            body => format!("{}={}", head, self.expr(body, indent, 0)),
        }
    }

    fn match_expr(&mut self, match_expr: &MatchExpr, indent: usize) {
        self.push(&self.match_text(match_expr, indent));
    }

    fn match_text(&self, match_expr: &MatchExpr, indent: usize) -> String {
        let mut out = String::new();
        out.push_str("match ");
        out.push_str(&self.expr(&match_expr.scrutinee, indent, 0));
        out.push('{');
        for (index, arm) in match_expr.arms.iter().enumerate() {
            out.push('\n');
            out.push_str(&INDENT.repeat(indent + 1));
            out.push_str(&self.pattern_text(&arm.pattern));
            if let Some(guard) = &arm.guard {
                out.push_str(" when ");
                out.push_str(&self.expr(guard, indent + 1, 0));
            }
            out.push_str("=>");
            out.push_str(&self.match_arm_body(&arm.body, indent + 1));
            if index + 1 < match_expr.arms.len() {
                out.push('|');
            }
        }
        out.push('\n');
        out.push_str(&INDENT.repeat(indent));
        out.push('}');
        out
    }

    fn match_arm_body(&self, body: &Expr, indent: usize) -> String {
        match body {
            Expr::Match(match_expr) => self.match_text(match_expr, indent),
            Expr::Let(let_expr) => self.let_text(let_expr, indent),
            other => self.expr(other, indent, 0),
        }
    }

    fn let_text(&self, let_expr: &LetExpr, indent: usize) -> String {
        let (bindings, body) = flatten_lets(let_expr);
        let mut out = String::from("{");
        for binding in bindings {
            out.push('\n');
            out.push_str(&INDENT.repeat(indent + 1));
            out.push_str("l ");
            out.push_str(&self.pattern_text(&binding.pattern));
            out.push('=');
            out.push_str(&self.expr(&binding.value, indent + 1, 0));
            out.push(';');
        }
        out.push('\n');
        out.push_str(&INDENT.repeat(indent + 1));
        out.push_str(&self.expr(body, indent + 1, 0));
        out.push('\n');
        out.push_str(&INDENT.repeat(indent));
        out.push('}');
        out
    }

    fn concurrent_text(&self, concurrent: &ConcurrentExpr, indent: usize) -> String {
        let width = if self.concurrent_width_can_be_bare(&concurrent.width) {
            self.expr(&concurrent.width, indent, 0)
        } else {
            format!("({})", self.expr(&concurrent.width, indent, 0))
        };
        let mut out = format!("concurrent {}@{}", concurrent.name, width);
        if let Some(policy) = self.concurrent_policy_for_print(concurrent) {
            out.push(':');
            out.push_str(&self.record_text(&policy, indent));
        }
        out.push('{');
        for step in &concurrent.steps {
            out.push('\n');
            out.push_str(&INDENT.repeat(indent + 1));
            match step {
                ConcurrentStep::Spawn(spawn) => {
                    out.push_str("spawn ");
                    out.push_str(&self.expr(&spawn.expr, indent + 1, 0));
                }
                ConcurrentStep::SpawnEach(spawn_each) => {
                    out.push_str("spawnEach ");
                    out.push_str(&self.expr(&spawn_each.list, indent + 1, 0));
                    out.push(' ');
                    out.push_str(&self.expr(&spawn_each.func, indent + 1, 0));
                }
            }
        }
        out.push('\n');
        out.push_str(&INDENT.repeat(indent));
        out.push('}');
        out
    }

    fn concurrent_policy_for_print(&self, concurrent: &ConcurrentExpr) -> Option<RecordExpr> {
        let policy = concurrent.policy.as_ref()?;
        let fields = policy
            .fields
            .iter()
            .filter(|field| match field.name.as_str() {
                "jitterMs" | "windowMs" => !self.is_none_expr(&field.value),
                "stopOn" => !self.is_default_stop_on_expr(&field.value),
                _ => true,
            })
            .cloned()
            .collect::<Vec<_>>();

        if fields.is_empty() {
            None
        } else {
            Some(RecordExpr {
                fields,
                location: policy.location,
            })
        }
    }

    fn concurrent_width_can_be_bare(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Identifier(_) | Expr::MemberAccess(_) => true,
            Expr::Literal(literal) => matches!(literal.value, LiteralValue::Int(_)),
            Expr::Application(application) => self.concurrent_width_can_be_bare(&application.func),
            Expr::FieldAccess(access) => self.concurrent_width_can_be_bare(&access.object),
            Expr::Index(index) => self.concurrent_width_can_be_bare(&index.object),
            _ => false,
        }
    }

    fn is_none_expr(&self, expr: &Expr) -> bool {
        matches!(
            expr,
            Expr::Application(application)
                if matches!(&application.func, Expr::Identifier(id) if id.name == "None")
                    && application.args.is_empty()
        )
    }

    fn is_default_stop_on_expr(&self, expr: &Expr) -> bool {
        let Expr::Lambda(lambda) = expr else {
            return false;
        };

        if !lambda.effects.is_empty() || lambda.params.len() != 1 {
            return false;
        }

        if !matches!(
            lambda.return_type,
            Type::Primitive(PrimitiveType {
                name: PrimitiveName::Bool,
                ..
            })
        ) {
            return false;
        }

        matches!(
            &lambda.body,
            Expr::Literal(LiteralExpr {
                value: LiteralValue::Bool(false),
                ..
            })
        )
    }

    fn record_text(&self, record: &RecordExpr, indent: usize) -> String {
        let fields = record
            .fields
            .iter()
            .map(|field| format!("{}:{}", field.name, self.expr(&field.value, indent, 0)))
            .collect::<Vec<_>>()
            .join(",");
        format!("{{{}}}", fields)
    }

    fn pattern_text(&self, pattern: &Pattern) -> String {
        match pattern {
            Pattern::Literal(literal) => pattern_literal_text(literal),
            Pattern::Identifier(identifier) => identifier.name.clone(),
            Pattern::Wildcard(_) => "_".to_string(),
            Pattern::Constructor(constructor) => {
                let prefix = if constructor.module_path.is_empty() {
                    constructor.name.clone()
                } else if is_project_types_module(&constructor.module_path) {
                    format!("µ{}", constructor.name)
                } else {
                    format!(
                        "{}.{}",
                        module_path_text(&constructor.module_path),
                        constructor.name
                    )
                };
                if constructor.patterns.is_empty() {
                    format!("{}()", prefix)
                } else {
                    format!(
                        "{}({})",
                        prefix,
                        constructor
                            .patterns
                            .iter()
                            .map(|pattern| self.pattern_text(pattern))
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                }
            }
            Pattern::List(list) => {
                let mut parts = list
                    .patterns
                    .iter()
                    .map(|pattern| self.pattern_text(pattern))
                    .collect::<Vec<_>>();
                if let Some(rest) = &list.rest {
                    parts.push(format!(".{}", rest));
                }
                format!("[{}]", parts.join(","))
            }
            Pattern::Record(record) => format!(
                "{{{}}}",
                record
                    .fields
                    .iter()
                    .map(|field| match &field.pattern {
                        Some(pattern) => format!("{}:{}", field.name, self.pattern_text(pattern)),
                        None => field.name.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Pattern::Tuple(tuple) => format!(
                "({})",
                tuple
                    .patterns
                    .iter()
                    .map(|pattern| self.pattern_text(pattern))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}

struct LetBindingRef<'a> {
    pattern: &'a Pattern,
    value: &'a Expr,
}

fn flatten_lets<'a>(let_expr: &'a LetExpr) -> (Vec<LetBindingRef<'a>>, &'a Expr) {
    let mut bindings = Vec::new();
    let mut current = let_expr;
    loop {
        bindings.push(LetBindingRef {
            pattern: &current.pattern,
            value: &current.value,
        });
        match &current.body {
            Expr::Let(next) => current = next,
            body => return (bindings, body),
        }
    }
}

fn precedence(expr: &Expr) -> u8 {
    match expr {
        Expr::Let(_) | Expr::Match(_) | Expr::If(_) | Expr::Lambda(_) | Expr::Concurrent(_) => 1,
        Expr::Pipeline(_) | Expr::Map(_) | Expr::Filter(_) | Expr::Fold(_) => 2,
        Expr::Binary(binary) => match binary.operator {
            BinaryOperator::Pipe | BinaryOperator::ComposeFwd | BinaryOperator::ComposeBwd => 2,
            BinaryOperator::Or => 3,
            BinaryOperator::And => 4,
            BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::Greater
            | BinaryOperator::LessEq
            | BinaryOperator::GreaterEq => 5,
            BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Append
            | BinaryOperator::ListAppend => 6,
            BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::Power => 7,
        },
        Expr::Unary(_) => 8,
        Expr::Application(_) | Expr::FieldAccess(_) | Expr::Index(_) => 9,
        Expr::TypeAscription(_) => 10,
        Expr::Literal(_)
        | Expr::Identifier(_)
        | Expr::List(_)
        | Expr::Record(_)
        | Expr::MapLiteral(_)
        | Expr::Tuple(_)
        | Expr::MemberAccess(_) => 11,
    }
}

fn literal_text(literal: &LiteralExpr) -> String {
    match &literal.value {
        LiteralValue::Int(value) => value.to_string(),
        LiteralValue::Float(value) => {
            let mut text = value.to_string();
            if !text.contains('.') && !text.contains('e') && !text.contains('E') {
                text.push_str(".0");
            }
            text
        }
        LiteralValue::String(value) => string_literal(value),
        LiteralValue::Char(value) => char_literal(*value),
        LiteralValue::Bool(value) => value.to_string(),
        LiteralValue::Unit => "()".to_string(),
    }
}

fn module_path_text(module_path: &[String]) -> String {
    if module_path == ["src".to_string(), "types".to_string()] {
        return "µ".to_string();
    }
    if let Some((root, rest)) = module_path.split_first() {
        if let Some(sigl) = root_sigil(root) {
            if rest.is_empty() {
                return sigl.to_string();
            }
            return format!("{}{}", sigl, rest.join("::"));
        }
    }
    module_path.join("::")
}

fn is_project_types_module(module_path: &[String]) -> bool {
    module_path.len() == 2 && module_path[0] == "src" && module_path[1] == "types"
}

fn root_sigil(root: &str) -> Option<&'static str> {
    match root {
        "stdlib" => Some("§"),
        "src" => Some("•"),
        "core" => Some("¶"),
        "config" => Some("¤"),
        "world" => Some("†"),
        "test" => Some("※"),
        _ => None,
    }
}

fn pattern_literal_text(literal: &LiteralPattern) -> String {
    match &literal.value {
        PatternLiteralValue::Int(value) => value.to_string(),
        PatternLiteralValue::Float(value) => {
            let mut text = value.to_string();
            if !text.contains('.') && !text.contains('e') && !text.contains('E') {
                text.push_str(".0");
            }
            text
        }
        PatternLiteralValue::String(value) => string_literal(value),
        PatternLiteralValue::Char(value) => char_literal(*value),
        PatternLiteralValue::Bool(value) => value.to_string(),
        PatternLiteralValue::Unit => "()".to_string(),
    }
}

fn string_literal(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push('\n'),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }
    escaped.push('"');
    escaped
}

fn char_literal(value: char) -> String {
    let escaped = match value {
        '\\' => "\\\\".to_string(),
        '\'' => "\\'".to_string(),
        '\n' => "\\n".to_string(),
        '\r' => "\\r".to_string(),
        '\t' => "\\t".to_string(),
        other => other.to_string(),
    };
    format!("'{}'", escaped)
}

#[cfg(test)]
mod tests {
    use super::string_literal;

    #[test]
    fn multiline_strings_print_with_literal_newlines() {
        assert_eq!(string_literal("hello\nworld"), "\"hello\nworld\"");
    }

    #[test]
    fn multiline_strings_still_escape_tabs_and_quotes() {
        assert_eq!(string_literal("say\t\"hi\""), "\"say\\t\\\"hi\\\"\"");
    }
}
