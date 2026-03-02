---
title: The # Operator: Why Sigil Has ONE Way to Get Length
date: February 24, 2026
author: Sigil Language Team
slug: 001-canonical-length-operator
---

# The `#` Operator: Why Sigil Has ONE Way to Get Length

## The Problem: Syntactic Noise in Training Data

When designing Sigil, we faced a fundamental question: how should programmers get the length of a string or list?

Most languages offer multiple approaches:
```python
# Python - multiple ways
len("hello")      # Built-in function
len([1,2,3])      # Same function, different type

# JavaScript - property access
"hello".length    // 5
[1,2,3].length    // 3

# Some libraries add helpers
StringUtils.len("hello")
ListUtils.len([1,2,3])
```

For a **machine-first language** optimized for AI code generation, this multiplicity is a bug, not a feature.

## The Training Data Quality Perspective

Consider what happens when an LLM learns to write code that gets string/list length:

**Noisy training data (what we avoid):**
```sigil
⟦ BAD - Multiple syntactic forms for same concept ⟧
stdlib⋅string_utils.len("hello")
stdlib⋅list.len([1,2,3])
s.length  ⟦ if we allowed property access ⟧
```

The model sees three different syntactic patterns for the identical semantic concept of "get length". This:
- Wastes model capacity learning syntactic variations
- Creates uncertainty in code generation
- Produces inconsistent codebases
- Pollutes training datasets with noise

**Clean training data (what we enforce):**
```sigil
⟦ GOOD - Single canonical operator ⟧
#"hello"     ⟦ → 5 ⟧
#[1,2,3]     ⟦ → 3 ⟧
#""          ⟦ → 0 ⟧
```

Every single example in every codebase uses `#`. No variations. No alternatives. **Deterministic code synthesis.**

## Why `#` Instead of a Function?

We considered several approaches:

### Option 1: Type-Specific Functions ❌
```sigil
stdlib⋅string_utils.len(s)
stdlib⋅list.len(xs)
```

**Problems:**
- Two different namespaces for same concept
- Longer syntax (verbose for machines)
- Requires importing different modules
- Semantically identical but syntactically different

### Option 2: Generic `len()` Function ❌
```sigil
len("hello")
len([1,2,3])
```

**Problems:**
- Requires polymorphism (complex type system feature)
- OR requires runtime type dispatch (breaks compile-time guarantees)
- Still not as concise as an operator
- Doesn't leverage Sigil's bidirectional type checking

### Option 3: The `#` Operator ✅
```sigil
#"hello"
#[1,2,3]
```

**Advantages:**
1. **ONE canonical form** - Zero syntactic variation
2. **Leverages bidirectional type checking** - Type is known at compile time, no polymorphism needed
3. **Concise** - Machine-first language optimizes for brevity (`#s` vs `len(s)`)
4. **Training data quality** - Single way to express "get length"
5. **Follows operator philosophy** - Like `++` for concat, `⧺` for list append, `↦` for map

## This Is Not Polymorphism

Importantly, `#` is **not a polymorphic function**. It's a compile-time checked primitive operator.

The type checker validates the operator based on the known type:
```typescript
case '#': {
  // Length operator - works on strings and lists
  const operandType = synthesize(env, expr.operand);

  // Check if type is sizeable (String or List)
  const isSizeable =
    (operandType.kind === 'primitive' && operandType.name === 'String') ||
    operandType.kind === 'list';

  if (!isSizeable) {
    throw new TypeError(
      `Cannot apply # to type ${formatType(operandType)}\n` +
      `Expected: 𝕊 or [T]`,
      expr.location
    );
  }

  return { kind: 'primitive', name: 'Int' }; // Always returns ℤ
}
```

Since Sigil uses bidirectional type checking with **mandatory type annotations everywhere**, the type of the operand is always known at compile time. No runtime dispatch. No type classes. No polymorphism.

## Codegen: Trivial and Efficient

Because types are known at compile time, codegen is trivial:

```typescript
case '#': {
  const operand = this.generateExpression(expr.operand);
  return `(await ${operand}).length`;
}
```

Both JavaScript strings and arrays use `.length`, so the generated code is identical regardless of type. But the type checker has already guaranteed this is valid.

## String Operations: Compiler Intrinsics

Alongside the `#` operator, we added comprehensive string operations as **compiler intrinsics**:

```sigil
stdlib⋅string.to_upper("hello")              ⟦ → "HELLO" ⟧
stdlib⋅string.substring("hello world",6,11)  ⟦ → "world" ⟧
stdlib⋅string.starts_with("# Title","# ")  ⟦ → true ⟧
```

These are not implemented in Sigil - they're recognized by the compiler and emit optimized JavaScript:

```typescript
private tryGenerateIntrinsic(func: AST.MemberAccessExpr, args: AST.Expr[]): string | null {
  const module = func.namespace.join('/');
  const member = func.member;

  if (module === 'stdlib/string') {
    const generatedArgs = args.map(arg => this.generateExpression(arg));

    switch (member) {
      case 'to_upper':
        return `(await ${generatedArgs[0]}).toUpperCase()`;
      case 'substring':
        return `(await ${generatedArgs[0]}).substring(await ${generatedArgs[1]}, await ${generatedArgs[2]})`;
      // ...
    }
  }
}
```

Users write pure Sigil, get native JavaScript performance.

## No Redundant Helpers

Following the "ONE way to do things" philosophy, we deliberately avoid redundant predicates:

**We DON'T provide:**
```sigil
⟦ These are redundant - users can compose them ⟧
is_empty(s)         ⟦ Just use: #s = 0 ⟧
is_whitespace(s)    ⟦ Just use: stdlib⋅string.trim(s) = "" ⟧
contains(s, search) ⟦ Just use: stdlib⋅string.index_of(s, search) ≠ -1 ⟧
```

Each of these can be composed from existing primitives. Adding them would create multiple ways to express the same concept - exactly what we're trying to avoid.

## Impact on LLM Code Generation

When an LLM trained on Sigil code needs to get the length of something:

1. **No decision fatigue** - There is exactly one way
2. **No context needed** - Same syntax for strings and lists
3. **Unambiguous** - `#` always means length
4. **Concise** - Fewer tokens in prompt and completion

Compare prompt budgets:

```
Traditional: "get the length of the string using len() or .length or String.length() or..."
Sigil: "get the length: #s"
```

The Sigil version is **deterministic**. No uncertainty. No variations.

## The Bigger Picture: Canonical Forms

The `#` operator is one example of Sigil's broader philosophy: **canonical forms only**.

Every semantic concept has exactly ONE syntactic representation:
- **Length?** `#`
- **String concatenation?** `++`
- **List concatenation?** `⧺`
- **Map over list?** `↦`
- **Pattern matching?** `≡`

This isn't about human ergonomics - it's about **machine learning efficiency**. When 93% of code is AI-generated (2026 stats), we should optimize for the 93%, not the 7%.

## Implementation Status

As of February 2026, Sigil has:
- ✅ `#` operator for strings and lists
- ✅ Compiler intrinsics for `stdlib⋅string` (10 functions)
- ✅ Compiler intrinsics for `stdlib⋅string` (2 predicates)
- ✅ Full type checking and error messages
- ✅ Optimized JavaScript codegen

The old `stdlib⋅list.len` function has been removed. Use `#` instead.

## Try It Yourself

```sigil
e console

λmain()→!IO 𝕌={
  console.log("Length of 'hello': "++(#"hello"));
  console.log("Length of list: "++(#[1,2,3,4]))
}
```

```bash
$ sigilc run demo.sigil
Length of 'hello': 5
Length of list: 4
```

One operator. Zero ambiguity. Maximum clarity.

---

**Takeaway:** In a machine-first language, eliminating syntactic variation isn't just aesthetic - it's fundamental to training data quality and deterministic code generation. The `#` operator is a small example of a big principle: **canonical forms everywhere**.
