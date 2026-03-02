# Canonical Form Enforcement

Sigil enforces "ONE way to do things" at the **compiler level**, not just through documentation.

## The Problem

Traditional languages allow multiple ways to write the same algorithm. For example, factorial can be written:

1. **Simple recursion**: `factorial(n) = n * factorial(n-1)`
2. **Tail recursion with accumulator**: `factorial(n, acc) = factorial(n-1, n*acc)`
3. **Iterative with loop**: `for i in range...`

This creates ambiguity for LLMs, leading to inconsistent code generation.

## Sigil's Solution

**Make alternative patterns syntactically impossible.**

### Rule 1: Recursive Functions → NO ACCUMULATOR PARAMETERS

**Enforced by:** Compiler uses static analysis to classify each parameter's role in recursion.

Sigil allows multi-parameter recursion, but **parameters cannot be accumulators**.

#### Parameter Classification

The compiler analyzes ALL recursive calls and classifies each parameter:

**STRUCTURAL** ✅ (Allowed)
- Decreases during recursion: `n-1`, `n/2`, `xs` (from `[x,.xs]`)
- Modulo/remainder: `a%b`
- Pattern decomposition: list tail, record fields

**QUERY** ✅ (Allowed)
- Stays constant: `target` in binary search, `base` in power
- Swaps algorithmically: pegs in Hanoi, `a` and `b` in GCD

**ACCUMULATOR** ❌ (Forbidden)
- Multiplication: `n*acc` (builds up product)
- Addition: `acc+n` (builds up sum)
- List construction: `[x,.acc]` (builds up list)
- String concatenation: `acc++s` (builds up string)

#### Detection Algorithm

The compiler analyzes ALL recursive calls and checks how each parameter's argument changes:
1. If argument is identical to parameter → **QUERY**
2. If argument decreases parameter → **STRUCTURAL**
3. If argument multiplies/adds parameters together → **ACCUMULATOR** (BLOCK)
4. If argument transforms parameter purely → **STRUCTURAL/QUERY**

#### Examples

##### ✅ ALLOWED: GCD (both params structural)
```sigil
λgcd(a:ℤ,b:ℤ)→ℤ≡b{0→a|b→gcd(b,a%b)}
```
- `a` → `b` (swap, structural transformation)
- `b` → `a%b` (modulo, always decreases)
- **Result**: COMPILES ✅

##### ✅ ALLOWED: Power (query + structural)
```sigil
λpower(base:ℤ,exp:ℤ)→ℤ≡exp{0→1|exp→base*power(base,exp-1)}
```
- `base` → `base` (query, unchanged)
- `exp` → `exp-1` (structural, decreases)
- **Result**: COMPILES ✅

##### ✅ ALLOWED: Nth Element (parallel decomposition)
```sigil
λnth(list:[ℤ],n:ℤ)→ℤ≡(list,n){
  ([x,.xs],0)→x|
  ([x,.xs],n)→nth(xs,n-1)
}
```
- `list` → `xs` (structural, list tail)
- `n` → `n-1` (structural, decreases)
- **Result**: COMPILES ✅

##### ❌ BLOCKED: Factorial with Accumulator
```sigil
λfactorial(n:ℤ,acc:ℤ)→ℤ≡n{0→acc|n→factorial(n-1,n*acc)}
```
- `n` → `n-1` (structural, decreases)
- `acc` → `n*acc` (ACCUMULATOR, multiplies/grows)
- **Result**: COMPILE ERROR ❌

**Error message:**
```
Accumulator-passing style detected in function 'factorial'.

Parameter roles:
  - n: structural (decreases)
  - acc: ACCUMULATOR (grows)

The parameter(s) [acc] are accumulators (grow during recursion).
Sigil does NOT support tail-call optimization or accumulator-passing style.

Accumulator pattern (FORBIDDEN):
  λfactorial(n:ℤ,acc:ℤ)→ℤ≡n{0→acc|n→factorial(n-1,n*acc)}
  - Parameter 'acc' only grows (n*acc) → ACCUMULATOR

Legitimate multi-parameter (ALLOWED):
  λgcd(a:ℤ,b:ℤ)→ℤ≡b{0→a|b→gcd(b,a%b)}
  - Both 'a' and 'b' transform algorithmically → structural

Use simple recursion without accumulator parameters.
```

##### ❌ BLOCKED: List Reverse with Accumulator
```sigil
λreverse(lst:[ℤ],acc:[ℤ])→[ℤ]≡lst{[]→acc|[x,.xs]→reverse(xs,[x])}
```
- `lst` → `xs` (structural, list tail)
- `acc` → `[x]` (ACCUMULATOR, list grows)
- **Result**: COMPILE ERROR ❌

### Rule 2: Canonical Pattern Matching

**Enforced by:** Compiler requires most direct pattern matching form

**Why:** Syntactic variations create training data ambiguity

**BLOCKED:** Boolean pattern matching when value matching is possible

**Example:**

```sigil
❌ COMPILE ERROR - Boolean matching when value matching works:
λisZero(n:ℤ)→𝔹≡(n=0){
  true→true|
  false→false
}

✅ COMPILES - Direct value matching:
λisZero(n:ℤ)→𝔹≡n{
  0→true|
  _→false
}
```

**ALLOWED:** Boolean tuples for complex multi-condition logic

```sigil
✅ COMPILES - Complex conditions (no simpler form exists):
λclassify(x:ℤ,y:ℤ)→𝕊≡(x>0,y>0){
  (true,true)→"quadrant 1"|
  (true,false)→"quadrant 4"|
  (false,true)→"quadrant 2"|
  (false,false)→"quadrant 3"
}
```

## Implementation

**Location:** `compiler/crates/sigil-validator/src/canonical.rs`

**Pipeline:**
```
Source → Tokenize → Parse → Validate Canonical Form → Type Check → Codegen
                                       ↑
                               Enforces ONE way
```

**Validation runs:**
- After parsing (AST available)
- Before type checking (fail fast)
- In both `compile` and `run` commands

## Why This Matters

### Traditional Approach
- Document: "Please write code this way"
- LLM: *generates code in alternative style*
- Human: *manually fixes*
- Result: Inconsistent codebase

### Sigil Approach
- Compiler: **REJECTS** alternative patterns
- LLM: Gets compile error immediately
- LLM: Generates canonical form
- Result: **100% consistency**

## Benefits

1. **Zero Ambiguity**: LLMs cannot generate non-canonical code
2. **Immediate Feedback**: Compile errors guide to correct form
3. **Training Data**: All Sigil code in the wild is canonical
4. **No Choice Paralysis**: One way = no decisions needed
5. **Future-Proof**: Even new LLMs learn the ONE way

## Testing

Test files in `src/test-tailrec/`:
- `test12-valid-canonical.sigil` - ✅ Compiles successfully (canonical form)
- `test18-factorial-acc-blocked.sigil` - ❌ Rejects accumulator-passing style
- `test13-boolean-match-blocked.sigil` - ❌ Rejects non-canonical pattern matching

Try them:
```bash
# This works
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- run language/test-fixtures/test-tailrec/test12-valid-canonical.sigil

# These fail with helpful errors
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- compile language/test-fixtures/test-tailrec/test18-factorial-acc-blocked.sigil
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- compile language/test-fixtures/test-tailrec/test13-boolean-match-blocked.sigil
```

## Philosophy

**"You can't write it wrong if the language won't let you."**

This is the key to machine-first programming languages. Instead of relying on:
- Style guides (humans forget)
- Linters (can be disabled)
- Code review (subjective)

We make the language **fundamentally incapable** of expressing alternatives.

Like how JavaScript can't express goto, or how Rust can't express null pointer dereference, Sigil can't express multiple ways to solve the same problem.

## Future Extensions

Other patterns we could enforce:
- ❌ `if/else` → ✅ Only pattern matching
- ❌ Multiple loop constructs → ✅ Only `map/filter/reduce`
- ❌ Null checks → ✅ Only `Option` type
- ❌ Try/catch → ✅ Only `Result` type

Each restriction eliminates ambiguity and makes LLM code generation more reliable.
