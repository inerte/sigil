# Sigil Syntax Reference

This is a **canonical syntax reference** for Sigil.

It exists for:
- reviewing generated Sigil code
- building tools (compiler, LSP, editors)
- grounding AI prompts against the current language surface

It is not a style guide for multiple alternatives, because Sigil intentionally has one canonical form.

## Scope

This document covers the current syntax surface in this repo:
- declarations (`export`, `λ`, `t`, `c`, `i`, `e`, `test`)
- expressions and pattern matching
- built-in list operators (`↦`, `⊳`, `⊕`, `⧺`)
- effects, mocks, and test syntax
- comments

For formatting/canonical whitespace rules, see:
- `docs/CANONICAL_FORMS.md`
- `docs/CANONICAL_ENFORCEMENT.md`

## Source Files

Sigil source files use canonical naming:
- Extension: `.sigil` (executables) or `.lib.sigil` (libraries)
- Format: lowercase letters, numbers, hyphens only
- Example: `user-service.lib.sigil`, `01-hello.sigil`
- Files should end with a final newline
- Tests live in project `./tests`
- App/library code lives in project `./src`

**Filename rules:**
- Lowercase only (a-z)
- Numbers allowed (0-9)
- Hyphens for word separation (-)
- No underscores, spaces, or special characters
- Must end with `.sigil` or `.lib.sigil`

**Valid:** `user-service.lib.sigil`, `01-intro.sigil`
**Invalid:** `UserService.sigil` (uppercase), `user_service.lib.sigil` (underscore)

## Comments

Sigil uses one comment syntax only:

```sigil
⟦ This is a comment ⟧

λfactorial(n:ℤ)→ℤ match n{
  0→1|  ⟦ inline comment ⟧
  n→n*factorial(n-1)
}
```

- `#`, `//`, and `/* ... */` are not Sigil comments

## Declarations

Sigil has six declaration categories in **strict canonical order**:

**`t → e → i → c → λ → test`**

- `t` = types (must come first so externs can reference them)
- `e` = externs (FFI imports)
- `i` = imports (Sigil modules)
- `c` = consts
- `λ` = functions
- `test` = tests

Within each category:
- Non-exported declarations first (alphabetically by name)
- Exported declarations second (alphabetically by name)

See [CANONICAL_FORMS.md](./CANONICAL_FORMS.md) for enforcement rules.

Module scope is declaration-only:
- valid: `t`, `e`, `i`, `c`, `λ`, `mockable λ`, `test`
- invalid: `l`

`l` is a local expression form, not a top-level declaration. Use `c` for immutable module-level values, or move setup bindings inside `main()` or another function body.

## Function declarations

```sigil
λadd(x:ℤ,y:ℤ)→ℤ=x+y
```

Generic top-level functions use explicit type parameters on the declaration:

```sigil
λidentity[T](x:T)→T=x
λmap_option[T,U](fn:λ(T)→U,opt:Option[T])→Option[U]=...
```

Rules:
- function name is required
- parameter types are required
- return type is required
- generic type parameters are allowed on top-level `λ` declarations
- `=` is required for regular expression bodies
- `=` is omitted when body starts with match (`match ...`)
- generic lambdas are not supported
- call-site type arguments like `f[ℤ](x)` are not supported

Match-body form:

```sigil
λfactorial(n:ℤ)→ℤ match n{
  0→1|
  1→1|
  n→n*factorial(n-1)
}
```

## Effectful function declarations

Effects are declared between `→` and the return type:

```sigil
λfetchUser(id:ℤ)→!Network 𝕊=axios.get("https://api.example.com/users/"+id)
λmain()→!IO 𝕌=console.log("hello")
```

Valid built-in effects are currently `!Error`, `!IO`, `!Mut`, and `!Network`.

## Mockable function declarations (tests)

```sigil
mockable λfetchUser(id:ℤ)→!Network 𝕊="real"
```

- `mockable` is only valid on functions
- mockable functions must be effectful
- mock targets are used by `withMock(...) { ... }` in tests

## Exported declarations (explicit)

Only explicitly exported top-level declarations are visible to other Sigil modules.

Canonical export forms:

```sigil
export λdouble(x:ℤ)→ℤ=x*2
export t Todo={done:𝔹,id:ℤ,text:𝕊}
export c version:𝕊="0.1"
```

Notes:
- `export` applies to top-level `λ`, `t`, and `c`
- `export test`, `export i ...`, and `export e ...` are invalid

## Type declarations (`t`)

## Product type (record)

```sigil
t User={active:𝔹,id:ℤ,name:𝕊}
```

Record fields are canonically alphabetical in:
- product type declarations
- record literals
- typed record construction
- record patterns

Local bindings are canonically non-shadowing:
- function parameters
- lambda parameters
- `l` bindings
- pattern bindings

Once a local name is introduced, nested scopes must use a fresh name instead of rebinding it.

## Sum type (ADT)

```sigil
t Color=Red|Green|Blue
t Option[T]=Some(T)|None
t Result[T,E]=Ok(T)|Err(E)
```

`Option[T]`, `Result[T,E]`, `Some`, `None`, `Ok`, and `Err` come from the implicit `core⋅prelude`. No import is required to use them.

Constructor usage:

```sigil
Red()
Some(42)
Err("not found")
```

## Map types and literals

Maps are dynamic keyed collections. They use `↦`, not `:`.

```sigil
t Headers={𝕊↦𝕊}

λdefault_headers()→{𝕊↦𝕊}={"content-type"↦"text/plain"}
λempty_headers()→{𝕊↦𝕊}=({↦}:{𝕊↦𝕊})
```

Rules:
- record literals use `:`
- map literals use `↦`
- `{}` is an empty record literal
- `{↦}` is an empty map literal
- `{"foo":1}` is invalid

## Constants (`c`)

```sigil
c answer:ℤ=42
c greeting:𝕊="hello"
```

Current parser behavior:
- constant identifiers use regular lowercase identifier form (e.g. `c answer:ℤ=42`)
- uppercase constant names like `c ANSWER:ℤ=42` are rejected today

## Imports and externs

## Sigil imports (`i`)

Sigil-to-Sigil imports are namespace imports only.

```sigil
i core⋅map
i src⋅todoDomain
i stdlib⋅json
i stdlib⋅list
i stdlib⋅file
i stdlib⋅path
i stdlib⋅time
i stdlib⋅url
```

Use imported members with fully qualified namespace access:

```sigil
src⋅todoDomain.completedCount(todos)
#[1,2,3]
src⋅graphTypes.Ordering([1,2,3])
```

Imported sum-type constructors use the same fully qualified namespace style in both expressions and match patterns:

```sigil
λrender(result:src⋅graphTypes.TopologicalSortResult)→[ℤ] match result{
  src⋅graphTypes.Ordering(order)→order|
  src⋅graphTypes.CycleDetected()→[]
}
```

Canonical Sigil import roots:
- `core⋅...`
- `src⋅...`
- `stdlib⋅...`

Not supported:
- `i ./...`
- `i ../...`
- selective imports
- aliasing

## External module interop (`e`)

```sigil
e console
e fs⋅promises
e react-dom⋅client
```

Use with namespace member access:

```sigil
console.log("hello")
fs⋅promises.writeFile("x.txt","data")
react-dom⋅client.createRoot(root)
```

## Tests

Tests are first-class declarations and must live under `./tests`.

## Basic test

```sigil
test "adds numbers" {
  1+1=2
}
```

## Effectful test

```sigil
e console

test "logs" →!IO {
  console.log("x")=()
}
```

## Mocked test

```sigil
mockable λfetchUser(id:ℤ)→!Network 𝕊="real"

test "mocked fetch" →!Network {
  withMock(fetchUser,λ(id:ℤ)→!Network 𝕊="mocked"){
    fetchUser(1)="mocked"
  }
}
```

## Expressions

## Literals and primitives

Primitive types:
- `ℤ` integer
- `ℝ` float
- `𝔹` boolean
- `𝕊` string
- `𝕌` unit

Boolean values:
- `true`
- `false`

Examples:

```sigil
42
3.14
"hello"
true
false
()
```

## Variables and calls

```sigil
add(1,2)
factorial(n-1)
```

## Pattern matching (`match`)

```sigil
match value{
  pattern1→result1|
  pattern2→result2|
  _→defaultResult
}
```

Examples:

```sigil
λsign(n:ℤ)→𝕊 match n{
  0→"zero"|
  n→"non-zero"
}

λdescribeBoth(a:𝔹,b:𝔹)→𝕊 match (a,b){
  (true,true)→"both"|
  (true,false)→"left"|
  (false,true)→"right"|
  (false,false)→"none"
}
```

## Pattern guards (`when`)

Pattern guards add conditional checks to pattern matching.
After a pattern binds variables, the guard expression is evaluated.
If the guard returns `false`, matching continues to the next arm.

Syntax:
```sigil
match value{
  pattern when guard_expr → result
}
```

The guard expression:
- Is evaluated **after** pattern bindings are established
- Has access to all bindings from the pattern
- Must have type `𝔹` (boolean)
- If `false`, matching falls through to the next arm

Examples:

```sigil
⟦ Range checking ⟧
λclassify(n:ℤ)→𝕊 match n{
  x when x>100 → "large"|
  x when x>10 → "medium"|
  x when x>0 → "small"|
  _ → "non-positive"
}

⟦ Conditional unpacking ⟧
t Result=Ok(ℤ)|Err(𝕊)

λprocess(r:Result)→𝕊 match r{
  Ok(n) when n>0 → "positive success"|
  Ok(n) → "non-positive success"|
  Err(msg) when #msg>0 → "error: "++msg|
  Err(_) → "unknown error"
}

⟦ Complex conditions ⟧
t Point={x:ℤ,y:ℤ}

λquadrant(p:Point)→𝕊 match p{
  {x,y} when x=0 and y=0 → "origin"|
  {x,y} when x>0 and y>0 → "quadrant I"|
  {x,y} when x<0 and y>0 → "quadrant II"|
  _ → "other"
}
```

Pattern guards are **backward compatible**: patterns without guards work exactly as before.

See `language/examples/patternGuards.sigil` for more examples.

## Lists

List literals:

```sigil
[]
[1,2,3]
["a","b","c"]
[ [1,2],[3] ]
```

List literals preserve nesting exactly as written.
If `xs:[ℤ]`, then `[xs]` has type `[[ℤ]]`.
Use `⧺` when you want concatenation.

List patterns:

```sigil
match xs{
  []→0|
  [x,.rest]→1
}
```

Concatenation:

```sigil
"ab"++"cd"      ⟦ string concat only ⟧
[1,2]⧺[3,4]     ⟦ list concat only ⟧
```

## Records and field access

```sigil
User{active:true,id:1,name:"A"}
todo.done
todo.text
```

## Indexing

```sigil
xs[0]
```

## Operators

## Arithmetic

```sigil
a+b
a-b
a*b
a/b
a%b
```

## Comparison

```sigil
a=b
a≠b
a<b
a>b
a≤b
a≥b
```

## Logical

```sigil
a and b
a or b
¬a
```

## Built-in list operators (language constructs)

Map:

```sigil
[1,2,3]↦λ(x:ℤ)→ℤ=x*2
```

Filter:

```sigil
[1,2,3,4]⊳λ(x:ℤ)→𝔹=x%2=0
```

`↦` and `⊳` require pure callbacks. Use `⊕` for ordered reductions that depend on sequential accumulator flow.

Fold:

```sigil
[1,2,3]⊕λ(acc:ℤ,x:ℤ)→ℤ=acc+x⊕0
```

## Lambdas

Lambda parameters and return type annotations are required.

```sigil
λ(x:ℤ)→ℤ=x*2
λ(todo:Todo)→𝔹=¬todo.done
```

Effectful lambda:

```sigil
λ(msg:𝕊)→!IO 𝕌=console.log(msg)
```

## Canonical Formatting Reminders

- No trailing whitespace
- Max one blank line
- Final newline required
- No tabs
- `λf()→T=...` for regular bodies
- `λf()→T match ...` for match bodies (no `=`)

See `docs/CANONICAL_FORMS.md` for the full enforced rules.
