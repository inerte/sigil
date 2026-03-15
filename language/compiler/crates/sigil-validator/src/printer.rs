use sigil_ast::*;

const INDENT: &str = "  ";

pub fn print_canonical_program(program: &Program) -> String {
    let mut printer = Printer::new();
    printer.program(program);
    printer.finish()
}

struct Printer {
    out: String,
}

impl Printer {
    fn new() -> Self {
        Self { out: String::new() }
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
            Declaration::Type(type_decl) => self.type_decl(type_decl, indent),
            Declaration::Import(import_decl) => {
                self.indent(indent);
                self.push("i ");
                self.push(&import_decl.module_path.join("::"));
            }
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
        self.effects(&function.effects);
        if let Some(return_type) = &function.return_type {
            self.push(&self.type_text(return_type));
        }

        match &function.body {
            Expr::Match(match_expr) => {
                self.push(" ");
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

    fn test_decl(&mut self, test_decl: &TestDecl, indent: usize) {
        self.indent(indent);
        self.push("test ");
        self.push(&string_literal(&test_decl.description));
        if !test_decl.effects.is_empty() {
            self.push(" =>");
            self.effects(&test_decl.effects);
        }
        self.push(" {");
        self.block_body(&test_decl.body, indent + 1);
        self.newline();
        self.indent(indent);
        self.push("}");
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
        match &type_decl.definition {
            TypeDef::Sum(sum) => {
                for (index, variant) in sum.variants.iter().enumerate() {
                    if index > 0 {
                        self.push("|");
                    }
                    self.push(&variant.name);
                    if variant.types.is_empty() {
                        self.push("()");
                    } else {
                        self.push("(");
                        for (arg_index, ty) in variant.types.iter().enumerate() {
                            if arg_index > 0 {
                                self.push(",");
                            }
                            self.push(&self.type_text(ty));
                        }
                        self.push(")");
                    }
                }
            }
            TypeDef::Product(product) => {
                self.push("{");
                for (index, field) in product.fields.iter().enumerate() {
                    if index > 0 {
                        self.push(",");
                    }
                    self.push(&field.name);
                    self.push(":");
                    self.push(&self.type_text(&field.field_type));
                }
                self.push("}");
            }
            TypeDef::Alias(alias) => self.push(&self.type_text(&alias.aliased_type)),
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

    fn effects(&mut self, effects: &[String]) {
        for effect in effects {
            self.push("!");
            self.push(effect);
        }
        if !effects.is_empty() {
            self.push(" ");
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
            Type::Map(map) => format!("{{{}↦{}}}", self.type_text(&map.key_type), self.type_text(&map.value_type)),
            Type::Function(function) => {
                let params = function
                    .param_types
                    .iter()
                    .map(|ty| self.type_text(ty))
                    .collect::<Vec<_>>()
                    .join(",");
                let effects = function
                    .effects
                    .iter()
                    .map(|effect| format!("!{}", effect))
                    .collect::<String>();
                if effects.is_empty() {
                    format!("λ({})=>{}", params, self.type_text(&function.return_type))
                } else {
                    format!("λ({})=>{} {}", params, effects, self.type_text(&function.return_type))
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
                format!("{}.{}{}", qualified.module_path.join("::"), qualified.type_name, args)
            }
        }
    }

    fn const_value(&self, const_decl: &ConstDecl) -> String {
        match &const_decl.type_annotation {
            Some(type_annotation) => {
                format!("({}:{})", self.expr(&const_decl.value, 0, 0), self.type_text(type_annotation))
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
            Expr::Unary(unary) => format!("{}{}", unary.operator, self.wrap_expr(&unary.operand, indent, precedence(expr))),
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
                        .map(|entry| format!("{}↦{}", self.expr(&entry.key, indent, 0), self.expr(&entry.value, indent, 0)))
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
                format!("{}.{}", self.wrap_expr(&access.object, indent, precedence(expr)), access.field)
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
                "{}↦{}",
                self.wrap_expr(&map.list, indent, precedence(expr)),
                self.wrap_expr(&map.func, indent, precedence(expr).saturating_add(1))
            ),
            Expr::Filter(filter) => format!(
                "{}⊳{}",
                self.wrap_expr(&filter.list, indent, precedence(expr)),
                self.wrap_expr(&filter.predicate, indent, precedence(expr).saturating_add(1))
            ),
            Expr::Fold(fold) => format!(
                "{}⊕{}⊕{}",
                self.wrap_expr(&fold.list, indent, precedence(expr)),
                self.wrap_expr(&fold.func, indent, precedence(expr).saturating_add(1)),
                self.wrap_expr(&fold.init, indent, precedence(expr).saturating_add(1))
            ),
            Expr::MemberAccess(member) => format!("{}.{}", member.namespace.join("::"), member.member),
            Expr::WithMock(with_mock) => self.with_mock_text(with_mock, indent),
            Expr::TypeAscription(ascription) => {
                format!("({}:{})", self.expr(&ascription.expr, indent, 0), self.type_text(&ascription.ascribed_type))
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
            format!("λ({})=>{} {}", params, effects, self.type_text(&lambda.return_type))
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

    fn with_mock_text(&self, with_mock: &WithMockExpr, indent: usize) -> String {
        let mut out = format!(
            "withMock({},{}){{",
            self.expr(&with_mock.target, indent, 0),
            self.expr(&with_mock.replacement, indent, 0)
        );
        match &with_mock.body {
            Expr::Let(let_expr) => {
                let inner = self.let_text(let_expr, indent);
                out.push_str(&inner);
            }
            body => {
                out.push_str(&self.expr(body, indent, 0));
            }
        }
        out.push('}');
        out
    }

    fn pattern_text(&self, pattern: &Pattern) -> String {
        match pattern {
            Pattern::Literal(literal) => pattern_literal_text(literal),
            Pattern::Identifier(identifier) => identifier.name.clone(),
            Pattern::Wildcard(_) => "_".to_string(),
            Pattern::Constructor(constructor) => {
                let prefix = if constructor.module_path.is_empty() {
                    constructor.name.clone()
                } else {
                    format!("{}.{}", constructor.module_path.join("::"), constructor.name)
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
        Expr::Let(_) | Expr::Match(_) | Expr::If(_) | Expr::WithMock(_) | Expr::Lambda(_) => 1,
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
            BinaryOperator::Multiply | BinaryOperator::Divide | BinaryOperator::Modulo | BinaryOperator::Power => 7,
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
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    format!("\"{}\"", escaped)
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
