---
title: Why Sigil for Coding Agents
---

# Why Sigil for Coding Agents

Most programming languages were designed for human authors. Sigil was designed
for the entity that is now writing most of the world's code.

LLMs predict tokens, not semantics. Ambiguity — multiple valid forms for the
same construct — compounds that uncertainty at every step.

Sigil's answer has two parts. The **canonical constraint hypothesis** reduces
the valid surface to one accepted form per construct — every valid program has
exactly one textual representation, enforced at compile time, so the model
never has to choose between equivalent spellings. The **tooling** targets the
same consumer: structured JSON diagnostics at every compiler phase, semantic
review that reports which function gained which effect rather than which lines
changed, and embedded docs the binary can serve directly.

The sections below explain what this looks like in practice.

---

## On This Page

- [Reading the Code](#reading-the-code) — Unicode characters and declaration keywords explained
- [One Representation Per AST](#one-representation-per-ast) — printer-first design
- [Match as the Only Branching Surface](#match-as-the-only-branching-surface) — no if/else
- [No Shadowing](#no-shadowing) — one binding per name, always
- [Canonical Names](#canonical-names) — two case rules, enforced everywhere
- [Alphabetical Ordering](#alphabetical-ordering) — parameters, fields, and effects
- [Explicit Effects](#explicit-effects) — effects declared, not inferred or hidden
- [World System](#world-system) — swappable effects, not mock functions
- [Contracts](#contracts) — requires, decreases, ensures
- [Refinement Types](#refinement-types) — type-level invariants backed by a solver
- [Topology as Typed Boundaries](#topology-as-typed-boundaries) — named dependency handles
- [Labels, Policies, and Trusted Transforms](#labels-policies-and-trusted-transforms) — boundary classification as checked program structure
- [Rooted References, No Imports](#rooted-references-no-imports) — module ownership at every call site
- [Protocol State Types](#protocol-state-types) — state machines at compile time
- [Named Concurrent Regions](#named-concurrent-regions) — the only concurrency surface
- [Semantic Review](#semantic-review) — declaration-level semantic diffs, not line diffs
- [JSON-First CLI](#json-first-cli) — structured output by default
- [Embedded Docs for Cold Starts](#embedded-docs-for-cold-starts) — the binary teaches the language
- [60+ Canonical Rules](#60-canonical-rules) — the full enumerated list

---

## Reading the Code

<a id="reading-the-code"></a>

Sigil uses Unicode characters as namespace and keyword markers. Each character
has one meaning, appears in one syntactic position, and is unambiguous without
surrounding context. This is also a token efficiency decision: `§` is one token; `stdlib::` is two.
`§string.contains` is three tokens; `stdlib::string::contains` is five. Every
rooted reference in Sigil compresses its namespace attribution into a single
character prefix. (Measured with tiktoken `cl100k_base`, the GPT-4 tokenizer.)

**Root sigils** identify which namespace a qualified name belongs to:

| Character | Namespace | Example |
|-----------|-----------|---------|
| `§` | standard library | `§list.sum`, `§string.join`, `§numeric.abs` |
| `†` | runtime / world builders | `†log.capture()`, `†runtime.World` |
| `•` | project modules | `•todoDomain.count`, `•topology.mailerApi`, `•policies.redactSsn`, `•flags.FeatureX` |
| `¤` | config modules | `¤site.basePath()`, `¤release.outputPath()` |
| `☴` | external packages | `☴router.match` |
| `¶` | core prelude | `¶option.mapOption` (most prelude vocabulary is implicit; helpers stay namespaced) |
| `µ` | project-defined sum type constructors | `µTodo`, `µAuditIssue` |
| `※` | test observation (inside test blocks only) | `※check::log.contains`, `※observe::http.requests` |

**Keyword and operator characters:**

| Character | Meaning |
|-----------|---------|
| `λ` | function declaration — `λname(params)=>ReturnType=body` |
| `!` | effect annotation — `=>!Http Result[...]` |
| `#` | length / size prefix — `#list`, `#string`, `#map` |
| `⧺` | list concatenation (`++` is string concatenation) |
| `↦` | map key-value, in types `{K↦V}` and literals `{k↦v}` |
| `¬` | logical NOT (`!` is taken by effects) |
| `≠` `≥` `≤` | not-equal, gte, lte — single-token replacements for `!=` `>=` `<=` |

**Short declaration keywords** — single letters at the start of a line:

| Keyword | Meaning |
|---------|---------|
| `t` | type declaration |
| `e` | extern declaration |
| `c` | module-level constant |
| `l` | local let binding (inside a function body) |

With that in mind, this example becomes readable:

```sigil program tests/observeTest.sigil
λmain()=>Unit=()

test "log is captured" =>!Log world {
  c log=(†log.capture():†log.LogEntry)
} {
  l _=(§io.println("hello"):Unit);
  ※check::log.contains("hello")
}
```

`λmain` declares the entrypoint. `=>!Log` is the effect annotation. `world { ... }` derives a local world overriding `Log` with a capturing implementation. `l _=(...)` sequences an effect without keeping the result. `※check::` asserts over the recorded trace.

---

## One Representation Per AST

<a id="one-representation-per-ast"></a>

The Sigil compiler owns a canonical source printer. Every valid AST has exactly
one accepted textual representation, and the compiler uses this printer as its
enforcement point — non-canonical source is a compile error, not a linter
warning. There is no separate formatter. There is no option to turn on
alternative layouts. There is no `--format` flag that produces a different
arrangement of the same program.

Concretely, this means the compiler defines canonical choices for every
formatting decision that would otherwise be up to the author:

- multiline form is required when an aggregate has two or more items; flat form
  is required with zero or one
- repeated `and`, `or`, `++`, and `⧺` chains print vertically, one continued
  operand per line
- `requires`, `decreases`, and `ensures` (when present) print on successive
  lines in that order, before the body
- `match` branches are always multiline; each arm starts as `pattern=>`
- direct `match` bodies begin on the same line as their header with no `=`

A model that generates valid Sigil generates canonical Sigil — there is only one form.

---

## Match as the Only Branching Surface

<a id="match-as-the-only-branching-surface"></a>

Sigil has one branching construct: `match`. There is no `if`/`else`, no ternary
operator, no `when`, no `cond`. Every conditional execution flows through a
`match` expression.

```sigil module
λclassify(n:Int)=>String match n{
  0=>"zero"|
  value=>match value>0{
    true=>"positive"|
    false=>"negative"
  }
}
```

`match` enforces exhaustiveness. Every branch must be covered; dead branches are
rejected. If a new constructor is added to a sum type, every `match` over that
type that does not handle it becomes a compile error. The compiler finds the
gaps; the model fills them in.

---

## No Shadowing

<a id="no-shadowing"></a>

Sigil rejects all shadowing with `SIGIL-CANON-NO-SHADOWING`. A name introduced
in any scope may not be reused in an inner scope. Every binding site requires a
fresh name; the compiler rejects ambiguity rather than resolving it silently.

```sigil module
λformatResult(n:Int)=>String match n{
  0=>"zero"|
  count=>match count>0{
    true=>"positive "++§string.intToString(count)|
    false=>"negative "++§string.intToString(§numeric.abs(count))
  }
}
```

---

## Canonical Names

<a id="canonical-names"></a>

Names in Sigil follow two rules. Types, constructors, and type variables use
`UpperCamelCase`. Everything else — functions, parameters, constants, locals,
record fields, module path segments, and filenames — uses `lowerCamelCase`.

The compiler enforces this. A function named `ProcessUser` is a compile error.
A type named `emailAddress` is a compile error. There is no linter configuration
to turn this off, no pragma to suppress it, and no formatter to post-process it
away. The canonical form is the only form the compiler accepts.

```sigil module
t UserId=String

t User={
  email:String,
  id:UserId,
  name:String
}

λcreateUser(email:String,name:String)=>User={
  email:email,
  id:"generated",
  name:name
}
```

When five spellings of the same name are equally valid, a model distributes
probability across all five. Sigil has one — `userId` — and the compiler
rejects the rest. This extends to filenames: `UserService.lib.sigil` is
rejected; only `userService.lib.sigil` compiles.

---

## Alphabetical Ordering

<a id="alphabetical-ordering"></a>

Sigil requires alphabetical ordering in three places: function parameters,
record field declarations and usage, and declared effects. Each ordering is
enforced with a distinct error code: `SIGIL-CANON-PARAM-ORDER`,
`SIGIL-CANON-RECORD-TYPE-FIELD-ORDER`, `SIGIL-CANON-RECORD-LITERAL-FIELD-ORDER`,
`SIGIL-CANON-RECORD-PATTERN-FIELD-ORDER`, and `SIGIL-CANON-EFFECT-ORDER`.

With alphabetical ordering, a model generating a call site does not need to
read the function definition. Given `λsend(body:String,headers:{String↦String},to:Email)`,
the argument order is derivable from the names: `b` before `h` before `t`.
The same holds for record literals — alphabetical, always.

```sigil module
t Message={
  body:String,
  from:String,
  subject:String,
  to:String
}

λdraft(body:String,from:String,subject:String,to:String)=>Message={
  body:body,
  from:from,
  subject:subject,
  to:to
}
```

---

## Explicit Effects

<a id="explicit-effects"></a>

Sigil's effect system tracks which effects every function may use, and enforces
those declarations at compile time. The primitive effects are: `Clock`, `Fs`,
`FsWatch`, `Http`, `Log`, `Process`, `Pty`, `Random`, `Sql`, `Stream`, `Task`,
`Tcp`, `Terminal`, `Timer`, and `WebSocket`.

A function that does not declare an effect cannot call a function that requires
one. The propagation is transitive: if `fetchProfile` calls `httpClient.get`,
then `fetchProfile` must declare `!Http`. Any caller that calls `fetchProfile`
must also declare `!Http`. This continues up the call graph. The compiler
enforces the entire chain.

```sigil module
λfetchUser(id:String)=>!Http Result[
  String,
  String
]=Ok("user:"++id)

λpure(x:Int)=>Int=x*2
```

`pure` cannot call `fetchUser` — the compiler rejects it. A function's full
side-effect profile is visible in its signature without reading the body.

Projects may also declare named effects in `src/effects.lib.sigil`. A named
effect must expand to at least two primitives — it cannot alias a single one.
Example: `DatabaseAccess` expanding to `Log` + `Tcp`.

---

## World System

<a id="world-system"></a>

A world is a concrete implementation of the effect primitives. Production code
runs in a production world. Tests run in a test world. Instead of mocking
individual functions, tests swap the entire effect runtime for an instrumented
one that records what happened.

```sigil program tests/logTest.sigil
λmain()=>Unit=()

test "write is observed" =>!Log world {
  c log=(†log.capture():†log.LogEntry)
} {
  l _=(§io.println("hello"):Unit);
  ※check::log.contains("hello")
}
```

`world { ... }` overrides the `Log` effect locally with a capturing implementation.
No separate mock API. No `jest.fn()`. The same `!Log` effect that production
code uses — the world changes, not the code.

The test surface exposes `※observe::http.requests`, `※check::http.calledOnce`,
`※check::log.contains`, and similar helpers — observations over recorded effect
traces, not generic assertion stubs.

---

## Contracts

<a id="contracts"></a>

Sigil has three function contract clauses: `requires`, `decreases`, and
`ensures`. When present, they appear on successive lines before the function
body, in that order. The compiler's proof fragment checks them.

`requires` specifies a precondition over the function's parameters. A caller
that cannot prove the precondition gets a compile error.

`ensures` specifies a postcondition over `result`. Every caller receives this
fact as a proof obligation discharged — if `ensures result > 0` is proved, every
caller that uses the return value may rely on that fact downstream without
re-proving it.

`decreases` is reserved for total self-recursive functions. It provides a pure
integer measure that the solver proves strictly decreases on every recursive
call. Without a provable `decreases` measure, a total function fails to compile.
This eliminates an entire class of runaway recursion bugs.

```sigil module
λdivide(dividend:Int,divisor:Int)=>Int
requires divisor≠0
=dividend/divisor

total λfibonacci(n:Int)=>Int
requires n≥0
=fibonacciStep(
  0,
  1,
  n
)

total λfibonacciStep(a:Int,b:Int,n:Int)=>Int
requires n≥0
decreases n
match n{
  0=>a|
  count=>fibonacciStep(
    b,
    a+b,
    count-1
  )
}
```

The proof fragment covers: integer arithmetic, comparisons, boolean logic,
literal values, field access, list length via `#`, pattern-derived facts from
tuples, records, and nominal sum constructors, and protocol state via
`handle.state`. Facts proved in `ensures` propagate into callers automatically.
`match` arms inject the branch fact into the proof context for their bodies.

These are not documentation strings. They are checked claims.

---

## Refinement Types

<a id="refinement-types"></a>

A type alias in Sigil can carry a `where` clause that refines its underlying
type with a logical predicate. The compiler enforces that predicate at every
point where a raw value is promoted into the refined type.

```sigil module
t NonEmptyString=String where #value>0

t PositiveInt=Int where value>0

t NonEmptyList[T]=[T] where #value>0
```

The refinement proof fragment covers integer and boolean arithmetic, length via
`#`, field access, comparisons, and pattern-derived facts — enough to express
structural invariants like "non-empty", "positive", or "within a range".
Unconstrained aliases (`t Email=String` with no `where`) normalize to their
underlying type and do not create a type-level distinction; the `where` clause
is what gives compile-time enforcement. For predicates that require external
logic — like whether a string matches an email format — the canonical pattern
is boundary conversion via `§decode` helpers.

The refinement lives in the type, not in a validation function. A
`NonEmptyString` is not a `String` that happens to be non-empty at the point
where it was checked. It is a `String` that the compiler requires to be provably
non-empty at every promotion site. Any function that accepts `NonEmptyString`
receives a value the compiler has already verified.

---

## Topology as Typed Boundaries

<a id="topology-as-typed-boundaries"></a>

In most languages, configuration and external endpoints can come from anywhere.
A URL might be hard-coded in the function that uses it. It might live in a
constant three files away. It might come from `process.env` read directly in
business logic. It might be passed as a parameter from a caller that got it
from another caller that read it from a config object that was initialized at
startup. A model working on such a codebase has no reliable way to answer
"where does this endpoint come from?" or "how do I change it for the test
environment?" without reading the entire call graph.

Sigil gives that question one answer. Dependencies are declared in
`src/topology.lib.sigil`, bound per-environment in `config/<env>.lib.sigil`,
and referenced in application code as named handles via `•topology`. Raw
endpoints are rejected by the compiler; the model must use `•topology.serviceName`.

```sigil module projects/topology-http/src/notifyUser.lib.sigil
λnotifyUser(message:String)=>!Http String match §httpClient.post(
  message,
  •topology.mailerApi,
  §httpClient.emptyHeaders(),
  "/notify"
){
  Ok(response)=>response.body|
  Err(error)=>"ERR:"++error.message
}
```

The compiler validates that `mailerApi` is declared in
`src/topology.lib.sigil` and that the selected environment's config wires it
to a concrete URL. If the topology handle is missing from the config, the build
fails with a specific diagnostic — not a runtime `undefined` at the network call.

Configuration questions collapse to two files: `src/topology.lib.sigil` for
what exists, `config/<env>.lib.sigil` for what it resolves to. The model never
invents a URL, and `process.env` in business logic is a compiler error.

---

## Labels, Policies, and Trusted Transforms

<a id="labels-policies-and-trusted-transforms"></a>

In most languages, "can this SSN go into the audit log?" and "should this API
key ever reach the filesystem?" are code review questions. Nothing in the type
system enforces the answer. Sigil makes them compiler questions.

`label` classifies types by domain meaning. Rules attach those classifications
to named topology boundaries with one of three outcomes: `Allow()`, `Block()`,
or `Through(transform)`.

```sigil module
label ApiKey

label Pii

t ApiKey=String label ApiKey

t Ssn=String label Pii
```

```sigil module
transform λredactSsn(ssn:µSsn)=>String="***-**-"++(§string.substring(
  #ssn,
  ssn,
  5
):String)

rule [µ.Pii] for •topology.auditLog=Through(•policies.redactSsn)

rule [µ.ApiKey] for •topology.auditLog=Block()
```

SSNs reaching the audit log are automatically redacted. API keys are blocked
entirely. Both are compile-time enforcement — not conventions, not review
comments, not runtime checks. The type carries the classification, the policy
file carries the rules, and the compiler checks both.

---

## Rooted References, No Imports

<a id="rooted-references-no-imports"></a>

Sigil has no import declarations. Module references are written at their use
sites with explicit root sigils:

- `§list.sum`, `§string.contains`, `§numeric.max` — standard library modules
- `•topology.serviceName`, `•config.databaseUrl`, `•flags.FeatureX` — project configuration
- `☴routerPackage.handlers` — external packages
- `†runtime.World`, `†http.mock` — runtime world builders

```sigil module
λabbreviate(limit:Int,text:String)=>String match #text≤limit{
  true=>text|
  false=>§string.take(
    limit,
    text
  )++"..."
}

λlongest(a:String,b:String)=>String match #a≥#b{
  true=>a|
  false=>b
}
```

The owning module is visible at every call site. No import block to maintain,
no aliased imports that shadow other names, no star imports. Adding a stdlib
call means writing the rooted reference — nothing else.

---

## Protocol State Types

<a id="protocol-state-types"></a>

Handles in real programs follow state machines. A database transaction may only
receive inserts, updates, and deletes while it is open; `commit` and `rollback`
close it; calling any mutation after close is an error. A WebSocket connection
follows a similar pattern. A PTY session follows another.

In most languages, these state machines live in documentation. Sigil encodes
them in the type system.

```sigil module
t Connection={id:String}

protocol Connection
  Closed → Open via open
  Open → Closed via close
  Open → Open via send
  initial = Closed
  terminal = Closed

λclose(connection:Connection)=>Bool
requires connection.state=Open
ensures connection.state=Closed
=true

λopen(connection:Connection)=>Bool
requires connection.state=Closed
ensures connection.state=Open
=true

λsend(connection:Connection,data:String)=>Bool
requires connection.state=Open
ensures connection.state=Open
=true
```

The solver tracks state through the proof context. After `open(conn)`, the
context gains `conn.state = Open`. After `close(conn)`, it becomes `Closed`.
A subsequent call to `send` — which requires `Open` — is a compile error.
Double-close, use-after-close, and wrong-order operations never reach runtime.

---

## Named Concurrent Regions

<a id="named-concurrent-regions"></a>

Sigil has one canonical concurrency surface: the named concurrent region.

```sigil module
λfetchAll(urls:[String])=>!Http [ConcurrentOutcome[
  String,
  String
]]=concurrent batchFetch@10:{
  jitterMs:Some({
    max:50,
    min:5
  }),
  stopOn:isSystemic,
  windowMs:Some(1000)
}{
  spawnEach urls fetchItem
}

λfetchItem(url:String)=>!Http Result[
  String,
  String
]=Ok(url)

λisSystemic(err:String)=>Bool=err="TIMEOUT"
```

A concurrent region is named. Its width — the maximum number of concurrent
children — is required and explicit. Its policy (jitter, stop predicate, rate
window) is optional and, when present, uses alphabetically ordered fields.
The body contains spawn directives, not arbitrary expressions.

There is no `Promise.all`, no `asyncio.gather`, no `goroutine` pattern to
distinguish from. There is one surface that the model generates when code needs
to run work in parallel, and the compiler enforces its structure.

The result type is always `[ConcurrentOutcome[T,E]]` — an ordered list of
outcomes in the same order as the inputs. `Success(value)` when the child
returned `Ok(value)`. `Failure(error)` when it returned `Err(error)`.
`Aborted()` when the region stopped it before it started. Order is stable.
The model can reason about the result shape without reading runtime semantics.

`map` and `filter` remain pure list transforms and are not the concurrency
surface. This separation prevents a common AI generation error: using a parallel
map as a substitute for an explicit concurrency region, with none of the policy
controls.

---

## Semantic Review

<a id="semantic-review"></a>

`git diff` shows what lines changed. `sigil review` shows what those line
changes mean at the declaration level: which functions gained or lost an effect,
which contracts changed, which signatures were modified, whether any changed
functions now lack test coverage evidence. For a human reviewer, that is the
difference between scanning a wall of line noise and opening directly on the
changes that actually matter.

Take this change to a single function — adding the `!Http` effect and a
`requires` precondition:

**Before:**

```sigil module
λfetchUser(id:String)=>Result[
  String,
  String
]=Ok("user:"++id)

λvalidateId(id:String)=>Bool=#id>0
```

**After:**

```sigil module
λfetchUser(id:String)=>!Http Result[
  String,
  String
]
requires #id>0
=Ok("user:"++id)

λvalidateId(id:String)=>Bool=#id>0
```

`git diff` reports:

```text
-λfetchUser(id:String)=>Result[
+λfetchUser(id:String)=>!Http Result[
   String,
   String
-]=Ok("user:"++id)
+]
+requires #id>0
+=Ok("user:"++id)
```

`sigil review` reports:

```text
## Sigil Review

Summary
- changed declarations: 1
- signature changes: 0
- contract changes: 1
- effect changes: 1
- type/refinement changes: 0
- trust surface changes: 0
- changed test files: 0

Effect Changes
- ~ function `fetchUser` in `src/api.lib.sigil`
  - effects: `<none>` -> `!Http`
  - requires: `<none>` -> `#id>0`

Contract Changes
- ~ function `fetchUser` in `src/api.lib.sigil`
  - effects: `<none>` -> `!Http`
  - requires: `<none>` -> `#id>0`

Test Evidence
- changed test files: none
- changed coverage targets: none
```

The diff shows added lines. The review shows that `fetchUser` crossed a
trust boundary: it went from pure to `!Http`, and it now imposes a precondition
that all callers must satisfy. An agent reviewing this change knows immediately
that it needs to check every call site for `!Http` propagation, and that there
is no test evidence for the changed function.

### Usage

```bash
sigil review                    # worktree changes (index → working tree)
sigil review --staged           # staged changes (HEAD → index)
sigil review --base HEAD~1      # last commit
sigil review --base main --head feature-branch
sigil review -- HEAD~3 HEAD     # raw git diff passthrough
```

All modes produce the same structured output: summary counts, per-declaration
change details grouped by kind (signature, effect, contract, termination, trust
surface, implementation), and test evidence.

### `--llm`

`sigil review --llm` emits the same semantic facts wrapped in a prompt preamble
designed for direct use as LLM input:

```text
You are reviewing a Sigil semantic diff.

Use only the facts below.
Do not infer behavior that is not explicitly listed.
If analysisMode is `parseOnly`, call out that limitation.
If any issue has severity `error`, list it first.

Facts:
{
  "formatVersion": 1,
  "command": "sigil review",
  "ok": true,
  "phase": "surface",
  "data": { ... }
}
```

`--json` emits the same fact envelope without the preamble, for agent loops
that parse and route the data themselves.

---

## JSON-First CLI

<a id="json-first-cli"></a>

`compile`, `test`, `inspect`, `validate`, and `featureFlag audit` always emit
JSON — there is no human-readable mode. The output envelope is stable and
versioned (`"formatVersion": 1`). Every error includes a stable `SIGIL-*`
code, the compiler phase, a corrective message, and structured location data.
The `"ok"` field on the envelope tells the agent loop whether the command
succeeded without text scraping.

`run` and `review` have opt-in `--json` flags for different reasons. `run`
passes a program's stdout through directly by default; `--json` wraps
everything — program output, exit code, trace data — in a JSON envelope.
`review` defaults to a human-readable markdown summary; `--json` emits the
full structured fact envelope for agent loops that parse and route the data
themselves. Compile-phase errors from `run` always emit JSON regardless of
the flag.

---

## Embedded Docs for Cold Starts

<a id="embedded-docs-for-cold-starts"></a>

No language model has Sigil in its training data. The binary solves this by
shipping an embedded corpus — syntax reference, stdlib spec, type system spec,
canonical forms doc, and design articles — queryable without a network request:

```bash
sigil docs search "feature flags"
sigil docs context syntax
sigil docs show docs/syntax-reference --start-line 1 --end-line 100
```

All commands return JSON. The embedded docs match the installed binary exactly —
no version drift between the compiler and the spec the model is reading.

---

## 60+ Canonical Rules

<a id="60-canonical-rules"></a>

The canonical constraint hypothesis works because the rules are numerous,
specific, and compiler-enforced. There are currently 66 `SIGIL-CANON-*`
diagnostic codes. The complete authoritative list lives in
`language/compiler/crates/sigil-diagnostics/src/codes.rs`. The most
agent-relevant rules are grouped below.

**Bindings and locals**

- `SIGIL-CANON-NO-SHADOWING` — a name in scope may not be reused in an inner binding
- `SIGIL-CANON-SINGLE-USE-PURE-BINDING` — single-use pure locals must be inlined
- `SIGIL-CANON-UNUSED-BINDING` — unused let bindings are rejected
- `SIGIL-CANON-DEAD-PURE-DISCARD` — pure expressions whose values are discarded are rejected
- `SIGIL-CANON-LET-UNTYPED` — effect-producing bindings must carry a type annotation

**Unused declarations**

- `SIGIL-CANON-UNUSED-DECLARATION` — unreachable top-level declarations rejected in executable files
- `SIGIL-CANON-UNUSED-EXTERN` — unused extern declarations rejected
- `SIGIL-CANON-UNUSED-IMPORT` — unused import declarations rejected

**Ordering**

- `SIGIL-CANON-PARAM-ORDER` — function parameters must be alphabetical
- `SIGIL-CANON-EFFECT-ORDER` — declared effects must be alphabetical
- `SIGIL-CANON-RECORD-TYPE-FIELD-ORDER` — record type fields must be alphabetical
- `SIGIL-CANON-RECORD-LITERAL-FIELD-ORDER` — record literals must list fields alphabetically
- `SIGIL-CANON-RECORD-PATTERN-FIELD-ORDER` — record patterns must list fields alphabetically
- `SIGIL-CANON-DECL-CATEGORY-ORDER` — declarations must follow `label → t → effect → e → featureFlag → c → transform → λ → rule → test`
- `SIGIL-CANON-DECL-ALPHABETICAL` — declarations within each category must be alphabetical

**Naming**

- `SIGIL-CANON-IDENTIFIER-FORM` — functions, parameters, locals, and fields must be lowerCamelCase
- `SIGIL-CANON-TYPE-NAME-FORM` — types must be UpperCamelCase
- `SIGIL-CANON-CONSTRUCTOR-NAME-FORM` — sum type constructors must be UpperCamelCase
- `SIGIL-CANON-FILENAME-CASE` — filenames must start with a lowercase letter
- `SIGIL-CANON-FILENAME-FORMAT` — filenames must be lowerCamelCase

**Recursion and canonical surfaces**

- `SIGIL-CANON-RECURSION-ACCUMULATOR` — accumulator-passing style rejected
- `SIGIL-CANON-BRANCHING-SELF-RECURSION` — multiple sibling self-calls each reducing the same parameter rejected
- `SIGIL-CANON-RECURSION-MISSING-DECREASES` — total self-recursive functions must provide a `decreases` measure
- `SIGIL-CANON-MUTUAL-RECURSION` — top-level mutual recursion rejected
- `SIGIL-CANON-RECURSION-MAP-CLONE` — hand-rolled `map` rejected
- `SIGIL-CANON-RECURSION-FILTER-CLONE` — hand-rolled `filter` rejected
- `SIGIL-CANON-RECURSION-FOLD-CLONE` — hand-rolled `reduce` rejected
- `SIGIL-CANON-HELPER-DIRECT-WRAPPER` — top-level wrappers around canonical helpers rejected
- `SIGIL-CANON-TRAVERSAL-FILTER-COUNT` — `#(filter(...))` rejected; use `§list.countIf`

**Layout**

- `SIGIL-CANON-SOURCE-FORM` — overall source must match canonical printer output
- `SIGIL-CANON-MATCH-LAYOUT` — match expressions must follow canonical layout
- `SIGIL-CANON-MATCH-ARM-LAYOUT` — each match arm must follow canonical form
- `SIGIL-CANON-DELIMITER-SPACING` — canonical spacing around delimiters
- `SIGIL-CANON-OPERATOR-SPACING` — canonical spacing around operators
- `SIGIL-CANON-REDUNDANT-PARENS` — redundant parentheses rejected

**Complete list (all 66)**

`SIGIL-CANON-BLANK-LINES` · `SIGIL-CANON-BRANCHING-SELF-RECURSION` · `SIGIL-CANON-CONSTRUCTOR-NAME-FORM` · `SIGIL-CANON-DEAD-PURE-DISCARD` · `SIGIL-CANON-DECL-ALPHABETICAL` · `SIGIL-CANON-DECL-CATEGORY-ORDER` · `SIGIL-CANON-DECL-EXPORT-ORDER` · `SIGIL-CANON-DELIMITER-SPACING` · `SIGIL-CANON-DUPLICATE-CONST` · `SIGIL-CANON-DUPLICATE-EXTERN` · `SIGIL-CANON-DUPLICATE-FUNCTION` · `SIGIL-CANON-DUPLICATE-IMPORT` · `SIGIL-CANON-DUPLICATE-TEST` · `SIGIL-CANON-DUPLICATE-TYPE` · `SIGIL-CANON-EFFECT-ORDER` · `SIGIL-CANON-EOF-NEWLINE` · `SIGIL-CANON-EXEC-NEEDS-MAIN` · `SIGIL-CANON-EXTERN-MEMBER-ORDER` · `SIGIL-CANON-FEATURE-FLAG-DECL` · `SIGIL-CANON-FILENAME-CASE` · `SIGIL-CANON-FILENAME-FORMAT` · `SIGIL-CANON-FILENAME-INVALID-CHAR` · `SIGIL-CANON-HELPER-DIRECT-WRAPPER` · `SIGIL-CANON-IDENTIFIER-FORM` · `SIGIL-CANON-LET-UNTYPED` · `SIGIL-CANON-LIB-NO-MAIN` · `SIGIL-CANON-MATCH-ARM-LAYOUT` · `SIGIL-CANON-MATCH-BODY-BLOCK` · `SIGIL-CANON-MATCH-LAYOUT` · `SIGIL-CANON-MODULE-PATH-FORM` · `SIGIL-CANON-MUTUAL-RECURSION` · `SIGIL-CANON-NO-SHADOWING` · `SIGIL-CANON-OPERATOR-SPACING` · `SIGIL-CANON-PARAM-ORDER` · `SIGIL-CANON-RECORD-EXACTNESS` · `SIGIL-CANON-RECORD-FIELD-FORM` · `SIGIL-CANON-RECORD-LITERAL-FIELD-ORDER` · `SIGIL-CANON-RECORD-PATTERN-FIELD-ORDER` · `SIGIL-CANON-RECORD-TYPE-FIELD-ORDER` · `SIGIL-CANON-RECURSION-ACCUMULATOR` · `SIGIL-CANON-RECURSION-ALL-CLONE` · `SIGIL-CANON-RECURSION-ANY-CLONE` · `SIGIL-CANON-RECURSION-APPEND-RESULT` · `SIGIL-CANON-RECURSION-COLLECTION-NONSTRUCTURAL` · `SIGIL-CANON-RECURSION-CPS` · `SIGIL-CANON-RECURSION-FILTER-CLONE` · `SIGIL-CANON-RECURSION-FIND-CLONE` · `SIGIL-CANON-RECURSION-FLATMAP-CLONE` · `SIGIL-CANON-RECURSION-FOLD-CLONE` · `SIGIL-CANON-RECURSION-MAP-CLONE` · `SIGIL-CANON-RECURSION-MISSING-DECREASES` · `SIGIL-CANON-RECURSION-REVERSE-CLONE` · `SIGIL-CANON-REDUNDANT-PARENS` · `SIGIL-CANON-SINGLE-USE-PURE-BINDING` · `SIGIL-CANON-SOURCE-FORM` · `SIGIL-CANON-TEST-LOCATION` · `SIGIL-CANON-TEST-NEEDS-MAIN` · `SIGIL-CANON-TEST-PATH` · `SIGIL-CANON-TRAILING-WHITESPACE` · `SIGIL-CANON-TRAVERSAL-FILTER-COUNT` · `SIGIL-CANON-TYPE-NAME-FORM` · `SIGIL-CANON-TYPE-VAR-FORM` · `SIGIL-CANON-UNUSED-BINDING` · `SIGIL-CANON-UNUSED-DECLARATION` · `SIGIL-CANON-UNUSED-EXTERN` · `SIGIL-CANON-UNUSED-IMPORT`

Every dimension of source structure — from filename case to recursion shape to
binding locality — has exactly one canonical form. The compiler exercises that
choice so the model does not have to.

---

## The Combinatorial Effect

Each rule in Sigil removes one dimension of uncertainty from code generation.
The canonical naming rules eliminate naming variation. The ordering rules
eliminate layout variation. The effect system eliminates hidden side-effect
profiles. The contract surface makes preconditions and postconditions explicit.
The world system makes test behavior structurally identical to production
behavior. The no-import surface eliminates import management entirely.

These rules do not just add up — they multiply. A model generating Sigil code
navigates a search space that is smaller at every dimension simultaneously. The
model does not have to choose between naming conventions AND ordering conventions
AND recursion patterns AND import styles AND concurrency surfaces AND mock
strategies. Every choice is made by the compiler, and the compiler communicates
what is canonical through structured JSON diagnostics with stable error codes.

The result is not a language where code generation is merely easier. It is a
language where the gap between "syntactically valid" and "correct and canonical"
is as small as Sigil's design can make it.

```bash
claude "Write a Sigil program that fetches a list of URLs concurrently and 
returns the response lengths"
```

The model writes one program. The compiler tells it, in JSON, exactly what is
wrong and what is canonical. The model corrects the one thing the compiler
identified. The program compiles. It is also the only valid Sigil representation
of that program — because every valid program has exactly one.
