---
title: "Canonical Declaration Ordering: ONE Way to Organize Code"
date: 2026-02-24
author: Sigil Language Team
slug: 005-canonical-declaration-ordering
---

# Canonical Declaration Ordering: ONE Way to Organize Code

> **🚨 BREAKING CHANGE (Feb 25, 2026):** The canonical ordering has been updated to **`t → e → i → c → λ → test`** (types now come first). This enables typed FFI declarations to reference named types. See the [Typed FFI and Declaration Ordering](/articles/typed-ffi-and-declaration-ordering) article for details and migration guide. The content below describes the original `e → i → t` ordering for historical context.

**TL;DR:** We enforced strict canonical declaration ordering in Sigil. ~~Every file must follow the same order: `e → i → t → c → λ → test`.~~ **Update:** The order is now `t → e → i → c → λ → test` (types first). Alphabetically within each category. Non-exported before exported. Zero flexibility. Maximum determinism.

## The Problem: Organization Bikeshedding

When you write code in a traditional language, you face dozens of micro-decisions:

```typescript
// JavaScript - every developer has their own style
import { Database } from './db';
import { Config } from './config';

const PORT = 3000;
const MAX_RETRIES = 5;

interface User { name: string; }
interface Post { title: string; }

function createUser(name: string): User { ... }
function deleteUser(id: string): void { ... }

// Or maybe imports after types?
// Or maybe consts at the bottom?
// Or maybe group related functions together?
// Or maybe...
```

**Every developer has a different answer.** Some group by concern. Some alphabetize everything. Some put imports first, others put types first. Some put exports at the top, others at the bottom.

For human developers, this is annoying bikeshedding. For **AI code generation**, it's a training data catastrophe.

## The AI Code Generation Problem

When an LLM learns to write code from a corpus of files, it sees:

```
File 1: imports → types → consts → functions
File 2: types → imports → functions → consts
File 3: functions → types → imports
File 4: imports → functions → consts → types
```

Every file teaches the model a **different organizational pattern**. The result:

1. **Non-deterministic output** - Same prompt, different ordering each time
2. **Inconsistent codebases** - AI-generated files don't match existing style
3. **Noisy diffs** - Regenerating a file moves declarations around
4. **Wasted context** - Model capacity spent learning syntactic variations

When you ask Claude Code to add a function to a file, it shouldn't have to *decide* where to put it. There should be **ONE CORRECT ANSWER**.

## The Solution: Strict Canonical Ordering

Sigil enforces a single canonical ordering at **compile time**:

```
Category Order:
  e    → externs (FFI imports)
  i    → imports (Sigil modules)
  t    → types
  c    → consts
  λ    → functions
  test → tests

Within each category:
  1. Non-exported declarations (alphabetically)
  2. Exported declarations (alphabetically)
```

**The compiler rejects any other ordering.**

### Example: Canonical File

```sigil
⟦ 1. Externs first ⟧
e console

⟦ 2. Imports second ⟧
i stdlib⋅list
i stdlib⋅string

⟦ 3. Types third ⟧
t Color=Red|Green|Blue
t Point={x:ℤ,y:ℤ}
t User={name:𝕊,age:ℤ}

⟦ 4. Consts fourth ⟧
c MAX_RETRIES=5
c TIMEOUT=1000

⟦ 5. Non-exported functions (alphabetically) ⟧
λhelper(n:ℤ)→ℤ=n+1
λvalidate(s:𝕊)→𝔹=#s>0

⟦ 6. Exported functions (alphabetically) ⟧
export λcreateUser(name:𝕊)→User={name:name,age:0}
export λformatPoint(p:Point)→𝕊=stdlib⋅string.int_to_string(p.x)

⟦ 7. Tests last ⟧
test "creates user with default age"={
  l user=createUser("Alice");
  stdlib⋅assert.equals(user.age,0)
}
```

**This is the ONLY valid ordering.** Move anything out of order and the compiler rejects it with a clear error message.

## Why This Order?

The category order is **dependency-based and execution-based**:

1. **Externs (`e`)** - Must come first because other code uses them
2. **Imports (`i`)** - Must come before types/functions that use imported modules
3. **Types (`t`)** - Must come before functions that use those types
4. **Consts (`c`)** - Must come before functions that reference them
5. **Functions (`λ`)** - The main logic (order doesn't affect semantics)
6. **Tests (`test`)** - Come last because they test everything above

Within each category, alphabetical order provides:
- **Predictable placement** - Adding `formatUser` goes after `createUser`
- **Easy diffing** - New declarations appear in deterministic positions
- **No bike-shedding** - Zero subjective ordering decisions

Non-exported before exported ensures:
- **Internal helpers first** - Read the private implementation before the public API
- **Consistent pattern** - Same rule across all categories

## Declaration Order Doesn't Affect Semantics

This is crucial: **Sigil supports forward references**.

You can write:

```sigil
λfoo()→ℤ=bar()  ⟦ bar() is defined below - OK! ⟧
λbar()→ℤ=42
```

The typechecker uses **two-pass checking**:
1. First pass: Collect all declarations
2. Second pass: Type check bodies (forward references work)

This means:
- **Mutual recursion works** - Functions can call each other
- **Types can reference each other** - Recursive types, sum types, etc.
- **Order is purely stylistic** - No semantic impact

So enforcing canonical ordering costs **nothing** in expressiveness. It's pure canonicalization.

## What It Looks Like When You Get It Wrong

The compiler catches ordering violations with **actionable error messages**:

### Wrong Category Order

```sigil
i stdlib⋅list  ⟦ Import ⟧
e console            ⟦ ERROR: extern comes after import ⟧
```

**Error:**
```
Canonical Ordering Error: Wrong category position

Found: e (extern) at line 2
Expected: extern declarations must come before import declarations

Category order: e → i → t → c → λ → test
  e    = externs (FFI imports)
  i    = imports (Sigil modules)
  t    = types
  c    = consts
  λ    = functions
  test = tests

Move all extern declarations to appear before import declarations.

Sigil enforces ONE way: canonical declaration ordering.
```

### Wrong Alphabetical Order

```sigil
t User={name:𝕊,age:ℤ}
t Point={x:ℤ,y:ℤ}    ⟦ ERROR: Point comes before User alphabetically ⟧
```

**Error:**
```
Canonical Ordering Error: Wrong alphabetical order

Found: t Point at line 2
Expected: Must come before t User at line 1

Within each category, declarations must be alphabetically ordered.

Move 't Point' before 't User'.

Sigil enforces ONE way: canonical declaration ordering.
```

### Export Before Non-Export

```sigil
export λcreateUser(name:𝕊)→User={name:name,age:0}
λhelper(n:ℤ)→ℤ=n+1  ⟦ ERROR: non-exported after exported ⟧
```

**Error:**
```
Canonical Ordering Error: Exports must come after non-exports

Found: λ helper at line 2
Before: export λ createUser at line 1

Within each category:
  1. Non-exported declarations (alphabetically)
  2. Exported declarations (alphabetically)

Move all exported function declarations to come after non-exported ones.

Sigil enforces ONE way: canonical declaration ordering.
```

**Every error message tells you exactly how to fix it.**

## Before and After: Real Examples

### Before (Messy)

This would be valid in most languages:

```sigil
⟦ Random order - different in every file ⟧
export λcreateUser(name:𝕊)→User={name:name,age:0}

t User={name:𝕊,age:ℤ}

i stdlib⋅string

λhelper(n:ℤ)→ℤ=n+1

e console

c MAX_RETRIES=5

t Point={x:ℤ,y:ℤ}

export λformatPoint(p:Point)→𝕊=stdlib⋅string.int_to_string(p.x)

c TIMEOUT=1000
```

**Problems:**
- Types scattered (lines 3 and 11)
- Functions scattered (lines 1, 7, 13)
- Consts scattered (lines 9, 15)
- Exports mixed with non-exports
- No predictable structure

### After (Canonical)

The ONLY valid form:

```sigil
⟦ Canonical order - identical in every file ⟧
e console

i stdlib⋅string

t Point={x:ℤ,y:ℤ}
t User={name:𝕊,age:ℤ}

c MAX_RETRIES=5
c TIMEOUT=1000

λhelper(n:ℤ)→ℤ=n+1

export λcreateUser(name:𝕊)→User={name:name,age:0}
export λformatPoint(p:Point)→𝕊=stdlib⋅string.int_to_string(p.x)
```

**Benefits:**
- Types grouped and alphabetical (lines 5-6)
- Consts grouped and alphabetical (lines 8-9)
- Functions separated: non-exported (line 11), then exported (lines 13-14)
- Imports at top (lines 1-3)
- Zero ambiguity about where anything goes

## Implementation: Validator Enforces at Compile Time

The canonical ordering validator runs after parsing, before typechecking:

```typescript
export function validateCanonicalForm(program: AST.Program): void {
  validateRecursiveFunctions(program);
  validateCanonicalPatternMatching(program);
  validateDeclarationOrdering(program);  // NEW
}
```

**It checks three things:**

### 1. Category Boundaries

```typescript
function validateCategoryBoundaries(decls: AST.Declaration[]): void {
  const categoryOrder = ['ExternDecl', 'ImportDecl', 'TypeDecl',
                        'ConstDecl', 'FunctionDecl', 'TestDecl'];
  let lastCategoryIndex = -1;

  for (const decl of decls) {
    const currentIndex = categoryOrder.indexOf(decl.type);

    if (currentIndex < lastCategoryIndex) {
      throw new CanonicalError(
        `Found: ${categorySymbol} at line ${decl.location.start.line}\n` +
        `Expected: ${category} must come before ${lastCategory}\n` +
        `Category order: e → i → t → c → λ → test`
      );
    }

    lastCategoryIndex = Math.max(lastCategoryIndex, currentIndex);
  }
}
```

### 2. Alphabetical Order Within Category

```typescript
function validateWithinCategoryOrder(
  declarations: AST.Declaration[],
  categoryName: string
): void {
  // Separate non-exported and exported
  const nonExported = declarations.filter(d => !isExportedDeclaration(d));
  const exported = declarations.filter(d => isExportedDeclaration(d));

  // Check each group is alphabetical
  checkAlphabeticalOrder(nonExported, categoryName, false);
  checkAlphabeticalOrder(exported, categoryName, true);

  // Check non-exported come before exported
  // (implementation details omitted for brevity)
}
```

### 3. Export Ordering

Non-exported declarations must appear before exported ones in each category.

**Total implementation: ~200 lines of validation code.** Small cost for massive benefit.

## Impact on AI Code Generation

When Claude Code adds a function to a Sigil file, it:

1. **Identifies the category** - Is this a function, type, const, import?
2. **Finds the insertion point** - Alphabetically within that category
3. **Checks export status** - Exported or non-exported section?
4. **Inserts in the ONLY valid position**

**Zero decision fatigue. Zero variation.**

Example prompt:
```
User: "Add a function to delete a user"

Claude: Determines:
  - Category: function (λ)
  - Name: deleteUser
  - Export: yes (public API)
  - Position: After createUser (alphabetically), in exported section
```

**Same prompt, same file structure, every time.** Deterministic code generation.

## Comparison to Other Languages

### Python (PEP 8) ❌

```python
# "Convention" - not enforced
# Imports at top (suggested)
# Classes then functions (suggested)
# But: no enforcement, every team differs
```

**Problem:** Linters warn, but don't enforce. Style varies wildly.

### Go (gofmt) ✅

```go
// gofmt enforces import grouping
import (
  "stdlib"    // Stdlib first

  "external"  // External next

  "local"     // Local last
)
```

**Better:** Automatic formatting enforces consistency. But only for imports.

### Rust (rustfmt) ✅

```rust
// rustfmt orders imports, enforces some structure
// But: functions/types/consts not ordered
```

**Better:** More enforcement than most languages. But incomplete.

### Sigil ✅✅

```sigil
⟦ Compiler enforces complete ordering ⟧
⟦ Rejects non-canonical code ⟧
⟦ Zero configuration, zero flexibility ⟧
e → i → t → c → λ → test
```

**Best:** Complete enforcement at compile time. Not a linter suggestion—a language requirement.

## Rollout: Updating 62+ Files

We enforced canonical ordering across the entire Sigil codebase:

**Stats:**
- **62 files updated** - Every `.sigil` file in the repo
- **Zero semantic changes** - Only moved declarations
- **All tests pass** - Order doesn't affect behavior
- **Diffs are clean** - Each file shows clear reorganization

**The process:**
1. Implemented the validator
2. Ran it on all files
3. Fixed violations following error messages
4. Committed the changes
5. Now: All new code must be canonical

**Key insight:** Forward references make this possible. In languages that require declarations before use, canonical ordering would conflict with dependency order. Sigil's two-pass typechecker eliminates this constraint.

## Benefits: Quantifiable

### 1. Deterministic AI Code Generation

**Before:** Generate same function 10 times, get 10 different placements.

**After:** Generate same function 10 times, get identical placement.

### 2. Smaller Diffs

**Before:** AI regenerates a file, moves all declarations around.

**After:** AI adds new declaration in canonical position, rest unchanged.

### 3. Faster Code Review

**Before:** Reviewer must check if new function is "appropriately placed."

**After:** Compiler guarantees correct placement. Reviewer focuses on logic.

### 4. Cleaner Training Data

**Before:** Training corpus has 50 different organizational patterns.

**After:** Training corpus has ONE pattern. Model learns it instantly.

### 5. Zero Bike-Shedding

**Before:** Team debates whether to group by concern or alphabetize.

**After:** Compiler decides. Team moves on.

## What We Learned

### 1. Forward References Are Essential

Without two-pass typechecking, canonical ordering would conflict with dependency ordering. You'd need types before functions, but functions might need types that reference other functions, etc.

**Two-pass checking breaks the cycle:**
- Pass 1: Collect all names
- Pass 2: Type check bodies

Order becomes **purely stylistic**.

### 2. Error Messages Must Be Actionable

Users don't care about abstract rules. They want to know:
1. What's wrong?
2. Where is it wrong?
3. How do I fix it?

Our error messages provide all three:
```
Found: λ bar at line 5
Expected: Must come before λ foo at line 3
Move 'λ bar' before 'λ foo'.
```

### 3. Enforcement > Convention

Style guides are ignored. Linters are disabled. Formatters are skipped.

**The only way to ensure consistency is compiler enforcement.**

Sigil doesn't suggest. Sigil requires.

### 4. AI Benefits Are Exponential

One canonical form helps AI 10x:
- Training: Learns one pattern, not fifty
- Generation: Zero decision points
- Consistency: Every file identical structure
- Diffs: Minimal, predictable changes
- Review: Focus on logic, not style

## Try It Yourself

Create a file with messy ordering:

```sigil
⟦ out-of-order.sigil ⟧
λfoo()→ℤ=42
t MyType=ℤ
i stdlib⋅list
```

Compile it:

```bash
node language/compiler/dist/cli.js compile out-of-order.sigil
```

You'll get:

```
Canonical Ordering Error: Wrong category position

Found: t (type) at line 2
Expected: type declarations must come before function declarations

Category order: e → i → t → c → λ → test
  ...
```

**Fix it:**

```sigil
⟦ canonical.sigil ⟧
i stdlib⋅list

t MyType=ℤ

λfoo()→ℤ=42
```

Compiles successfully. Zero complaints.

## The Bigger Picture: Machine-First Languages

Canonical declaration ordering is one piece of Sigil's broader philosophy: **optimize for machine code generation**.

When 93% of code is AI-generated (2026 stats), languages should:

1. **Eliminate syntactic variation** - ONE way to write everything
2. **Enforce canonical forms** - Compiler rejects alternatives
3. **Produce deterministic output** - Same input → same code
4. **Minimize decision points** - Fewer choices for AI
5. **Optimize training data** - Clean, consistent corpus

Canonical ordering achieves all five:

- ✅ **No variation** - Only one valid ordering
- ✅ **Enforced** - Compiler rejects others
- ✅ **Deterministic** - Same declarations → same order
- ✅ **Zero decisions** - AI knows where everything goes
- ✅ **Clean corpus** - Every file identical structure

**This isn't just about organization. It's about training data quality.**

## Status and Next Steps

**Current status (February 2026):**
- ✅ Validator implemented (~200 lines)
- ✅ All 62+ files in compliance
- ✅ Error messages clear and actionable
- ✅ Tests passing
- ✅ Documentation updated

**Future work:**
- Auto-formatter to fix ordering (optional tool)
- LSP integration to highlight violations in real-time
- Git hook to reject non-canonical commits
- Metrics tracking ordering violations in external projects

**The rule is permanent:** `e → i → t → c → λ → test`. Alphabetically within categories. Non-exported before exported. Forever.

## Conclusion

Canonical declaration ordering isn't revolutionary. It's obvious.

**If you want deterministic AI code generation, you need deterministic code structure.**

Letting every developer (or AI) choose their own ordering creates:
- Training data noise
- Non-deterministic output
- Messy diffs
- Bike-shedding debates
- Review friction

Enforcing ONE way eliminates all of this.

**The cost:** Zero. (Forward references make order semantically irrelevant.)

**The benefit:** Massive. (Deterministic AI generation, clean training data, predictable code.)

In 2026, when AI writes most code, languages should optimize for the 93%, not the 7%.

Canonical declaration ordering is how Sigil does it.

---

**See it in action:** Every file in `language/` follows canonical ordering.

**Try it yourself:**
```bash
git clone https://github.com/sigil-lang/sigil.git
node language/compiler/dist/cli.js compile your-file.sigil
```

**Read the validator:** `language/compiler/src/validator/canonical.ts`

**ONE way. Zero exceptions. Maximum determinism.**
