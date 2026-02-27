//! Recursive descent parser implementation
//!
//! This parser converts a stream of tokens into an Abstract Syntax Tree (AST).
//! It matches the TypeScript parser implementation exactly for compatibility.

use sigil_ast::*;
use sigil_lexer::{Position, SourceLocation, Token, TokenType};
use crate::error::ParseError;

/// The Sigil parser
pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
    filename: String,
}

impl Parser {
    /// Create a new parser from a token stream
    pub fn new(tokens: Vec<Token>, filename: impl Into<String>) -> Self {
        // Filter out newlines for parsing (but keep them in diagnostics)
        let tokens: Vec<Token> = tokens
            .into_iter()
            .filter(|t| t.token_type != TokenType::NEWLINE)
            .collect();

        Self {
            tokens,
            current: 0,
            filename: filename.into(),
        }
    }

    /// Parse the token stream into a Program AST
    pub fn parse(&mut self) -> Result<Program, ParseError> {
        let start = self.peek();
        let mut declarations = Vec::new();

        while !self.is_at_end() {
            declarations.push(self.declaration()?);
        }

        let end = self.previous();
        let location = self.make_location(start.location.start, end.location.end);

        Ok(Program::new(declarations, location))
    }

    // ========================================================================
    // DECLARATIONS
    // ========================================================================

    fn declaration(&mut self) -> Result<Declaration, ParseError> {
        let mut is_exported = false;
        if self.match_token(TokenType::EXPORT) {
            is_exported = true;
        }

        // Mockable function declaration: mockable λ...
        if self.match_token(TokenType::MOCKABLE) {
            if !self.check(TokenType::LAMBDA) {
                return Err(self.error("Expected \"λ\" after \"mockable\""));
            }
            let mockable_start = self.previous();
            self.consume(TokenType::LAMBDA, "Expected \"λ\" after \"mockable\"")?;
            return self.function_declaration(true, Some(mockable_start), is_exported);
        }

        // Function declaration: λ identifier(params)...
        if self.match_token(TokenType::LAMBDA) {
            return self.function_declaration(false, None, is_exported);
        }

        // Type declaration: t TypeName = ...
        if self.match_token(TokenType::TYPE) {
            return self.type_declaration(is_exported);
        }

        // Const declaration: c name = value
        if self.match_token(TokenType::CONST) {
            return self.const_declaration(is_exported);
        }

        // Import declaration: i module⋅path
        if self.match_token(TokenType::IMPORT) {
            if is_exported {
                return Err(self.error_at_current(
                    "Cannot export import declarations (canonical form: use \"i module⋅path\" only)",
                ));
            }
            return self.import_declaration();
        }

        // Extern declaration: e module⋅path
        if self.match_token(TokenType::EXTERN) {
            if is_exported {
                return Err(self.error_at_current(
                    "Cannot export extern declarations (canonical form: use \"e module⋅path\" only)",
                ));
            }
            return self.extern_declaration();
        }

        // Test declaration: test "description" { ... }
        if self.check_identifier("test") {
            if is_exported {
                return Err(self.error_at_current(
                    "Cannot export test declarations (tests are file-local)",
                ));
            }
            self.advance();
            return self.test_declaration();
        }

        if is_exported {
            return Err(self.error("Expected exportable declaration after \"export\" (λ, t, or c)"));
        }

        Err(self.error("Expected declaration (λ for function, t for type, etc.)"))
    }

    fn function_declaration(
        &mut self,
        is_mockable: bool,
        start_token: Option<Token>,
        is_exported: bool,
    ) -> Result<Declaration, ParseError> {
        let start = start_token.unwrap_or_else(|| self.previous());
        let name = self.consume(TokenType::IDENTIFIER, "Expected function name")?.value.clone();

        // Optional generic type parameters: λfunc[T,U](...)
        if self.match_token(TokenType::LBRACKET) {
            // Skip type parameters for now (they'll be in type inference)
            while !self.check(TokenType::RBRACKET) && !self.is_at_end() {
                self.advance();
            }
            self.consume(TokenType::RBRACKET, "Expected \"]\" after type parameters")?;
        }

        self.consume(TokenType::LPAREN, "Expected \"(\" after function name")?;
        let params = self.parameter_list()?;
        self.consume(TokenType::RPAREN, "Expected \")\" after parameters")?;

        // Return type annotation is MANDATORY (canonical form)
        self.consume(
            TokenType::ARROW,
            &format!(
                "Expected \"→\" after parameters for function \"{}\". Return type annotations are required (canonical form).",
                name
            ),
        )?;

        // Parse optional effect annotations: →!IO !Network Type
        let effects = self.parse_effects()?;

        let return_type = Some(self.parse_type()?);

        // Canonical form: = required UNLESS body starts with ≡ (match expression)
        let has_equal = self.match_token(TokenType::EQUAL);
        let is_match_expr = self.check(TokenType::MATCH);

        if is_match_expr && has_equal {
            return Err(self.error("Unexpected \"=\" before match expression (canonical form: λf()→T≡...)"));
        } else if !is_match_expr && !has_equal {
            return Err(self.error("Expected \"=\" before function body (canonical form: λf()→T=...)"));
        }

        let body = self.expression()?;

        let end = self.previous();
        let location = self.make_location(start.location.start, end.location.end);

        Ok(Declaration::Function(FunctionDecl {
            name,
            is_exported,
            is_mockable,
            params,
            effects,
            return_type,
            body,
            location,
        }))
    }

    fn parameter_list(&mut self) -> Result<Vec<Param>, ParseError> {
        if self.check(TokenType::RPAREN) {
            return Ok(Vec::new());
        }

        let mut params = Vec::new();
        loop {
            let start = self.peek();
            let name = self.consume(TokenType::IDENTIFIER, "Expected parameter name")?.value.clone();

            // Type annotation is MANDATORY (canonical form)
            self.consume(
                TokenType::COLON,
                &format!(
                    "Expected \":\" after parameter \"{}\". Type annotations are required (canonical form).",
                    name
                ),
            )?;

            // Check for mut modifier
            let is_mutable = self.match_token(TokenType::MUT);

            let type_annotation = Some(self.parse_type()?);

            let end = self.previous();
            let location = self.make_location(start.location.start, end.location.end);

            params.push(Param {
                name,
                type_annotation,
                is_mutable,
                location,
            });

            if !self.match_token(TokenType::COMMA) {
                break;
            }
        }

        Ok(params)
    }

    fn type_declaration(&mut self, is_exported: bool) -> Result<Declaration, ParseError> {
        let start = self.previous();
        let name = self.consume(TokenType::UPPER_IDENTIFIER, "Expected type name")?.value.clone();

        let mut type_params = Vec::new();
        if self.match_token(TokenType::LBRACKET) {
            loop {
                type_params.push(
                    self.consume(TokenType::UPPER_IDENTIFIER, "Expected type parameter")?.value.clone(),
                );
                if !self.match_token(TokenType::COMMA) {
                    break;
                }
            }
            self.consume(TokenType::RBRACKET, "Expected \"]\"")?;
        }

        self.consume(TokenType::EQUAL, "Expected \"=\"")?;
        let definition = self.type_definition()?;

        let end = self.previous();
        let location = self.make_location(start.location.start, end.location.end);

        Ok(Declaration::Type(TypeDecl {
            name,
            is_exported,
            type_params,
            definition,
            location,
        }))
    }

    fn type_definition(&mut self) -> Result<TypeDef, ParseError> {
        // Product type (record): { field: Type, ... }
        if self.check(TokenType::LBRACE) {
            return self.product_type().map(TypeDef::Product);
        }

        // Sum type or type alias
        let start = self.peek();
        let first_variant = self.variant_or_type()?;

        // If followed by |, it's a sum type
        if self.check(TokenType::PIPE_SEP) {
            return self.sum_type(first_variant).map(TypeDef::Sum);
        }

        // Otherwise, type alias
        let end = self.previous();
        Ok(TypeDef::Alias(TypeAlias {
            aliased_type: Type::Constructor(first_variant),
            location: self.make_location(start.location.start, end.location.end),
        }))
    }

    fn variant_or_type(&mut self) -> Result<TypeConstructor, ParseError> {
        let start = self.peek();
        let name = self.consume(TokenType::UPPER_IDENTIFIER, "Expected type or variant name")?.value.clone();

        let mut type_args = Vec::new();
        if self.match_token(TokenType::LPAREN) {
            if !self.check(TokenType::RPAREN) {
                loop {
                    type_args.push(self.parse_type()?);
                    if !self.match_token(TokenType::COMMA) {
                        break;
                    }
                }
            }
            self.consume(TokenType::RPAREN, "Expected \")\"")?;
        }

        let end = self.previous();
        Ok(TypeConstructor {
            name,
            type_args,
            location: self.make_location(start.location.start, end.location.end),
        })
    }

    fn sum_type(&mut self, first_variant: TypeConstructor) -> Result<SumType, ParseError> {
        let start_pos = first_variant.location.start;
        let mut variants = vec![Variant {
            name: first_variant.name,
            types: first_variant.type_args,
            location: first_variant.location,
        }];

        while self.match_token(TokenType::PIPE_SEP) {
            let var_start = self.peek();
            let variant = self.variant_or_type()?;
            let var_end = self.previous();
            variants.push(Variant {
                name: variant.name,
                types: variant.type_args,
                location: self.make_location(var_start.location.start, var_end.location.end),
            });
        }

        let end_pos = self.previous().location.end;
        Ok(SumType {
            variants,
            location: SourceLocation::new(start_pos, end_pos),
        })
    }

    fn product_type(&mut self) -> Result<ProductType, ParseError> {
        let start = self.peek();
        self.consume(TokenType::LBRACE, "Expected \"{\"")?;

        let mut fields = Vec::new();
        if !self.check(TokenType::RBRACE) {
            loop {
                let field_start = self.peek();
                let name = self.consume(TokenType::IDENTIFIER, "Expected field name")?.value.clone();
                self.consume(TokenType::COLON, "Expected \":\"")?;
                let field_type = self.parse_type()?;

                let field_end = self.previous();
                fields.push(Field {
                    name,
                    field_type,
                    location: self.make_location(field_start.location.start, field_end.location.end),
                });

                if !self.match_token(TokenType::COMMA) {
                    break;
                }
            }
        }

        self.consume(TokenType::RBRACE, "Expected \"}\"")?;

        let end = self.previous();
        Ok(ProductType {
            fields,
            location: self.make_location(start.location.start, end.location.end),
        })
    }

    fn const_declaration(&mut self, is_exported: bool) -> Result<Declaration, ParseError> {
        let start = self.previous();

        if self.check(TokenType::UPPER_IDENTIFIER) {
            let bad = self.peek();
            return Err(ParseError::InvalidConstantName {
                found: bad.value.clone(),
                line: bad.location.start.line,
                column: bad.location.start.column,
                location: bad.location,
            });
        }

        let name = self.consume(TokenType::IDENTIFIER, "Expected constant name")?.value.clone();

        // Type annotation is MANDATORY (canonical form)
        self.consume(
            TokenType::COLON,
            &format!(
                "Expected \":\" after constant \"{}\". Type annotations are required (canonical form).",
                name
            ),
        )?;
        let type_annotation = Some(self.parse_type()?);

        self.consume(TokenType::EQUAL, "Expected \"=\"")?;
        let value = self.expression()?;

        let end = self.previous();
        Ok(Declaration::Const(ConstDecl {
            name,
            is_exported,
            type_annotation,
            value,
            location: self.make_location(start.location.start, end.location.end),
        }))
    }

    fn import_declaration(&mut self) -> Result<Declaration, ParseError> {
        let start = self.previous();
        let mut module_path = Vec::new();

        // Parse module path: i stdlib⋅list
        loop {
            module_path.push(self.module_path_segment()?);
            if !self.match_token(TokenType::NAMESPACE_SEP) {
                break;
            }
        }

        if self.check(TokenType::SLASH) || self.check(TokenType::DOT) {
            let bad = self.peek();
            return Err(ParseError::InvalidNamespaceSeparator {
                found: bad.value.clone(),
                line: bad.location.start.line,
                column: bad.location.start.column,
                location: bad.location,
            });
        }

        let end = self.previous();
        Ok(Declaration::Import(ImportDecl {
            module_path,
            location: self.make_location(start.location.start, end.location.end),
        }))
    }

    fn extern_declaration(&mut self) -> Result<Declaration, ParseError> {
        let start = self.previous();
        let mut module_path = Vec::new();

        // Parse module path (e.g., fs⋅promises, axios, lodash)
        module_path.push(self.module_path_segment()?);

        // Handle namespace separators: fs⋅promises
        while self.match_token(TokenType::NAMESPACE_SEP) {
            module_path.push(self.module_path_segment()?);
        }

        if self.check(TokenType::SLASH) || self.check(TokenType::DOT) {
            let bad = self.peek();
            return Err(ParseError::InvalidNamespaceSeparator {
                found: bad.value.clone(),
                line: bad.location.start.line,
                column: bad.location.start.column,
                location: bad.location,
            });
        }

        // Optional type annotation: e console : { log : (𝕊) → 𝕌, ... }
        let members = if self.match_token(TokenType::COLON) {
            self.consume(TokenType::LBRACE, "Expected \"{\" after \":\" in typed extern declaration")?;
            let mut members_list = Vec::new();

            while !self.check(TokenType::RBRACE) && !self.is_at_end() {
                let member_start = self.peek();
                let member_name = self.consume(TokenType::IDENTIFIER, "Expected member name in extern type declaration")?.value.clone();
                self.consume(TokenType::COLON, "Expected \":\" after member name")?;
                let member_type = self.parse_type()?;

                let member_end = self.previous();
                members_list.push(ExternMember {
                    name: member_name,
                    member_type,
                    location: self.make_location(member_start.location.start, member_end.location.end),
                });

                // Allow comma as separator, break if we hit }
                if self.check(TokenType::RBRACE) {
                    break;
                }
                if !self.match_token(TokenType::COMMA) {
                    if !self.check(TokenType::RBRACE) {
                        return Err(self.error("Expected \",\" between extern members"));
                    }
                }
            }

            self.consume(TokenType::RBRACE, "Expected \"}\" after extern members")?;
            Some(members_list)
        } else {
            None
        };

        let end = self.previous();
        Ok(Declaration::Extern(ExternDecl {
            module_path,
            members,
            location: self.make_location(start.location.start, end.location.end),
        }))
    }

    fn module_path_segment(&mut self) -> Result<String, ParseError> {
        let mut parts = Vec::new();

        // Consume first part
        if self.match_token(TokenType::IDENTIFIER)
            || self.match_token(TokenType::UPPER_IDENTIFIER)
            || self.match_token(TokenType::INTEGER)
        {
            parts.push(self.previous().value.clone());
        } else {
            return Err(self.error("Expected module name"));
        }

        // Handle hyphenated names like "test-fixtures"
        while self.match_token(TokenType::MINUS) {
            parts.push("-".to_string());
            if self.match_token(TokenType::IDENTIFIER)
                || self.match_token(TokenType::UPPER_IDENTIFIER)
                || self.match_token(TokenType::INTEGER)
            {
                parts.push(self.previous().value.clone());
            } else {
                return Err(self.error("Expected module path segment after \"-\""));
            }
        }

        Ok(parts.join(""))
    }

    fn test_declaration(&mut self) -> Result<Declaration, ParseError> {
        let start = self.previous();
        let description = self.consume(TokenType::STRING, "Expected test description")?.value.clone();

        let effects = if self.match_token(TokenType::ARROW) {
            self.parse_effects()?
        } else {
            Vec::new()
        };

        self.consume(TokenType::LBRACE, "Expected \"{\"")?;
        let body = self.expression()?;
        self.consume(TokenType::RBRACE, "Expected \"}\"")?;

        let end = self.previous();
        Ok(Declaration::Test(TestDecl {
            description,
            effects,
            body,
            location: self.make_location(start.location.start, end.location.end),
        }))
    }

    // ========================================================================
    // TYPES
    // ========================================================================

    fn parse_effects(&mut self) -> Result<Vec<String>, ParseError> {
        let mut effects = Vec::new();
        let valid_effects = vec!["IO", "Network", "Async", "Error", "Mut"];

        while self.match_token(TokenType::BANG) {
            if self.match_token(TokenType::UPPER_IDENTIFIER) {
                let effect = self.previous().value.clone();

                if !valid_effects.contains(&effect.as_str()) {
                    let loc = self.previous().location;
                    return Err(ParseError::InvalidEffect {
                        effect,
                        valid: valid_effects.join(", "),
                        line: loc.start.line,
                        column: loc.start.column,
                        location: loc,
                    });
                }

                effects.push(effect);
            } else {
                return Err(self.error(&format!(
                    "Expected effect name ({}) after \"!\"",
                    valid_effects.join(", ")
                )));
            }
        }

        Ok(effects)
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        // Primitive types
        if self.match_token(TokenType::TYPE_INT) {
            let loc = self.previous().location;
            return Ok(Type::Primitive(PrimitiveType {
                name: PrimitiveName::Int,
                location: loc,
            }));
        }
        if self.match_token(TokenType::TYPE_FLOAT) {
            let loc = self.previous().location;
            return Ok(Type::Primitive(PrimitiveType {
                name: PrimitiveName::Float,
                location: loc,
            }));
        }
        if self.match_token(TokenType::TYPE_BOOL) {
            let loc = self.previous().location;
            return Ok(Type::Primitive(PrimitiveType {
                name: PrimitiveName::Bool,
                location: loc,
            }));
        }
        if self.match_token(TokenType::TYPE_STRING) {
            let loc = self.previous().location;
            return Ok(Type::Primitive(PrimitiveType {
                name: PrimitiveName::String,
                location: loc,
            }));
        }
        if self.match_token(TokenType::TYPE_CHAR) {
            let loc = self.previous().location;
            return Ok(Type::Primitive(PrimitiveType {
                name: PrimitiveName::Char,
                location: loc,
            }));
        }
        if self.match_token(TokenType::TYPE_UNIT) {
            let loc = self.previous().location;
            return Ok(Type::Primitive(PrimitiveType {
                name: PrimitiveName::Unit,
                location: loc,
            }));
        }

        // List type: [T]
        if self.match_token(TokenType::LBRACKET) {
            let start = self.previous();
            let element_type = self.parse_type()?;
            self.consume(TokenType::RBRACKET, "Expected \"]\"")?;
            let end = self.previous();
            return Ok(Type::List(Box::new(ListType {
                element_type,
                location: self.make_location(start.location.start, end.location.end),
            })));
        }

        // Map type: {K:V} or Function type: λ(T1,T2)→R
        if self.match_token(TokenType::LBRACE) {
            let start = self.previous();
            let key_type = self.parse_type()?;
            self.consume(TokenType::COLON, "Expected \":\" in map type")?;
            let value_type = self.parse_type()?;
            self.consume(TokenType::RBRACE, "Expected \"}\"")?;
            let end = self.previous();
            return Ok(Type::Map(Box::new(MapType {
                key_type,
                value_type,
                location: self.make_location(start.location.start, end.location.end),
            })));
        }

        // Function type: λ(T1, T2)→!IO !Network R
        if self.match_token(TokenType::LAMBDA) {
            let start = self.previous();
            self.consume(TokenType::LPAREN, "Expected \"(\"")?;
            let mut param_types = Vec::new();
            if !self.check(TokenType::RPAREN) {
                loop {
                    param_types.push(self.parse_type()?);
                    if !self.match_token(TokenType::COMMA) {
                        break;
                    }
                }
            }
            self.consume(TokenType::RPAREN, "Expected \")\"")?;
            self.consume(TokenType::ARROW, "Expected \"→\"")?;

            // Parse optional effect annotations in function types
            let effects = self.parse_effects()?;

            let return_type = self.parse_type()?;

            let end = self.previous();
            return Ok(Type::Function(Box::new(FunctionType {
                param_types,
                effects,
                return_type,
                location: self.make_location(start.location.start, end.location.end),
            })));
        }

        // Qualified type or type constructor/variable
        if self.match_token(TokenType::IDENTIFIER) || self.match_token(TokenType::UPPER_IDENTIFIER) {
            let start = self.previous();
            let first_segment = start.value.clone();
            let is_upper = start.token_type == TokenType::UPPER_IDENTIFIER;

            // Check for qualified type
            if self.check(TokenType::NAMESPACE_SEP) {
                let mut module_path = vec![first_segment];

                while self.match_token(TokenType::NAMESPACE_SEP) {
                    module_path.push(self.module_path_segment()?);
                }

                // Expect DOT then type name
                self.consume(
                    TokenType::DOT,
                    &format!(
                        "Expected \".\" after module path \"{}\". Qualified types use syntax: module⋅path.TypeName",
                        module_path.join("⋅")
                    ),
                )?;

                let type_name = self.consume(TokenType::UPPER_IDENTIFIER, "Expected type name after \".\"")?.value.clone();

                // Check for type arguments
                let mut type_args = Vec::new();
                if self.match_token(TokenType::LBRACKET) {
                    loop {
                        type_args.push(self.parse_type()?);
                        if !self.match_token(TokenType::COMMA) {
                            break;
                        }
                    }
                    self.consume(TokenType::RBRACKET, "Expected \"]\"")?;
                }

                let end = self.previous();
                return Ok(Type::Qualified(QualifiedType {
                    module_path,
                    type_name,
                    type_args,
                    location: self.make_location(start.location.start, end.location.end),
                }));
            }

            // Simple type constructor or variable
            let name = first_segment;

            // Check for type arguments
            if self.match_token(TokenType::LBRACKET) {
                let mut type_args = Vec::new();
                loop {
                    type_args.push(self.parse_type()?);
                    if !self.match_token(TokenType::COMMA) {
                        break;
                    }
                }
                self.consume(TokenType::RBRACKET, "Expected \"]\"")?;

                let end = self.previous();
                return Ok(Type::Constructor(TypeConstructor {
                    name,
                    type_args,
                    location: self.make_location(start.location.start, end.location.end),
                }));
            }

            // Type variable (uppercase without arguments)
            if is_upper {
                return Ok(Type::Variable(TypeVariable {
                    name,
                    location: start.location,
                }));
            }

            // Error: lowercase identifier without qualified path
            return Err(self.error("Expected type"));
        }

        Err(self.error("Expected type"))
    }

    // ========================================================================
    // EXPRESSIONS
    // ========================================================================

    fn expression(&mut self) -> Result<Expr, ParseError> {
        self.pipeline()
    }

    fn pipeline(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.list_operations()?;

        while self.match_token(TokenType::PIPE) {
            let right = self.list_operations()?;
            let start = expr.location().start;
            let end = self.previous().location.end;
            expr = Expr::Pipeline(Box::new(PipelineExpr {
                left: expr,
                operator: PipelineOperator::Pipe,
                right,
                location: SourceLocation::new(start, end),
            }));
        }

        Ok(expr)
    }

    fn list_operations(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.logical()?;

        // Built-in list operations (language constructs, not functions)
        loop {
            let start = expr.location().start;

            if self.match_token(TokenType::MAP) {
                // [1,2,3] ↦ λx→x*2
                let func = self.logical()?;
                let end = self.previous().location.end;
                expr = Expr::Map(Box::new(MapExpr {
                    list: expr,
                    func,
                    location: SourceLocation::new(start, end),
                }));
            } else if self.match_token(TokenType::FILTER) {
                // [1,2,3] ⊳ λx→x>1
                let predicate = self.logical()?;
                let end = self.previous().location.end;
                expr = Expr::Filter(Box::new(FilterExpr {
                    list: expr,
                    predicate,
                    location: SourceLocation::new(start, end),
                }));
            } else if self.match_token(TokenType::FOLD) {
                // [1,2,3] ⊕ λ(acc,x)→acc+x ⊕ 0
                let func = self.logical()?;
                self.consume(TokenType::FOLD, "Expected \"⊕\" before initial value")?;
                let init = self.logical()?;
                let end = self.previous().location.end;
                expr = Expr::Fold(Box::new(FoldExpr {
                    list: expr,
                    func,
                    init,
                    location: SourceLocation::new(start, end),
                }));
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn logical(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.comparison()?;

        while self.match_token(TokenType::AND) || self.match_token(TokenType::OR) {
            let op = if self.previous().token_type == TokenType::AND {
                BinaryOperator::And
            } else {
                BinaryOperator::Or
            };
            let right = self.comparison()?;
            let start = expr.location().start;
            let end = right.location().end;
            expr = Expr::Binary(Box::new(BinaryExpr {
                left: expr,
                operator: op,
                right,
                location: SourceLocation::new(start, end),
            }));
        }

        Ok(expr)
    }

    fn comparison(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.additive()?;

        while self.match_any(&[
            TokenType::EQUAL,
            TokenType::NOT_EQUAL,
            TokenType::LESS,
            TokenType::GREATER,
            TokenType::LESS_EQ,
            TokenType::GREATER_EQ,
        ]) {
            let op = match self.previous().token_type {
                TokenType::EQUAL => BinaryOperator::Equal,
                TokenType::NOT_EQUAL => BinaryOperator::NotEqual,
                TokenType::LESS => BinaryOperator::Less,
                TokenType::GREATER => BinaryOperator::Greater,
                TokenType::LESS_EQ => BinaryOperator::LessEq,
                TokenType::GREATER_EQ => BinaryOperator::GreaterEq,
                _ => unreachable!(),
            };
            let right = self.additive()?;
            let start = expr.location().start;
            let end = right.location().end;
            expr = Expr::Binary(Box::new(BinaryExpr {
                left: expr,
                operator: op,
                right,
                location: SourceLocation::new(start, end),
            }));
        }

        Ok(expr)
    }

    fn additive(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.multiplicative()?;

        while self.match_any(&[
            TokenType::PLUS,
            TokenType::MINUS,
            TokenType::APPEND,
            TokenType::LIST_APPEND,
        ]) {
            let op = match self.previous().token_type {
                TokenType::PLUS => BinaryOperator::Add,
                TokenType::MINUS => BinaryOperator::Subtract,
                TokenType::APPEND => BinaryOperator::Append,
                TokenType::LIST_APPEND => BinaryOperator::ListAppend,
                _ => unreachable!(),
            };
            let right = self.multiplicative()?;
            let start = expr.location().start;
            let end = right.location().end;
            expr = Expr::Binary(Box::new(BinaryExpr {
                left: expr,
                operator: op,
                right,
                location: SourceLocation::new(start, end),
            }));
        }

        Ok(expr)
    }

    fn multiplicative(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.unary()?;

        while self.match_any(&[
            TokenType::STAR,
            TokenType::SLASH,
            TokenType::PERCENT,
            TokenType::CARET,
        ]) {
            let op = match self.previous().token_type {
                TokenType::STAR => BinaryOperator::Multiply,
                TokenType::SLASH => BinaryOperator::Divide,
                TokenType::PERCENT => BinaryOperator::Modulo,
                TokenType::CARET => BinaryOperator::Power,
                _ => unreachable!(),
            };
            let right = self.unary()?;
            let start = expr.location().start;
            let end = right.location().end;
            expr = Expr::Binary(Box::new(BinaryExpr {
                left: expr,
                operator: op,
                right,
                location: SourceLocation::new(start, end),
            }));
        }

        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr, ParseError> {
        if self.match_any(&[TokenType::MINUS, TokenType::NOT, TokenType::HASH]) {
            let start = self.previous();
            let op = match start.token_type {
                TokenType::MINUS => UnaryOperator::Negate,
                TokenType::NOT => UnaryOperator::Not,
                TokenType::HASH => UnaryOperator::Length,
                _ => unreachable!(),
            };
            let operand = self.unary()?;
            let end = self.previous().location.end;
            return Ok(Expr::Unary(Box::new(UnaryExpr {
                operator: op,
                operand,
                location: self.make_location(start.location.start, end),
            })));
        }

        self.postfix()
    }

    fn postfix(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.primary()?;

        // Due to complexity, I'll implement a simplified postfix that handles
        // field access, index, and application
        loop {
            // Function application: f(args...)
            if self.check(TokenType::LPAREN) {
                self.advance();
                let mut args = Vec::new();
                if !self.check(TokenType::RPAREN) {
                    loop {
                        args.push(self.expression()?);
                        if !self.match_token(TokenType::COMMA) {
                            break;
                        }
                    }
                }
                self.consume(TokenType::RPAREN, "Expected \")\"")?;
                let end = self.previous().location.end;
                let start = expr.location().start;
                expr = Expr::Application(Box::new(ApplicationExpr {
                    func: expr,
                    args,
                    location: SourceLocation::new(start, end),
                }));
            }
            // Field access: record.field
            else if self.match_token(TokenType::DOT) {
                let field = self.consume(TokenType::IDENTIFIER, "Expected field name")?.value.clone();
                let end = self.previous().location.end;
                let start = expr.location().start;
                expr = Expr::FieldAccess(Box::new(FieldAccessExpr {
                    object: expr,
                    field,
                    location: SourceLocation::new(start, end),
                }));
            }
            // Index: list[index]
            else if self.match_token(TokenType::LBRACKET) {
                let index = self.expression()?;
                self.consume(TokenType::RBRACKET, "Expected \"]\"")?;
                let end = self.previous().location.end;
                let start = expr.location().start;
                expr = Expr::Index(Box::new(IndexExpr {
                    object: expr,
                    index,
                    location: SourceLocation::new(start, end),
                }));
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        // Literals
        if self.match_token(TokenType::INTEGER) {
            let tok = self.previous();
            let value = tok.value.parse::<i64>().map_err(|_| {
                self.error_at(tok.location, "Invalid integer literal")
            })?;
            return Ok(Expr::Literal(LiteralExpr {
                value: LiteralValue::Int(value),
                literal_type: LiteralType::Int,
                location: tok.location,
            }));
        }

        if self.match_token(TokenType::FLOAT) {
            let tok = self.previous();
            let value = tok.value.parse::<f64>().map_err(|_| {
                self.error_at(tok.location, "Invalid float literal")
            })?;
            return Ok(Expr::Literal(LiteralExpr {
                value: LiteralValue::Float(value),
                literal_type: LiteralType::Float,
                location: tok.location,
            }));
        }

        if self.match_token(TokenType::STRING) {
            let tok = self.previous();
            return Ok(Expr::Literal(LiteralExpr {
                value: LiteralValue::String(tok.value.clone()),
                literal_type: LiteralType::String,
                location: tok.location,
            }));
        }

        if self.match_token(TokenType::CHAR) {
            let tok = self.previous();
            let ch = tok.value.chars().next().ok_or_else(|| {
                self.error_at(tok.location, "Invalid character literal")
            })?;
            return Ok(Expr::Literal(LiteralExpr {
                value: LiteralValue::Char(ch),
                literal_type: LiteralType::Char,
                location: tok.location,
            }));
        }

        if self.match_token(TokenType::TRUE) {
            let tok = self.previous();
            return Ok(Expr::Literal(LiteralExpr {
                value: LiteralValue::Bool(true),
                literal_type: LiteralType::Bool,
                location: tok.location,
            }));
        }

        if self.match_token(TokenType::FALSE) {
            let tok = self.previous();
            return Ok(Expr::Literal(LiteralExpr {
                value: LiteralValue::Bool(false),
                literal_type: LiteralType::Bool,
                location: tok.location,
            }));
        }

        if self.match_token(TokenType::UNIT) {
            let tok = self.previous();
            return Ok(Expr::Literal(LiteralExpr {
                value: LiteralValue::Unit,
                literal_type: LiteralType::Unit,
                location: tok.location,
            }));
        }

        // Identifier
        if self.match_token(TokenType::IDENTIFIER) || self.match_token(TokenType::UPPER_IDENTIFIER) {
            let tok = self.previous();

            // Check for member access (FFI): module⋅path.member
            if self.check(TokenType::NAMESPACE_SEP) {
                let mut namespace = vec![tok.value.clone()];
                let start = tok.location.start;

                while self.match_token(TokenType::NAMESPACE_SEP) {
                    namespace.push(self.module_path_segment()?);
                }

                self.consume(TokenType::DOT, "Expected \".\" after namespace path")?;
                let member = self.consume(TokenType::IDENTIFIER, "Expected member name")?.value.clone();

                let end = self.previous().location.end;
                return Ok(Expr::MemberAccess(MemberAccessExpr {
                    namespace,
                    member,
                    location: SourceLocation::new(start, end),
                }));
            }

            return Ok(Expr::Identifier(IdentifierExpr {
                name: tok.value.clone(),
                location: tok.location,
            }));
        }

        // Lambda expression: λ(x:Int)→Int{ x+1 }
        if self.match_token(TokenType::LAMBDA) {
            return self.lambda_expression();
        }

        // Match expression: value ≡ pattern → body | ...
        if self.match_token(TokenType::MATCH) {
            return self.match_expression();
        }

        // Let expression: l x = 5 { body }
        if self.match_token(TokenType::LET) {
            return self.let_expression();
        }

        // List literal: [1, 2, 3]
        if self.match_token(TokenType::LBRACKET) {
            return self.list_expression();
        }

        // Record literal or tuple: {x:1, y:2} or (1, 2)
        if self.match_token(TokenType::LBRACE) {
            return self.record_expression();
        }

        if self.match_token(TokenType::LPAREN) {
            // Could be tuple or grouped expression
            if self.check(TokenType::RPAREN) {
                // Empty tuple? Or unit? In Sigil, () is unit literal
                self.advance();
                let end = self.previous().location.end;
                return Ok(Expr::Literal(LiteralExpr {
                    value: LiteralValue::Unit,
                    literal_type: LiteralType::Unit,
                    location: SourceLocation::new(self.previous().location.start, end),
                }));
            }

            let first = self.expression()?;

            if self.match_token(TokenType::COMMA) {
                // Tuple
                let mut elements = vec![first];
                loop {
                    elements.push(self.expression()?);
                    if !self.match_token(TokenType::COMMA) {
                        break;
                    }
                }
                self.consume(TokenType::RPAREN, "Expected \")\"")?;
                let end = self.previous().location.end;
                return Ok(Expr::Tuple(TupleExpr {
                    elements,
                    location: SourceLocation::new(self.previous().location.start, end),
                }));
            } else {
                // Grouped expression
                self.consume(TokenType::RPAREN, "Expected \")\"")?;
                return Ok(first);
            }
        }

        // with_mock expression
        if self.match_token(TokenType::WITH_MOCK) {
            return self.with_mock_expression();
        }

        Err(self.error("Expected expression"))
    }

    fn lambda_expression(&mut self) -> Result<Expr, ParseError> {
        let start = self.previous();
        self.consume(TokenType::LPAREN, "Expected \"(\"")?;
        let params = self.parameter_list()?;
        self.consume(TokenType::RPAREN, "Expected \")\"")?;
        self.consume(TokenType::ARROW, "Expected \"→\"")?;

        let effects = self.parse_effects()?;
        let return_type = self.parse_type()?;

        self.consume(TokenType::LBRACE, "Expected \"{\"")?;
        let body = self.expression()?;
        self.consume(TokenType::RBRACE, "Expected \"}\"")?;

        let end = self.previous();
        Ok(Expr::Lambda(Box::new(LambdaExpr {
            params,
            effects,
            return_type,
            body,
            location: self.make_location(start.location.start, end.location.end),
        })))
    }

    fn match_expression(&mut self) -> Result<Expr, ParseError> {
        // Match syntax: ≡scrutinee{pattern→body|pattern→body}
        let start = self.previous();
        let scrutinee = self.expression()?;
        self.consume(TokenType::LBRACE, "Expected \"{\"")?;

        let mut arms = Vec::new();
        loop {
            let arm_start = self.peek();
            let pattern = self.pattern()?;

            // Parse optional guard: when expr
            let guard = if self.match_token(TokenType::WHEN) {
                Some(self.expression()?)
            } else {
                None
            };

            self.consume(TokenType::ARROW, "Expected \"→\"")?;
            let body = self.expression()?;

            let arm_end = self.previous();
            arms.push(MatchArm {
                pattern,
                guard,
                body,
                location: self.make_location(arm_start.location.start, arm_end.location.end),
            });

            if !self.match_token(TokenType::PIPE_SEP) {
                break;
            }
        }

        self.consume(TokenType::RBRACE, "Expected \"}\"")?;

        let end = self.previous();
        Ok(Expr::Match(Box::new(MatchExpr {
            scrutinee,
            arms,
            location: self.make_location(start.location.start, end.location.end),
        })))
    }

    fn let_expression(&mut self) -> Result<Expr, ParseError> {
        let start = self.previous();
        let pattern = self.pattern()?;
        self.consume(TokenType::EQUAL, "Expected \"=\"")?;
        let value = self.expression()?;
        self.consume(TokenType::LBRACE, "Expected \"{\"")?;
        let body = self.expression()?;
        self.consume(TokenType::RBRACE, "Expected \"}\"")?;

        let end = self.previous();
        Ok(Expr::Let(Box::new(LetExpr {
            pattern,
            value,
            body,
            location: self.make_location(start.location.start, end.location.end),
        })))
    }

    fn list_expression(&mut self) -> Result<Expr, ParseError> {
        let start = self.previous();
        let mut elements = Vec::new();

        if !self.check(TokenType::RBRACKET) {
            loop {
                elements.push(self.expression()?);
                if !self.match_token(TokenType::COMMA) {
                    break;
                }
            }
        }

        self.consume(TokenType::RBRACKET, "Expected \"]\"")?;
        let end = self.previous();
        Ok(Expr::List(ListExpr {
            elements,
            location: self.make_location(start.location.start, end.location.end),
        }))
    }

    fn record_expression(&mut self) -> Result<Expr, ParseError> {
        let start = self.previous();
        let mut fields = Vec::new();

        if !self.check(TokenType::RBRACE) {
            loop {
                let field_start = self.peek();
                let name = self.consume(TokenType::IDENTIFIER, "Expected field name")?.value.clone();
                self.consume(TokenType::COLON, "Expected \":\"")?;
                let value = self.expression()?;

                let field_end = self.previous();
                fields.push(RecordField {
                    name,
                    value,
                    location: self.make_location(field_start.location.start, field_end.location.end),
                });

                if !self.match_token(TokenType::COMMA) {
                    break;
                }
            }
        }

        self.consume(TokenType::RBRACE, "Expected \"}\"")?;
        let end = self.previous();
        Ok(Expr::Record(RecordExpr {
            fields,
            location: self.make_location(start.location.start, end.location.end),
        }))
    }

    fn with_mock_expression(&mut self) -> Result<Expr, ParseError> {
        let start = self.previous();
        let target = self.primary()?;
        let replacement = self.primary()?;
        self.consume(TokenType::LBRACE, "Expected \"{\"")?;
        let body = self.expression()?;
        self.consume(TokenType::RBRACE, "Expected \"}\"")?;

        let end = self.previous();
        Ok(Expr::WithMock(Box::new(WithMockExpr {
            target,
            replacement,
            body,
            location: self.make_location(start.location.start, end.location.end),
        })))
    }

    // ========================================================================
    // PATTERNS
    // ========================================================================

    fn pattern(&mut self) -> Result<Pattern, ParseError> {
        // Literal pattern
        if self.match_token(TokenType::INTEGER)
            || self.match_token(TokenType::FLOAT)
            || self.match_token(TokenType::STRING)
            || self.match_token(TokenType::CHAR)
            || self.match_token(TokenType::TRUE)
            || self.match_token(TokenType::FALSE)
            || self.match_token(TokenType::UNIT)
        {
            let tok = self.previous();
            let (value, literal_type) = match tok.token_type {
                TokenType::INTEGER => {
                    let val = tok.value.parse::<i64>().map_err(|_| {
                        self.error_at(tok.location, "Invalid integer literal")
                    })?;
                    (PatternLiteralValue::Int(val), PatternLiteralType::Int)
                }
                TokenType::FLOAT => {
                    let val = tok.value.parse::<f64>().map_err(|_| {
                        self.error_at(tok.location, "Invalid float literal")
                    })?;
                    (PatternLiteralValue::Float(val), PatternLiteralType::Float)
                }
                TokenType::STRING => (
                    PatternLiteralValue::String(tok.value.clone()),
                    PatternLiteralType::String,
                ),
                TokenType::CHAR => {
                    let ch = tok.value.chars().next().ok_or_else(|| {
                        self.error_at(tok.location, "Invalid character literal")
                    })?;
                    (PatternLiteralValue::Char(ch), PatternLiteralType::Char)
                }
                TokenType::TRUE => (PatternLiteralValue::Bool(true), PatternLiteralType::Bool),
                TokenType::FALSE => (PatternLiteralValue::Bool(false), PatternLiteralType::Bool),
                TokenType::UNIT => (PatternLiteralValue::Unit, PatternLiteralType::Unit),
                _ => unreachable!(),
            };

            return Ok(Pattern::Literal(LiteralPattern {
                value,
                literal_type,
                location: tok.location,
            }));
        }

        // Wildcard pattern: _
        if self.match_token(TokenType::UNDERSCORE) {
            let tok = self.previous();
            return Ok(Pattern::Wildcard(WildcardPattern {
                location: tok.location,
            }));
        }

        // Constructor pattern or identifier: Some x, None, x
        if self.match_token(TokenType::UPPER_IDENTIFIER) {
            let start = self.previous();
            let name = start.value.clone();

            // Check for constructor with arguments: Some(x, y)
            if self.match_token(TokenType::LPAREN) {
                let mut patterns = Vec::new();
                if !self.check(TokenType::RPAREN) {
                    loop {
                        patterns.push(self.pattern()?);
                        if !self.match_token(TokenType::COMMA) {
                            break;
                        }
                    }
                }
                self.consume(TokenType::RPAREN, "Expected \")\"")?;
                let end = self.previous();
                return Ok(Pattern::Constructor(ConstructorPattern {
                    name,
                    patterns,
                    location: self.make_location(start.location.start, end.location.end),
                }));
            }

            // Constructor without arguments: None
            return Ok(Pattern::Constructor(ConstructorPattern {
                name,
                patterns: vec![],
                location: start.location,
            }));
        }

        // Identifier pattern: x
        if self.match_token(TokenType::IDENTIFIER) {
            let tok = self.previous();
            return Ok(Pattern::Identifier(IdentifierPattern {
                name: tok.value.clone(),
                location: tok.location,
            }));
        }

        // List pattern: [x, y, .rest]
        if self.match_token(TokenType::LBRACKET) {
            let start = self.previous();
            let mut patterns = Vec::new();
            let mut rest = None;

            if !self.check(TokenType::RBRACKET) {
                loop {
                    // Check for rest pattern: .xs
                    if self.match_token(TokenType::DOT) {
                        rest = Some(self.consume(TokenType::IDENTIFIER, "Expected identifier after \".\"")?.value.clone());
                        break;
                    }

                    patterns.push(self.pattern()?);
                    if !self.match_token(TokenType::COMMA) {
                        break;
                    }
                }
            }

            self.consume(TokenType::RBRACKET, "Expected \"]\"")?;
            let end = self.previous();
            return Ok(Pattern::List(ListPattern {
                patterns,
                rest,
                location: self.make_location(start.location.start, end.location.end),
            }));
        }

        // Record pattern: {x, y: value}
        if self.match_token(TokenType::LBRACE) {
            let start = self.previous();
            let mut fields = Vec::new();

            if !self.check(TokenType::RBRACE) {
                loop {
                    let field_start = self.peek();
                    let name = self.consume(TokenType::IDENTIFIER, "Expected field name")?.value.clone();

                    let pattern = if self.match_token(TokenType::COLON) {
                        Some(self.pattern()?)
                    } else {
                        None
                    };

                    let field_end = self.previous();
                    fields.push(RecordPatternField {
                        name,
                        pattern,
                        location: self.make_location(field_start.location.start, field_end.location.end),
                    });

                    if !self.match_token(TokenType::COMMA) {
                        break;
                    }
                }
            }

            self.consume(TokenType::RBRACE, "Expected \"}\"")?;
            let end = self.previous();
            return Ok(Pattern::Record(RecordPattern {
                fields,
                location: self.make_location(start.location.start, end.location.end),
            }));
        }

        // Tuple pattern: (x, y, z)
        if self.match_token(TokenType::LPAREN) {
            let start = self.previous();
            let mut patterns = Vec::new();

            if !self.check(TokenType::RPAREN) {
                loop {
                    patterns.push(self.pattern()?);
                    if !self.match_token(TokenType::COMMA) {
                        break;
                    }
                }
            }

            self.consume(TokenType::RPAREN, "Expected \")\"")?;
            let end = self.previous();
            return Ok(Pattern::Tuple(TuplePattern {
                patterns,
                location: self.make_location(start.location.start, end.location.end),
            }));
        }

        Err(self.error("Expected pattern"))
    }

    // ========================================================================
    // HELPER METHODS
    // ========================================================================

    fn match_token(&mut self, token_type: TokenType) -> bool {
        if self.check(token_type) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn match_any(&mut self, types: &[TokenType]) -> bool {
        for &token_type in types {
            if self.check(token_type) {
                self.advance();
                return true;
            }
        }
        false
    }

    fn check(&self, token_type: TokenType) -> bool {
        if self.is_at_end() {
            false
        } else {
            self.peek().token_type == token_type
        }
    }

    fn check_identifier(&self, value: &str) -> bool {
        if self.is_at_end() {
            false
        } else {
            let tok = self.peek();
            tok.token_type == TokenType::IDENTIFIER && tok.value == value
        }
    }

    fn advance(&mut self) -> Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.previous()
    }

    fn is_at_end(&self) -> bool {
        self.peek().token_type == TokenType::EOF
    }

    fn peek(&self) -> Token {
        self.tokens.get(self.current).cloned().unwrap_or_else(|| {
            // Return EOF token if we're past the end
            Token::new(
                TokenType::EOF,
                String::new(),
                SourceLocation::single(Position::new(0, 0, 0)),
            )
        })
    }

    fn previous(&self) -> Token {
        self.tokens.get(self.current.saturating_sub(1)).cloned().unwrap_or_else(|| {
            Token::new(
                TokenType::EOF,
                String::new(),
                SourceLocation::single(Position::new(0, 0, 0)),
            )
        })
    }

    fn consume(&mut self, token_type: TokenType, message: &str) -> Result<Token, ParseError> {
        if self.check(token_type) {
            Ok(self.advance())
        } else {
            Err(self.error(message))
        }
    }

    fn error(&self, message: &str) -> ParseError {
        let tok = self.peek();
        ParseError::Generic {
            message: message.to_string(),
            line: tok.location.start.line,
            column: tok.location.start.column,
            location: tok.location,
        }
    }

    fn error_at(&self, location: SourceLocation, message: &str) -> ParseError {
        ParseError::Generic {
            message: message.to_string(),
            line: location.start.line,
            column: location.start.column,
            location,
        }
    }

    fn error_at_current(&self, message: &str) -> ParseError {
        let tok = self.peek();
        ParseError::Generic {
            message: message.to_string(),
            line: tok.location.start.line,
            column: tok.location.start.column,
            location: tok.location,
        }
    }

    fn make_location(&self, start: Position, end: Position) -> SourceLocation {
        SourceLocation::new(start, end)
    }
}

// Helper trait to get location from any Expr
trait HasLocation {
    fn location(&self) -> SourceLocation;
}

impl HasLocation for Expr {
    fn location(&self) -> SourceLocation {
        match self {
            Expr::Literal(e) => e.location,
            Expr::Identifier(e) => e.location,
            Expr::Lambda(e) => e.location,
            Expr::Application(e) => e.location,
            Expr::Binary(e) => e.location,
            Expr::Unary(e) => e.location,
            Expr::Match(e) => e.location,
            Expr::Let(e) => e.location,
            Expr::If(e) => e.location,
            Expr::List(e) => e.location,
            Expr::Record(e) => e.location,
            Expr::Tuple(e) => e.location,
            Expr::FieldAccess(e) => e.location,
            Expr::Index(e) => e.location,
            Expr::Pipeline(e) => e.location,
            Expr::Map(e) => e.location,
            Expr::Filter(e) => e.location,
            Expr::Fold(e) => e.location,
            Expr::MemberAccess(e) => e.location,
            Expr::WithMock(e) => e.location,
        }
    }
}

/// Convenience function to parse source code
pub fn parse(tokens: Vec<Token>, filename: impl Into<String>) -> Result<Program, ParseError> {
    let mut parser = Parser::new(tokens, filename);
    parser.parse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sigil_lexer::tokenize;

    #[test]
    fn test_simple_function() {
        let source = "λ add(x: ℤ, y: ℤ) → ℤ = x + y";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        assert_eq!(program.declarations.len(), 1);
    }

    #[test]
    fn test_type_declaration() {
        let source = "t Maybe[T] = Some(T) | None";
        let tokens = tokenize(source).unwrap();
        let program = parse(tokens, "test.sigil").unwrap();
        assert_eq!(program.declarations.len(), 1);
    }
}
