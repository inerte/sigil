---
title: "Pattern Guards: How Dog-Fooding Evolved Sigil"
date: February 24, 2026
author: Sigil Language Team
slug: 003-pattern-guards-dog-fooding
---

# Pattern Guards: How Dog-Fooding Evolved Sigil

**TL;DR:** We tried to build Sigil's website in Sigil. We discovered the language needed pattern guards to write clean parsers. We added them, finished the website, and learned that dog-fooding works.

## The Plan

Sigil has a "batteries included" philosophy. We ship a comprehensive standard library with the compiler—no npm, no dependency hell, just one canonical way to do things.

To prove this works, we decided to build Sigil's own website using only Sigil and its stdlib:
- **`stdlib⋅markdown`** - Pure Sigil markdown parser
- **`stdlib⋅http_server`** - HTTP server wrapper
- **`stdlib⋅io`** - File operations
- **`stdlib⋅string`** - String utilities

Everything in stdlib, nothing external. If we couldn't build our own website in pure Sigil, the language wasn't ready.

## The Problem

We started implementing `stdlib⋅markdown` — a markdown-to-HTML converter written entirely in Sigil. No FFI to existing parsers, no shortcuts. Real dog-fooding.

Markdown parsing is fundamentally a state machine:
- Track whether you're inside a code block
- Track list nesting
- Track paragraph accumulation

Here's what the code looked like **without** pattern guards:

```sigil
⟦ Hypothetical pre-guards syntax - deeply nested matches ⟧
λparse_line(state:ParseState, line:𝕊)→(ParseState,[Block]) match state{
  {in_code=true,..} → match is_code_fence(line){
    true → close_code_block(state)|
    false → accumulate_code_line(state,line)
  }|
  {in_code=false,..} → match is_code_fence(line){
    true → start_code_block(state,line)|
    false → match is_header(line){
      true → parse_header(state,line)|
      false → match is_hr(line){
        true → parse_hr(state)|
        false → match is_empty(line){
          true → flush_paragraph(state)|
          false → accumulate_para(state,line)
        }
      }
    }
  }
}
```

See the problem? **Deeply nested match expressions.** We're matching on state structure, then nesting additional matches on boolean predicates. The logic is buried in seven levels of indentation. It works, but it's ugly and hard to read.

## The Gap

Sigil only had pattern matching with `match`:

```sigil
match value{
  pattern1 → result1|
  pattern2 → result2
}
```

This is great for structural matching (destructuring records, unpacking tuples, matching sum types). But what if you need to **check a condition on the bindings**?

You need to either:
1. Nest another `match` expression (ugly)
2. Use an `if` inside the body (breaks the flow)
3. Duplicate patterns for different conditions (verbose)

None of these are clean. And for a state machine like a markdown parser, it becomes unbearable.

## The Design

Other languages have solved this:
- **Haskell:** `case x of { n | n > 5 -> "big" }`
- **Rust:** `match x { n if n > 5 => "big" }`
- **OCaml:** `match x with | n when n > 5 -> "big"`

We chose syntax closest to our philosophy: **explicit and canonical**.

```sigil
match value{
  pattern when boolean_expr → result
}
```

The `when` keyword makes it crystal clear: "match this pattern, AND check this condition."

**Design decisions:**
1. **Guard after pattern:** Bindings are established before guard evaluation
2. **Must be boolean:** Type checker enforces `𝔹` type for guards
3. **Fall-through:** If guard is `false`, try the next arm
4. **Backward compatible:** Patterns without guards work exactly as before

## The Implementation

Adding pattern guards touched four layers:

### 1. Lexer
Added `WHEN` token recognition:

```typescript
case 'when': return TokenType.WHEN;
```

### 2. Parser
Extended `MatchArm` AST with optional guard:

```typescript
export interface MatchArm {
  pattern: Pattern;
  guard: Expr | null;  // NEW
  body: Expr;
  location: SourceLocation;
}
```

Parse `when` clause between pattern and `→`:

```typescript
const pattern = this.pattern();
let guard: Expr | null = null;
if (this.match(TokenType.WHEN)) {
  guard = this.expression();
}
this.consume(TokenType.ARROW, 'Expected "→"');
```

### 3. Type Checker
Verify guards are boolean, evaluated in **extended environment** (with pattern bindings):

```typescript
const bindings = checkPatternAndGetBindings(env, arm.pattern, scrutineeType);
const armEnv = env.extend(bindings);  // Bindings available here

if (arm.guard) {
  const guardType = synthesize(armEnv, arm.guard);
  const boolType: InferenceType = { kind: 'primitive', name: 'Bool' };
  if (!typesEqual(guardType, boolType)) {
    throw new TypeError(`Pattern guard must have type 𝔹, got ${formatType(guardType)}`);
  }
}
```

### 4. Code Generator
Generate nested `if` for guard check:

```typescript
if (bindings) {
  lines.push(`    ${bindings}`);  // Establish bindings
}

if (arm.guard) {
  const guardExpr = this.generateExpression(arm.guard);
  lines.push(`    if (await ${guardExpr}) {`);  // Check guard
  lines.push(`      return ${body};`);
  lines.push(`    }`);
} else {
  lines.push(`    return ${body};`);
}
```

Total implementation: **~50 lines of code**. Minimal, backward-compatible, type-safe.

## The Payoff

Now the markdown parser looks like this:

```sigil
⟦ With pattern guards - clean and linear ⟧
λparse_line(state:ParseState, line:𝕊)→(ParseState,[Block]) match state{
  {in_code=true,..} when is_code_fence(line) → close_code_block(state)|
  {in_code=true,..} → accumulate_code_line(state,line)|
  {in_code=false,..} when is_code_fence(line) → start_code_block(state,line)|
  {in_code=false,..} when is_header(line) → parse_header(state,line)|
  {in_code=false,..} when is_hr(line) → parse_hr(state)|
  {in_code=false,..} when is_empty(line) → flush_paragraph(state)|
  {in_code=false,..} → accumulate_para(state,line)
}
```

**One** level of matching. Clean, linear, readable. Match on state structure, guard on line content. Perfect.

The difference is dramatic: instead of nesting matches seven levels deep, we express each case as a flat pattern-plus-condition. The `when` keyword lets us combine structural matching (destructuring the state) with predicate checking (testing properties of the line) in a single, readable line.

## Beyond Markdown

Pattern guards aren't just for parsers. They're useful anywhere you need to:

**Validate data:**
```sigil
t User={name:𝕊,age:ℤ}

λvalidate(u:User)→𝕊 match u{
  {name,age} when age<0 → "invalid age"|
  {name,..} when #name=0 → "invalid name"|
  {name,age} → "valid"
}
```

**Range checking:**
```sigil
λclassify(n:ℤ)→𝕊 match n{
  x when x>100 → "large"|
  x when x>10 → "medium"|
  x when x>0 → "small"|
  _ → "non-positive"
}
```

**Conditional unpacking:**
```sigil
t Result=Ok(ℤ)|Err(𝕊)

λprocess(r:Result)→𝕊 match r{
  Ok(n) when n>100 → "big success"|
  Ok(n) when n>0 → "success"|
  Ok(_) → "zero or negative"|
  Err(msg) → "error: "++msg
}
```

## The Lesson

**Dog-fooding works.**

We didn't sit in a room theorizing about what features Sigil needed. We tried to build something real (the markdown parser), hit a wall (nested conditionals), and added exactly what was missing (pattern guards).

The result:
- ✅ **Minimal:** 50 lines of implementation
- ✅ **Backward compatible:** Existing code unaffected
- ✅ **Type safe:** Guards checked at compile time
- ✅ **Canonical:** One way to do conditional matching
- ✅ **Practical:** Solves real problems (state machines, validation, ranges)

And we finished `stdlib⋅markdown`, which we're using to build this website, which you're reading right now.

## Try It

Pattern guards are available in Sigil today:

```bash
brew install sigil
```

```sigil
λclassify(n:ℤ)→𝕊 match n{
  x when x>10 → "big"|
  x when x>0 → "small"|
  _ → "non-positive"
}
```

See `language/examples/pattern-guards.sigil` for more examples.

## What's Next?

Pattern guards suggest a broader pattern: **state machines as a language construct**.

Right now we write:
```sigil
match state{
  {mode:𝕊,..} when mode="active" → ...
}
```

What if we had:
```sigil
machine ParserState{
  Idle(input:𝕊) when #input>0 → Parsing |
  Parsing when complete → Done |
  Done → Idle
}
```

Maybe. We'll build more parsers, more state machines, more real code. If the pattern keeps appearing, we'll consider it.

That's how dog-fooding works: **build real things, evolve the language, repeat.**

---

*This article was written in markdown, parsed by `stdlib⋅markdown` (using pattern guards), and served by `stdlib⋅http_server`. Meta.*
