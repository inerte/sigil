/**
 * Mint Programming Language - Parser (Stub Implementation)
 *
 * This is a minimal parser stub that compiles successfully.
 * Full parser implementation coming soon.
 */

import { Token, TokenType } from '../lexer/token.js';
import * as AST from './ast.js';

export class Parser {
  private tokens: Token[];
  private current = 0;

  constructor(tokens: Token[]) {
    // Filter out newlines for now
    this.tokens = tokens.filter(t => t.type !== TokenType.NEWLINE);
  }

  parse(): AST.Program {
    const declarations: AST.Declaration[] = [];
    const start = this.peek();

    while (!this.isAtEnd()) {
      declarations.push(this.declaration());
    }

    return {
      type: 'Program',
      declarations,
      location: this.makeLocation(start, this.previous()),
    };
  }

  private declaration(): AST.Declaration {
    // Function declaration: λ identifier(params)...
    if (this.match(TokenType.LAMBDA)) {
      return this.functionDeclaration();
    }

    // Type declaration: t TypeName = ...
    if (this.match(TokenType.TYPE)) {
      return this.typeDeclaration();
    }

    // Const declaration: c name = value
    if (this.match(TokenType.CONST)) {
      return this.constDeclaration();
    }

    // Import declaration: i module/path
    if (this.match(TokenType.IMPORT)) {
      return this.importDeclaration();
    }

    // Test declaration: test "description" { ... }
    if (this.checkIdentifier('test')) {
      this.advance();
      return this.testDeclaration();
    }

    throw this.error('Expected declaration (λ for function, t for type, etc.)');
  }

  private functionDeclaration(): AST.FunctionDecl {
    const start = this.previous();
    const name = this.consume(TokenType.IDENTIFIER, 'Expected function name').value;

    // Optional generic type parameters: λfunc[T,U](...)
    if (this.match(TokenType.LBRACKET)) {
      // Skip type parameters for now (they'll be in type inference)
      while (!this.check(TokenType.RBRACKET) && !this.isAtEnd()) {
        this.advance();
      }
      this.consume(TokenType.RBRACKET, 'Expected "]" after type parameters');
    }

    this.consume(TokenType.LPAREN, 'Expected "(" after function name');
    const params = this.parameterList();
    this.consume(TokenType.RPAREN, 'Expected ")" after parameters');

    // Return type annotation is MANDATORY (canonical form)
    this.consume(TokenType.ARROW, `Expected "→" after parameters for function "${name}". Return type annotations are required (canonical form).`);
    const returnType = this.type();

    // Optional = before body (dense format can omit it)
    this.match(TokenType.EQUAL);

    const body = this.expression();

    return {
      type: 'FunctionDecl',
      name,
      params,
      returnType,
      body,
      location: this.makeLocation(start, this.previous()),
    };
  }

  private parameterList(): AST.Param[] {
    if (this.check(TokenType.RPAREN)) {
      return [];
    }

    const params: AST.Param[] = [];
    do {
      const start = this.peek();
      const name = this.consume(TokenType.IDENTIFIER, 'Expected parameter name').value;

      // Type annotation is MANDATORY (canonical form)
      this.consume(TokenType.COLON, `Expected ":" after parameter "${name}". Type annotations are required (canonical form).`);
      const typeAnnotation = this.type();

      params.push({
        name,
        typeAnnotation,
        location: this.makeLocation(start, this.previous()),
      });
    } while (this.match(TokenType.COMMA));

    return params;
  }

  private typeDeclaration(): AST.TypeDecl {
    const start = this.previous();
    const name = this.consume(TokenType.UPPER_IDENTIFIER, 'Expected type name').value;

    const typeParams: string[] = [];
    if (this.match(TokenType.LBRACKET)) {
      do {
        typeParams.push(this.consume(TokenType.UPPER_IDENTIFIER, 'Expected type parameter').value);
      } while (this.match(TokenType.COMMA));
      this.consume(TokenType.RBRACKET, 'Expected "]"');
    }

    this.consume(TokenType.EQUAL, 'Expected "="');
    const definition = this.typeDefinition();

    return {
      type: 'TypeDecl',
      name,
      typeParams,
      definition,
      location: this.makeLocation(start, this.previous()),
    };
  }

  private typeDefinition(): AST.TypeDef {
    // Product type (record): { field: Type, ... }
    if (this.check(TokenType.LBRACE)) {
      return this.productType();
    }

    // Sum type or type alias
    const start = this.peek();
    const firstVariant = this.variantOrType();

    // If followed by |, it's a sum type
    if (this.check(TokenType.PIPE_SEP)) {
      return this.sumType(firstVariant);
    }

    // Otherwise, type alias
    return {
      type: 'TypeAlias',
      aliasedType: firstVariant,
      location: this.makeLocation(start, this.previous()),
    };
  }

  private variantOrType(): AST.TypeConstructor {
    const start = this.peek();
    const name = this.consume(TokenType.UPPER_IDENTIFIER, 'Expected type or variant name').value;

    const typeArgs: AST.Type[] = [];
    if (this.match(TokenType.LPAREN)) {
      if (!this.check(TokenType.RPAREN)) {
        do {
          typeArgs.push(this.type());
        } while (this.match(TokenType.COMMA));
      }
      this.consume(TokenType.RPAREN, 'Expected ")"');
    }

    return {
      type: 'TypeConstructor',
      name,
      typeArgs,
      location: this.makeLocation(start, this.previous()),
    };
  }

  private sumType(firstVariant: AST.TypeConstructor): AST.SumType {
    const start = firstVariant.location.start;
    const variants: AST.Variant[] = [{
      name: firstVariant.name,
      types: firstVariant.typeArgs,
      location: firstVariant.location,
    }];

    while (this.match(TokenType.PIPE_SEP)) {
      const varStart = this.peek();
      const variant = this.variantOrType();
      variants.push({
        name: variant.name,
        types: variant.typeArgs,
        location: this.makeLocation(varStart, this.previous()),
      });
    }

    return {
      type: 'SumType',
      variants,
      location: { start, end: this.previous().end },
    };
  }

  private productType(): AST.ProductType {
    const start = this.peek();
    this.consume(TokenType.LBRACE, 'Expected "{"');

    const fields: AST.Field[] = [];
    if (!this.check(TokenType.RBRACE)) {
      do {
        const fieldStart = this.peek();
        const name = this.consume(TokenType.IDENTIFIER, 'Expected field name').value;
        this.consume(TokenType.COLON, 'Expected ":"');
        const fieldType = this.type();

        fields.push({
          name,
          fieldType,
          location: this.makeLocation(fieldStart, this.previous()),
        });
      } while (this.match(TokenType.COMMA));
    }

    this.consume(TokenType.RBRACE, 'Expected "}"');

    return {
      type: 'ProductType',
      fields,
      location: this.makeLocation(start, this.previous()),
    };
  }

  private constDeclaration(): AST.ConstDecl {
    const start = this.previous();
    const name = this.consume(TokenType.IDENTIFIER, 'Expected constant name').value;

    // Type annotation is MANDATORY (canonical form)
    this.consume(TokenType.COLON, `Expected ":" after constant "${name}". Type annotations are required (canonical form).`);
    const typeAnnotation = this.type();

    this.consume(TokenType.EQUAL, 'Expected "="');
    const value = this.expression();

    return {
      type: 'ConstDecl',
      name,
      typeAnnotation,
      value,
      location: this.makeLocation(start, this.previous()),
    };
  }

  private importDeclaration(): AST.ImportDecl {
    const start = this.previous();
    const modulePath: string[] = [];

    // Parse module path (e.g., std/io/file)
    do {
      modulePath.push(this.consume(TokenType.IDENTIFIER, 'Expected module name').value);
    } while (this.match(TokenType.SLASH));

    // Optional import list
    let imports: string[] | null = null;
    if (this.match(TokenType.LBRACE)) {
      imports = [];
      do {
        imports.push(this.consume(TokenType.IDENTIFIER, 'Expected import name').value);
      } while (this.match(TokenType.COMMA));
      this.consume(TokenType.RBRACE, 'Expected "}"');
    }

    return {
      type: 'ImportDecl',
      modulePath,
      imports,
      location: this.makeLocation(start, this.previous()),
    };
  }

  private testDeclaration(): AST.TestDecl {
    const start = this.previous();
    const description = this.consume(TokenType.STRING, 'Expected test description').value;
    this.consume(TokenType.LBRACE, 'Expected "{"');
    const body = this.expression();
    this.consume(TokenType.RBRACE, 'Expected "}"');

    return {
      type: 'TestDecl',
      description,
      body,
      location: this.makeLocation(start, this.previous()),
    };
  }

  // ============================================================================
  // TYPES
  // ============================================================================

  private type(): AST.Type {
    // Primitive types
    if (this.match(TokenType.TYPE_INT)) {
      return { type: 'PrimitiveType', name: 'Int', location: this.makeLocation(this.previous(), this.previous()) };
    }
    if (this.match(TokenType.TYPE_FLOAT)) {
      return { type: 'PrimitiveType', name: 'Float', location: this.makeLocation(this.previous(), this.previous()) };
    }
    if (this.match(TokenType.TYPE_BOOL)) {
      return { type: 'PrimitiveType', name: 'Bool', location: this.makeLocation(this.previous(), this.previous()) };
    }
    if (this.match(TokenType.TYPE_STRING)) {
      return { type: 'PrimitiveType', name: 'String', location: this.makeLocation(this.previous(), this.previous()) };
    }
    if (this.match(TokenType.TYPE_CHAR)) {
      return { type: 'PrimitiveType', name: 'Char', location: this.makeLocation(this.previous(), this.previous()) };
    }
    if (this.match(TokenType.TYPE_UNIT)) {
      return { type: 'PrimitiveType', name: 'Unit', location: this.makeLocation(this.previous(), this.previous()) };
    }

    // List type: [T]
    if (this.match(TokenType.LBRACKET)) {
      const start = this.previous();
      const elementType = this.type();
      this.consume(TokenType.RBRACKET, 'Expected "]"');
      return {
        type: 'ListType',
        elementType,
        location: this.makeLocation(start, this.previous()),
      };
    }

    // Map type: {K:V} - check if next after LBRACE is a type token
    if (this.match(TokenType.LBRACE)) {
      const start = this.previous();
      const keyType = this.type();
      this.consume(TokenType.COLON, 'Expected ":" in map type');
      const valueType = this.type();
      this.consume(TokenType.RBRACE, 'Expected "}"');
      return {
        type: 'MapType',
        keyType,
        valueType,
        location: this.makeLocation(start, this.previous()),
      };
    }

    // Function type: λ(T1, T2)→R
    if (this.match(TokenType.LAMBDA)) {
      const start = this.previous();
      this.consume(TokenType.LPAREN, 'Expected "("');
      const paramTypes: AST.Type[] = [];
      if (!this.check(TokenType.RPAREN)) {
        do {
          paramTypes.push(this.type());
        } while (this.match(TokenType.COMMA));
      }
      this.consume(TokenType.RPAREN, 'Expected ")"');
      this.consume(TokenType.ARROW, 'Expected "→"');
      const returnType = this.type();

      return {
        type: 'FunctionType',
        paramTypes,
        returnType,
        location: this.makeLocation(start, this.previous()),
      };
    }

    // Type constructor or variable
    if (this.match(TokenType.UPPER_IDENTIFIER)) {
      const start = this.previous();
      const name = start.value;

      // Check for type arguments
      if (this.match(TokenType.LBRACKET)) {
        const typeArgs: AST.Type[] = [];
        do {
          typeArgs.push(this.type());
        } while (this.match(TokenType.COMMA));
        this.consume(TokenType.RBRACKET, 'Expected "]"');

        return {
          type: 'TypeConstructor',
          name,
          typeArgs,
          location: this.makeLocation(start, this.previous()),
        };
      }

      return {
        type: 'TypeVariable',
        name,
        location: this.makeLocation(start, start),
      };
    }

    throw this.error('Expected type');
  }

  // ============================================================================
  // EXPRESSIONS
  // ============================================================================

  private expression(): AST.Expr {
    return this.pipeline();
  }

  private pipeline(): AST.Expr {
    let expr = this.listOperations();

    while (this.match(TokenType.PIPE)) {
      const right = this.listOperations();
      expr = {
        type: 'PipelineExpr',
        left: expr,
        operator: '|>',
        right,
        location: this.makeLocation(expr.location.start, this.previous().end),
      };
    }

    return expr;
  }

  private listOperations(): AST.Expr {
    let expr = this.logical();

    // Built-in list operations (language constructs, not functions)
    // Parse left-to-right: [1,2,3] ⊳ λx→x>0 ↦ λx→x*2 ⊕ λ(a,x)→a+x ⊕ 0
    while (true) {
      const start = expr.location.start;

      if (this.match(TokenType.MAP)) {
        // [1,2,3] ↦ λx→x*2
        const fn = this.logical();
        expr = {
          type: 'MapExpr',
          list: expr,
          fn,
          location: this.makeLocation(start, this.previous().end),
        };
      } else if (this.match(TokenType.FILTER)) {
        // [1,2,3] ⊳ λx→x>1
        const predicate = this.logical();
        expr = {
          type: 'FilterExpr',
          list: expr,
          predicate,
          location: this.makeLocation(start, this.previous().end),
        };
      } else if (this.match(TokenType.FOLD)) {
        // [1,2,3] ⊕ λ(acc,x)→acc+x ⊕ 0
        // First operand is the folding function (parse at same level to avoid consuming next ⊕)
        const fn = this.logical();
        // Second ⊕ with the initial value
        this.consume(TokenType.FOLD, 'Expected "⊕" before initial value');
        const init = this.logical();
        expr = {
          type: 'FoldExpr',
          list: expr,
          fn,
          init,
          location: this.makeLocation(start, this.previous().end),
        };
      } else {
        break;
      }
    }

    return expr;
  }

  private logical(): AST.Expr {
    let expr = this.comparison();

    while (this.match(TokenType.AND, TokenType.OR)) {
      const op = this.previous().type === TokenType.AND ? '∧' : '∨';
      const right = this.comparison();
      expr = {
        type: 'BinaryExpr',
        left: expr,
        operator: op,
        right,
        location: this.makeLocation(expr.location.start, right.location.end),
      };
    }

    return expr;
  }

  private comparison(): AST.Expr {
    let expr = this.additive();

    while (this.match(TokenType.EQUAL, TokenType.NOT_EQUAL, TokenType.LESS, TokenType.GREATER, TokenType.LESS_EQ, TokenType.GREATER_EQ)) {
      const op = this.getComparisonOp(this.previous().type);
      const right = this.additive();
      expr = {
        type: 'BinaryExpr',
        left: expr,
        operator: op,
        right,
        location: this.makeLocation(expr.location.start, right.location.end),
      };
    }

    return expr;
  }

  private additive(): AST.Expr {
    let expr = this.multiplicative();

    while (this.match(TokenType.PLUS, TokenType.MINUS, TokenType.APPEND)) {
      const op = this.previous().type === TokenType.PLUS ? '+'
                : this.previous().type === TokenType.MINUS ? '-'
                : '++';
      const right = this.multiplicative();
      expr = {
        type: 'BinaryExpr',
        left: expr,
        operator: op,
        right,
        location: this.makeLocation(expr.location.start, right.location.end),
      };
    }

    return expr;
  }

  private multiplicative(): AST.Expr {
    let expr = this.unary();

    while (this.match(TokenType.STAR, TokenType.SLASH, TokenType.PERCENT, TokenType.CARET)) {
      const op = this.previous().type === TokenType.STAR ? '*'
                : this.previous().type === TokenType.SLASH ? '/'
                : this.previous().type === TokenType.PERCENT ? '%'
                : '^';
      const right = this.unary();
      expr = {
        type: 'BinaryExpr',
        left: expr,
        operator: op,
        right,
        location: this.makeLocation(expr.location.start, right.location.end),
      };
    }

    return expr;
  }

  private unary(): AST.Expr {
    if (this.match(TokenType.MINUS, TokenType.NOT)) {
      const start = this.previous();
      const op = start.type === TokenType.MINUS ? '-' : '¬';
      const operand = this.unary();
      return {
        type: 'UnaryExpr',
        operator: op,
        operand,
        location: this.makeLocation(start, this.previous()),
      };
    }

    return this.postfix();
  }

  private postfix(): AST.Expr {
    let expr = this.primary();

    while (true) {
      // Record construction: TypeName{field:value, ...}
      // This handles constructor syntax like Response{status:200, body:"OK"}
      // Only for UPPERCASE identifiers (type names)
      if (this.check(TokenType.LBRACE) &&
          expr.type === 'IdentifierExpr' &&
          expr.name[0] === expr.name[0].toUpperCase()) {
        this.advance(); // consume {
        const fields: AST.RecordField[] = [];

        if (!this.check(TokenType.RBRACE)) {
          do {
            const fieldStart = this.peek();
            const fieldName = this.consume(TokenType.IDENTIFIER, 'Expected field name').value;
            this.consume(TokenType.COLON, 'Expected ":"');
            const fieldValue = this.expression();
            fields.push({
              name: fieldName,
              value: fieldValue,
              location: this.makeLocation(fieldStart, this.previous()),
            });
          } while (this.match(TokenType.COMMA));
        }

        this.consume(TokenType.RBRACE, 'Expected "}"');

        // For now, just treat as a record expression
        // Type checker will verify it matches the type
        expr = {
          type: 'RecordExpr',
          fields,
          location: this.makeLocation(expr.location.start, this.previous()),
        };
      }
      // Function call
      else if (this.match(TokenType.LPAREN)) {
        const args: AST.Expr[] = [];
        if (!this.check(TokenType.RPAREN)) {
          do {
            args.push(this.expression());
          } while (this.match(TokenType.COMMA));
        }
        this.consume(TokenType.RPAREN, 'Expected ")"');
        expr = {
          type: 'ApplicationExpr',
          func: expr,
          args,
          location: this.makeLocation(expr.location.start, this.previous().end),
        };
      }
      // Field access
      else if (this.match(TokenType.DOT)) {
        const field = this.consume(TokenType.IDENTIFIER, 'Expected field name').value;
        expr = {
          type: 'FieldAccessExpr',
          object: expr,
          field,
          location: this.makeLocation(expr.location.start, this.previous().end),
        };
      }
      // Index access
      else if (this.match(TokenType.LBRACKET)) {
        const index = this.expression();
        this.consume(TokenType.RBRACKET, 'Expected "]"');
        expr = {
          type: 'IndexExpr',
          object: expr,
          index,
          location: this.makeLocation(expr.location.start, this.previous().end),
        };
      }
      else {
        break;
      }
    }

    return expr;
  }

  private primary(): AST.Expr {
    const start = this.peek();

    // Literals
    if (this.match(TokenType.INTEGER)) {
      return {
        type: 'LiteralExpr',
        value: parseInt(this.previous().value, 10),
        literalType: 'Int',
        location: this.makeLocation(this.previous(), this.previous()),
      };
    }
    if (this.match(TokenType.FLOAT)) {
      return {
        type: 'LiteralExpr',
        value: parseFloat(this.previous().value),
        literalType: 'Float',
        location: this.makeLocation(this.previous(), this.previous()),
      };
    }
    if (this.match(TokenType.STRING)) {
      return {
        type: 'LiteralExpr',
        value: this.previous().value,
        literalType: 'String',
        location: this.makeLocation(this.previous(), this.previous()),
      };
    }
    if (this.match(TokenType.CHAR)) {
      return {
        type: 'LiteralExpr',
        value: this.previous().value,
        literalType: 'Char',
        location: this.makeLocation(this.previous(), this.previous()),
      };
    }
    if (this.match(TokenType.TRUE)) {
      return {
        type: 'LiteralExpr',
        value: true,
        literalType: 'Bool',
        location: this.makeLocation(this.previous(), this.previous()),
      };
    }
    if (this.match(TokenType.FALSE)) {
      return {
        type: 'LiteralExpr',
        value: false,
        literalType: 'Bool',
        location: this.makeLocation(this.previous(), this.previous()),
      };
    }
    if (this.match(TokenType.UNIT)) {
      return {
        type: 'LiteralExpr',
        value: null,
        literalType: 'Unit',
        location: this.makeLocation(this.previous(), this.previous()),
      };
    }

    // Lambda expression: λx→expr  or  λ(x,y)→expr
    if (this.match(TokenType.LAMBDA)) {
      return this.lambdaExpr();
    }

    // Match expression: ≡expr{...}
    if (this.match(TokenType.MATCH)) {
      return this.matchExpr();
    }

    // Let binding: l x=value;body
    if (this.match(TokenType.LET)) {
      return this.letExpr();
    }

    // List literal: [1, 2, 3]
    if (this.match(TokenType.LBRACKET)) {
      return this.listExpr(start);
    }

    // Record literal or grouped expression: {x:1} or {expr}
    if (this.match(TokenType.LBRACE)) {
      return this.recordOrGrouped(start);
    }

    // Tuple or grouped: (1,2,3) or (expr)
    if (this.match(TokenType.LPAREN)) {
      return this.tupleOrGrouped(start);
    }

    // Identifier
    if (this.match(TokenType.IDENTIFIER, TokenType.UPPER_IDENTIFIER)) {
      return {
        type: 'IdentifierExpr',
        name: this.previous().value,
        location: this.makeLocation(this.previous(), this.previous()),
      };
    }

    throw this.error('Expected expression');
  }

  private lambdaExpr(): AST.LambdaExpr {
    const start = this.previous();
    const params: AST.Param[] = [];

    // Parse params: λ(x:T,y:U)→R  (type annotations MANDATORY)
    if (this.check(TokenType.LPAREN)) {
      this.advance();
      if (!this.check(TokenType.RPAREN)) {
        do {
          const pStart = this.peek();
          const name = this.consume(TokenType.IDENTIFIER, 'Expected parameter').value;
          // Type annotation is MANDATORY (canonical form)
          this.consume(TokenType.COLON, `Expected ":" after lambda parameter "${name}". Type annotations are required (canonical form).`);
          const typeAnnotation = this.type();
          params.push({
            name,
            typeAnnotation,
            location: this.makeLocation(pStart, this.previous()),
          });
        } while (this.match(TokenType.COMMA));
      }
      this.consume(TokenType.RPAREN, 'Expected ")"');
    } else if (this.check(TokenType.IDENTIFIER)) {
      const pStart = this.peek();
      const name = this.advance().value;
      // Single parameter lambda also requires type annotation
      this.consume(TokenType.COLON, `Expected ":" after lambda parameter "${name}". Type annotations are required (canonical form).`);
      const typeAnnotation = this.type();
      params.push({
        name,
        typeAnnotation,
        location: this.makeLocation(pStart, this.previous()),
      });
    }

    // Return type annotation is MANDATORY (canonical form)
    this.consume(TokenType.ARROW, 'Expected "→" after lambda parameters. Return type annotations are required (canonical form).');
    const returnType = this.type();

    // Consume "=" before body (optional in some formats)
    this.match(TokenType.EQUAL);

    const body = this.expression();

    return {
      type: 'LambdaExpr',
      params,
      returnType,
      body,
      location: this.makeLocation(start, this.previous()),
    };
  }

  private matchExpr(): AST.MatchExpr {
    const start = this.previous();
    const scrutinee = this.expression();
    this.consume(TokenType.LBRACE, 'Expected "{"');

    const arms: AST.MatchArm[] = [];
    do {
      const armStart = this.peek();
      const pattern = this.pattern();
      this.consume(TokenType.ARROW, 'Expected "→"');
      const body = this.expression();
      arms.push({
        pattern,
        body,
        location: this.makeLocation(armStart, this.previous()),
      });
    } while (this.match(TokenType.PIPE_SEP));

    this.consume(TokenType.RBRACE, 'Expected "}"');

    return {
      type: 'MatchExpr',
      scrutinee,
      arms,
      location: this.makeLocation(start, this.previous()),
    };
  }

  private letExpr(): AST.LetExpr {
    const start = this.previous();
    const pattern = this.pattern();
    this.consume(TokenType.EQUAL, 'Expected "="');
    const value = this.expression();
    this.consume(TokenType.SEMICOLON, 'Expected ";"');
    const body = this.expression();

    return {
      type: 'LetExpr',
      pattern,
      value,
      body,
      location: this.makeLocation(start, this.previous()),
    };
  }

  private listExpr(start: Token): AST.ListExpr {
    const elements: AST.Expr[] = [];
    if (!this.check(TokenType.RBRACKET)) {
      do {
        // Check for spread operator: .expr (like [x, .rest])
        if (this.match(TokenType.DOT)) {
          // Parse the identifier after the dot
          const name = this.consume(TokenType.IDENTIFIER, 'Expected identifier after "."').value;
          // Create identifier for the rest expression
          const restId: AST.IdentifierExpr = {
            type: 'IdentifierExpr',
            name,
            location: this.makeLocation(this.previous(), this.previous()),
          };
          // Now check for function call
          if (this.match(TokenType.LPAREN)) {
            const args: AST.Expr[] = [];
            if (!this.check(TokenType.RPAREN)) {
              do {
                args.push(this.expression());
              } while (this.match(TokenType.COMMA));
            }
            this.consume(TokenType.RPAREN, 'Expected ")"');
            elements.push({
              type: 'ApplicationExpr',
              func: restId,
              args,
              location: this.makeLocation(start, this.previous()),
            });
          } else {
            elements.push(restId);
          }
        } else {
          elements.push(this.expression());
        }
      } while (this.match(TokenType.COMMA));
    }
    this.consume(TokenType.RBRACKET, 'Expected "]"');

    return {
      type: 'ListExpr',
      elements,
      location: this.makeLocation(start, this.previous()),
    };
  }

  private recordOrGrouped(start: Token): AST.Expr {
    // Empty record: {}
    if (this.check(TokenType.RBRACE)) {
      this.advance();
      return {
        type: 'RecordExpr',
        fields: [],
        location: this.makeLocation(start, this.previous()),
      };
    }

    // Try to parse as record/map field (id:expr or "key":expr)
    if (this.check(TokenType.IDENTIFIER) || this.check(TokenType.STRING)) {
      const checkpoint = this.current;
      const nameToken = this.advance();
      const name = nameToken.value;

      if (this.match(TokenType.COLON)) {
        // It's a record or map literal
        const value = this.expression();
        const fields: AST.RecordField[] = [{
          name,
          value,
          location: this.makeLocation(this.tokens[checkpoint], this.previous()),
        }];

        while (this.match(TokenType.COMMA)) {
          const fieldStart = this.peek();
          const fieldNameToken = this.check(TokenType.STRING)
            ? this.advance()
            : this.consume(TokenType.IDENTIFIER, 'Expected field name');
          const fieldName = fieldNameToken.value;
          this.consume(TokenType.COLON, 'Expected ":"');
          const fieldValue = this.expression();
          fields.push({
            name: fieldName,
            value: fieldValue,
            location: this.makeLocation(fieldStart, this.previous()),
          });
        }

        this.consume(TokenType.RBRACE, 'Expected "}"');
        return {
          type: 'RecordExpr',
          fields,
          location: this.makeLocation(start, this.previous()),
        };
      } else {
        // Backtrack - it's grouped
        this.current = checkpoint;
      }
    }

    // Grouped expression
    const expr = this.expression();
    this.consume(TokenType.RBRACE, 'Expected "}"');
    return expr;
  }

  private tupleOrGrouped(start: Token): AST.Expr {
    // Unit: ()
    if (this.check(TokenType.RPAREN)) {
      this.advance();
      return {
        type: 'LiteralExpr',
        value: null,
        literalType: 'Unit',
        location: this.makeLocation(start, this.previous()),
      };
    }

    const first = this.expression();

    // Tuple: (expr, expr, ...)
    if (this.match(TokenType.COMMA)) {
      const elements = [first];
      do {
        elements.push(this.expression());
      } while (this.match(TokenType.COMMA));
      this.consume(TokenType.RPAREN, 'Expected ")"');
      return {
        type: 'TupleExpr',
        elements,
        location: this.makeLocation(start, this.previous()),
      };
    }

    // Grouped: (expr)
    this.consume(TokenType.RPAREN, 'Expected ")"');
    return first;
  }

  // ============================================================================
  // PATTERNS
  // ============================================================================

  private pattern(): AST.Pattern {
    // Wildcard: _
    if (this.match(TokenType.UNDERSCORE)) {
      return {
        type: 'WildcardPattern',
        location: this.makeLocation(this.previous(), this.previous()),
      };
    }

    // Literals
    if (this.check(TokenType.INTEGER) || this.check(TokenType.FLOAT) ||
        this.check(TokenType.STRING) || this.check(TokenType.CHAR) ||
        this.check(TokenType.TRUE) || this.check(TokenType.FALSE) ||
        this.check(TokenType.UNIT)) {
      return this.literalPattern();
    }

    // List pattern: [x, y] or [x, .xs]
    if (this.match(TokenType.LBRACKET)) {
      return this.listPattern();
    }

    // Tuple pattern: (x, y, z)
    if (this.match(TokenType.LPAREN)) {
      return this.tuplePattern();
    }

    // Constructor or identifier
    if (this.match(TokenType.UPPER_IDENTIFIER)) {
      const start = this.previous();
      const name = start.value;

      // Constructor with args: Some(x)
      if (this.match(TokenType.LPAREN)) {
        const patterns: AST.Pattern[] = [];
        if (!this.check(TokenType.RPAREN)) {
          do {
            patterns.push(this.pattern());
          } while (this.match(TokenType.COMMA));
        }
        this.consume(TokenType.RPAREN, 'Expected ")"');

        return {
          type: 'ConstructorPattern',
          name,
          patterns,
          location: this.makeLocation(start, this.previous()),
        };
      }

      // Nullary constructor: None
      return {
        type: 'ConstructorPattern',
        name,
        patterns: [],
        location: this.makeLocation(start, start),
      };
    }

    // Identifier pattern: x
    if (this.match(TokenType.IDENTIFIER)) {
      return {
        type: 'IdentifierPattern',
        name: this.previous().value,
        location: this.makeLocation(this.previous(), this.previous()),
      };
    }

    throw this.error('Expected pattern');
  }

  private literalPattern(): AST.LiteralPattern {
    if (this.match(TokenType.INTEGER)) {
      return {
        type: 'LiteralPattern',
        value: parseInt(this.previous().value, 10),
        literalType: 'Int',
        location: this.makeLocation(this.previous(), this.previous()),
      };
    }
    if (this.match(TokenType.FLOAT)) {
      return {
        type: 'LiteralPattern',
        value: parseFloat(this.previous().value),
        literalType: 'Float',
        location: this.makeLocation(this.previous(), this.previous()),
      };
    }
    if (this.match(TokenType.STRING)) {
      return {
        type: 'LiteralPattern',
        value: this.previous().value,
        literalType: 'String',
        location: this.makeLocation(this.previous(), this.previous()),
      };
    }
    if (this.match(TokenType.CHAR)) {
      return {
        type: 'LiteralPattern',
        value: this.previous().value,
        literalType: 'Char',
        location: this.makeLocation(this.previous(), this.previous()),
      };
    }
    if (this.match(TokenType.TRUE)) {
      return {
        type: 'LiteralPattern',
        value: true,
        literalType: 'Bool',
        location: this.makeLocation(this.previous(), this.previous()),
      };
    }
    if (this.match(TokenType.FALSE)) {
      return {
        type: 'LiteralPattern',
        value: false,
        literalType: 'Bool',
        location: this.makeLocation(this.previous(), this.previous()),
      };
    }
    if (this.match(TokenType.UNIT)) {
      return {
        type: 'LiteralPattern',
        value: null,
        literalType: 'Unit',
        location: this.makeLocation(this.previous(), this.previous()),
      };
    }

    throw this.error('Expected literal pattern');
  }

  private listPattern(): AST.ListPattern {
    const start = this.previous();
    const patterns: AST.Pattern[] = [];
    let rest: string | null = null;

    if (!this.check(TokenType.RBRACKET)) {
      do {
        // Check for rest pattern: .xs
        if (this.match(TokenType.DOT)) {
          rest = this.consume(TokenType.IDENTIFIER, 'Expected identifier after "."').value;
          break;
        }
        patterns.push(this.pattern());
      } while (this.match(TokenType.COMMA));
    }

    this.consume(TokenType.RBRACKET, 'Expected "]"');

    return {
      type: 'ListPattern',
      patterns,
      rest,
      location: this.makeLocation(start, this.previous()),
    };
  }

  private tuplePattern(): AST.TuplePattern {
    const start = this.previous();
    const patterns: AST.Pattern[] = [];

    // Empty tuple is Unit: ()
    if (this.check(TokenType.RPAREN)) {
      this.advance();
      // Return a literal pattern for Unit
      return {
        type: 'TuplePattern',
        patterns: [],
        location: this.makeLocation(start, this.previous()),
      };
    }

    // Parse patterns: (p1, p2, p3, ...)
    do {
      patterns.push(this.pattern());
    } while (this.match(TokenType.COMMA));

    this.consume(TokenType.RPAREN, 'Expected ")"');

    return {
      type: 'TuplePattern',
      patterns,
      location: this.makeLocation(start, this.previous()),
    };
  }

  // ============================================================================
  // UTILITY METHODS
  // ============================================================================

  private match(...types: TokenType[]): boolean {
    for (const type of types) {
      if (this.check(type)) {
        this.advance();
        return true;
      }
    }
    return false;
  }

  private check(type: TokenType): boolean {
    if (this.isAtEnd()) return false;
    return this.peek().type === type;
  }

  private checkIdentifier(value: string): boolean {
    if (this.isAtEnd()) return false;
    const token = this.peek();
    return token.type === TokenType.IDENTIFIER && token.value === value;
  }

  private advance(): Token {
    if (!this.isAtEnd()) this.current++;
    return this.previous();
  }

  private isAtEnd(): boolean {
    return this.peek().type === TokenType.EOF;
  }

  private peek(): Token {
    return this.tokens[this.current];
  }

  private previous(): Token {
    return this.tokens[this.current - 1];
  }

  private consume(type: TokenType, message: string): Token {
    if (this.check(type)) return this.advance();
    throw this.error(message);
  }

  private error(message: string): Error {
    const token = this.peek();
    return new Error(
      `Parse error at line ${token.start.line}, column ${token.start.column}: ${message}\n` +
      `Got: ${token.type}${token.value ? ` (${token.value})` : ''}`
    );
  }

  private makeLocation(start: Token | { line: number; column: number; offset: number }, end: Token | { line: number; column: number; offset: number }): AST.SourceLocation {
    const startLoc = 'type' in start ? start.start : start;
    const endLoc = 'type' in end ? end.end : end;
    return {
      start: startLoc,
      end: endLoc,
    };
  }

  private getComparisonOp(type: TokenType): AST.BinaryOperator {
    switch (type) {
      case TokenType.EQUAL: return '=';
      case TokenType.NOT_EQUAL: return '≠';
      case TokenType.LESS: return '<';
      case TokenType.GREATER: return '>';
      case TokenType.LESS_EQ: return '≤';
      case TokenType.GREATER_EQ: return '≥';
      default: throw new Error(`Not a comparison operator: ${type}`);
    }
  }

}

export function parse(tokens: Token[]): AST.Program {
  const parser = new Parser(tokens);
  return parser.parse();
}
