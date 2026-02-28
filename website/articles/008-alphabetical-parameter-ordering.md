# Why Sigil Enforces Alphabetical Parameter Order

**Published:** 2026-02-27
**Category:** Language Design
**Tags:** canonical-forms, parameters, determinism

## The Problem

Most programming languages let you order function parameters however you want:

```python
# Python - both are valid
def send_email(to, from, subject, body):
    ...

def send_email(subject, body, to, from):
    ...
```

This flexibility creates problems:

1. **Multiple valid representations** - Same function signature, different orderings
2. **Arbitrary choices** - Is `(to, from, subject, body)` better than `(subject, body, to, from)`?
3. **Training data pollution** - AI models see the same function written 24 different ways
4. **No clear "right" answer** - Debates about "natural" ordering

## The Sigil Solution: ONE WAY

Sigil enforces **alphabetical parameter ordering**. Every function signature has exactly one valid form.

```sigil
✅ VALID - alphabetical order:
λsend_email(body:𝕊, from:𝕊, subject:𝕊, to:𝕊)→𝕌=...

❌ REJECTED - non-alphabetical:
λsend_email(to:𝕊, from:𝕊, subject:𝕊, body:𝕊)→𝕌=...
```

The compiler catches this immediately:

```
Error: SIGIL-CANON-PARAM-ORDER
Parameter out of alphabetical order in function "send_email"

Found: from at position 2
After: to at position 1

Parameters must be alphabetically ordered.
Reorder parameters: body, from, subject, to

Sigil enforces ONE WAY: canonical parameter ordering.
```

## Why Alphabetical?

### 1. Deterministic

There's no debate. No judgment calls. Alphabetical ordering is:
- **Language-agnostic** - Works in any human language
- **Tool-friendly** - Trivial to validate and auto-fix
- **Predictable** - Sort by Unicode code point, done

### 2. Consistent with Everything Else

Sigil already uses alphabetical ordering for:
- Declaration categories (types, externs, imports, consts, functions, tests)
- Declarations within categories
- Record fields
- Effect annotations

Parameter ordering extends this philosophy to function signatures.

### 3. Eliminates Choice

In traditional languages, developers argue about parameter order:
- "Put the most important parameter first!"
- "Group related parameters together!"
- "Follow the order of the English sentence!"

In Sigil: **There is no choice.** Alphabetical is the only way.

### 4. Training Data Quality

When AI models learn from Sigil code, they see:
- **One canonical pattern** for every function signature
- **No syntactic variations** polluting the dataset
- **Deterministic generation** - models learn to sort parameters alphabetically

This is fundamental to Sigil's mission: be the first language designed for AI code generation from the ground up.

## What About "Natural" Ordering?

Developers often want to order parameters "naturally":

```javascript
// JavaScript - "natural" ordering
function createUser(name, email, age) { ... }
```

But "natural" is subjective:
- Is it `(name, email, age)` or `(email, name, age)`?
- What about `(age, name, email)`?
- Should we group by type? By importance? By frequency of use?

**Sigil eliminates this debate entirely.** There is no "natural" ordering - only alphabetical.

```sigil
✅ The ONE way in Sigil:
λcreate_user(age:ℤ, email:𝕊, name:𝕊)→User=...
```

## Effect Ordering Too

Sigil also enforces alphabetical ordering for effect annotations:

```sigil
✅ VALID - alphabetical order:
λfetch_data()→!Async !IO !Network 𝕊=...

❌ REJECTED - non-alphabetical:
λfetch_data()→!Network !IO !Async 𝕊=...
```

Standard effects in alphabetical order:
- `!Async`
- `!Error`
- `!IO`
- `!Mut`
- `!Network`

## Migration Impact

When we added this rule, we found:
- **6 stdlib files** needed updates
- **25 functions** reordered
- **~50 function calls** updated to match

Example from `stdlib⋅http-server`:

```sigil
# Before:
λjson(status:ℤ, body:𝕊)→Response=...

# After (alphabetical):
λjson(body:𝕊, status:ℤ)→Response=...
```

All calls were updated automatically:

```sigil
# Before:
stdlib⋅http-server.json(200, "{}")

# After:
stdlib⋅http-server.json("{}", 200)
```

## For AI Code Generation

When generating Sigil code, AI models should:

1. **Always sort parameters alphabetically** by name
2. **Always sort effects alphabetically**
3. **Never** try to guess "natural" or "logical" ordering

This is enforced at compile time, so non-canonical code will fail immediately.

## The Bigger Picture

Alphabetical parameter ordering is part of Sigil's broader philosophy:

**ONE WAY to write everything:**
- ONE way to format code (enforced by canonical validator)
- ONE way to order declarations (alphabetical by category, then by name)
- ONE way to implement recursion (no accumulator-passing)
- ONE way to pattern match (canonical patterns only)
- ONE way to order parameters (alphabetical)
- ONE way to order effects (alphabetical)

This creates:
- **Perfect training data** for AI models
- **Byte-for-byte reproducibility** - same semantics = same bytes
- **Zero ambiguity** - no judgment calls, no style debates
- **Deterministic generation** - AI generates exactly one correct form

## Conclusion

Sigil doesn't ask "What's the best parameter order?"

Sigil asks: **"What's the ONE parameter order?"**

The answer is alphabetical. Always.

No exceptions. No debates. No choice.

**This is Sigil: ONE WAY.**

---

**Learn more:**
- [Canonical Forms Documentation](../docs/CANONICAL_FORMS.md)
- [Language Design Philosophy](./001-why-canonical-forms.md)
- [Alphabetical Declaration Ordering](./005-declaration-ordering.md)

**Error codes:**
- `SIGIL-CANON-PARAM-ORDER` - Parameter out of alphabetical order
- `SIGIL-CANON-EFFECT-ORDER` - Effect out of alphabetical order
