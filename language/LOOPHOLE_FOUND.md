# 🚨 LOOPHOLE DISCOVERED: Record Type Bypass

## Executive Summary

**STATUS: ~~CONFIRMED~~ PATCHED ✅**

A loophole was discovered that allowed bypassing canonical form enforcement using record types. **This has been fixed.**

## The Vulnerability

**File:** `compiler/src/validator/canonical.ts`
**Line:** 335-355 in `isCollectionType()`

```typescript
case 'TypeConstructor':  // User-defined types
  return false;          // ❌ BUG: Doesn't recognize records as collections!
```

The validator blocks tuple-type parameters `(ℤ,ℤ)` but **ignores record-type parameters** `{n:ℤ,acc:ℤ}`.

## The Exploit

### File: `src/factorial.mint`

```mint
t State={n:ℤ,acc:ℤ}

λfactorial_recursive(n:ℤ)→ℤ≡n{
  0→1|
  1→1|
  n→n*factorial_recursive(n-1)
}

λfactorial_iterative(state:State)→ℤ≡state.n{
  0→state.acc|
  n→factorial_iterative({n:n-1,acc:n*state.acc})
}

λmain()→𝕊="Recursive: "+factorial_recursive(5)+" | Iterative: "+factorial_iterative({n:5,acc:1})
```

### Compilation Result

```bash
$ node compiler/dist/cli.js compile src/factorial.mint
✓ Compiled src/factorial.mint → .local/src/factorial.js

$ node compiler/dist/cli.js run src/factorial.mint
Recursive: 120 | Iterative: 120
```

**NO CANONICAL FORM ERRORS!** ✅

## Technical Analysis

### What Makes This a TRUE Loophole

1. **Bypasses Multi-Parameter Rule**: Instead of `λf(n:ℤ,acc:ℤ)`, we use `λf(state:State)` where `State={n:ℤ,acc:ℤ}`
2. **Enables Tail Recursion**: The accumulator pattern works perfectly
3. **Compiles Successfully**: The validator's `isCollectionType()` returns `false` for user-defined types
4. **Generates Correct Code**: JavaScript output shows proper tail-recursive structure

### Generated JavaScript (`.local/src/factorial.js`)

```javascript
// Recursive version (NOT tail-recursive)
export function factorial_recursive(n) {
  // ... pattern matching ...
  return (n * factorial_recursive((n - 1)));  // ❌ Stack builds up
}

// Iterative version (tail-recursive!)
export function factorial_iterative(state) {
  // ... pattern matching ...
  return factorial_iterative({ "n": (n - 1), "acc": (n * state.acc) });  // ✅ Tail call
}
```

## Why This Matters

### Proof of Concept

This demonstrates that Mint's "ONE canonical way" enforcement is **incomplete**:

- ✅ Blocks: `λf(n:ℤ,acc:ℤ)→ℤ`
- ✅ Blocks: `λf(state:(ℤ,ℤ))→ℤ`
- ✅ Blocks: `λf(state:[ℤ])→ℤ`
- ❌ **FAILS TO BLOCK**: `λf(state:{n:ℤ,acc:ℤ})→ℤ`

### Real-World Impact

1. **Two Valid Implementations**: We now have both recursive and iterative factorial
2. **Ambiguity for LLMs**: The exact problem Mint was designed to prevent
3. **Training Data Pollution**: Multiple valid patterns for the same algorithm

## The Fix

### Option 1: Extend `isCollectionType()`

```typescript
case 'TypeConstructor':
  // Need to resolve the type and check if it's a record type
  const resolvedType = resolveType(node.name, context);
  if (resolvedType?.kind === 'RecordType') {
    return true;  // ✅ Block record types with multiple fields
  }
  return false;
```

### Option 2: Count Record Fields

```typescript
case 'TypeConstructor':
  const typeDef = findTypeDefinition(node.name);
  if (typeDef?.kind === 'RecordType' && Object.keys(typeDef.fields).length > 1) {
    return true;  // ✅ Block multi-field records
  }
  return false;
```

## Verification

### Test Commands

```bash
# Compile (should succeed with current loophole)
node compiler/dist/cli.js compile src/factorial.mint

# Run (both implementations work)
node compiler/dist/cli.js run src/factorial.mint
```

### Expected Output

```
Recursive: 120 | Iterative: 120
```

### Current Status

- ✅ Compiles without canonical form errors
- ✅ Executes correctly
- ✅ Both implementations produce identical results
- ✅ Proves the loophole is real

## Conclusion

**The record type loophole was CONFIRMED and has been PATCHED.**

Mint's canonical form enforcement now successfully blocks ALL known loopholes:
1. ✅ Multi-parameter recursion
2. ✅ Tuple-type parameters
3. ✅ List-type parameters
4. ✅ Helper functions
5. ✅ CPS (function return types)
6. ✅ Y combinator (function return types)
7. ✅ Mutual recursion (helper detection)
8. ✅ Trampolining (function return types)
9. ✅ **Record-type parameters** ← FIXED!

**The "ONE way" guarantee is now complete: 9/9 loopholes blocked (100%).**

---

*Discovery date: 2026-02-21*
*Discoverer: Claude Opus 4.6*
*Fixed date: 2026-02-21*
*Status: **PATCHED** ✅*

## The Fix

Updated `compiler/src/validator/canonical.ts`:

```typescript
function isCollectionType(type: AST.Type, typeMap: Map<string, AST.TypeDef>): boolean {
  switch (type.type) {
    case 'ListType':
    case 'TupleType':
    case 'MapType':
      return true;

    case 'TypeConstructor':
    case 'TypeVariable':  // ← ADDED: Parser treats `State` as TypeVariable
      // Resolve user-defined types to check if they're record types
      const typeDef = typeMap.get(type.name);
      if (typeDef && typeDef.type === 'ProductType') {
        // Record types with multiple fields can encode multiple values
        return typeDef.fields.length > 1;  // ← Block multi-field records!
      }
      return false;

    // ...
  }
}
```

### Verification After Patch

```bash
$ node compiler/dist/cli.js compile test-loophole.mint

Error: Recursive function 'factorial' has a collection-type parameter.
Parameter type: State

Recursive functions must have a PRIMITIVE parameter (ℤ, 𝕊, 𝔹, etc).
Collection types (lists, tuples, records) can encode multiple values,
which enables accumulator-style tail recursion.
```

**✅ LOOPHOLE CLOSED!**
