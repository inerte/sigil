---
title: Why Sigil for Coding Agents
---

# Why Sigil for Coding Agents

Most programming languages were designed for human authors. Sigil was designed
for the entity that is now writing most of the world's code.

This is not a claim about AI being special. It is a claim about the mismatch
between how existing languages were designed and how language models actually
generate code. LLMs do not read a codebase and reason holistically. They predict
the next token given the preceding context, and they do so probabilistically.
That means ambiguity — multiple valid forms for the same construct — multiplies
uncertainty at every step.

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
- [Explicit Effects](#explicit-effects) — effects declared, not inferred or hidden
- [World System](#world-system) — swappable effects, not mock functions
- [Contracts](#contracts) — requires, decreases, ensures
- [Refinement Types](#refinement-types) — type-level invariants backed by a solver
- [Topology as Typed Boundaries](#topology-as-typed-boundaries) — named dependency handles
- [Labels, Policies, and Trusted Transforms](#labels-policies-and-trusted-transforms) — boundary classification as checked program structure
- [Canonical Names](#canonical-names) — two case rules, enforced everywhere
- [Alphabetical Ordering](#alphabetical-ordering) — parameters, fields, and effects
- [Rooted References, No Imports](#rooted-references-no-imports) — module ownership at every call site
- [Protocol State Types](#protocol-state-types) — state machines at compile time
- [Semantic Review](#semantic-review) — declaration-level semantic diffs, not line diffs
- [JSON-First CLI](#json-first-cli) — structured output by default
- [Embedded Docs for Cold Starts](#embedded-docs-for-cold-starts) — the binary teaches the language
- [Named Concurrent Regions](#named-concurrent-regions) — the only concurrency surface
- [50+ Canonical Rules](#50-canonical-rules) — the full enumerated list

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
| `•` | project config (topology, config, flags) | `•topology.db`, `•flags.FeatureX` |
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
| `e` | named effect declaration |
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

`λmain` is a function declaration. `=>!Log` means the test uses the `Log`
effect. `world { c log=... }` derives a local world that overrides `Log` with
a capturing implementation (`†log.capture()`). `l _=(...)` is a let binding
whose value is discarded (sequencing an effect). `§io.println` is the stdlib IO
print function. `※check::log.contains` asserts on what the captured log recorded.

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

For code generation, this property is profound. It means there is exactly one
correct way to write any valid Sigil program. A model that generates valid code
generates canonical code. There is no valid variation to choose between, no
layout decision to make, no subtle indentation difference to produce. The
compiler confirms validity and canonicality in one pass.

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
rejected. The compiler's exhaustiveness checker covers `Bool`, `Unit`, tuples,
list shapes, exact record patterns, and nominal sum constructors. If a new
constructor is added to a sum type, every `match` over that type that does not
cover the new constructor becomes a compile error, not a runtime gap.

For a model generating branching logic, this removes the disambiguation problem.
Python has `if`, `elif`, `else`, ternary expressions, match-case, and short-circuit
operators as branching surfaces. Sigil has `match`. The model generates `match`.

Exhaustiveness means the model cannot forget a case. Adding a case to a sum type
propagates into a compile error at every `match` over that type that does not
handle the new constructor. The compiler finds all the gaps; the model fills them
in. There is no way to ship a case analysis with a missing branch.

---

## No Shadowing

<a id="no-shadowing"></a>

Sigil rejects all shadowing with `SIGIL-CANON-NO-SHADOWING`. A name introduced
in an outer scope may not be reused in any inner scope within the same function.

This is the answer to a problem that causes subtle bugs even for expert human
programmers: when two bindings share a name and the inner one silently hides
the outer one, readers must track all active bindings and their precedence at
every point in the function. For a language model, this is compounded — the
model has already predicted the outer binding's name based on its type and
role, and if a later branch introduces the same name for a different value, the
model must revise its implicit mapping.

Sigil requires fresh, descriptive names at every binding site. If `result` is
already in scope, the inner binding must be named something else — `parsedResult`,
`validatedResult`, `nextResult`. The compiler rejects ambiguity rather than
resolving it silently.

```sigil module
λformatResult(n:Int)=>String match n{
  0=>"zero"|
  count=>match count>0{
    true=>"positive "++§string.intToString(count)|
    false=>"negative "++§string.intToString(§numeric.abs(count))
  }
}
```

Every name in this function refers to exactly one value. There is no question
about which `count` or which `result` a reference resolves to, because there
can only be one.

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

`pure` cannot call `fetchUser`. The compiler rejects the call because `pure`
has no declared effects and `fetchUser` requires `!Http`. There is no runtime
mechanism needed to detect this. The call graph's effect requirements are a
static property of the program.

For a language model generating code, this produces a clear contract at every
function boundary. If a function signature declares `!Http`, the model knows
that function interacts with the network. If a function signature declares no
effects, the model knows it is computationally pure. A model generating a pure
function cannot accidentally introduce a network call — the compiler rejects it.
A model reading a function signature knows its full side-effect profile without
reading the body.

Projects may also declare named effects in `src/effects.lib.sigil`. A named
effect must expand to at least two primitive effects, which means it cannot be
used to hide a single primitive under an alias. Named effects encourage
consistent cross-module effect annotation for domain-specific boundaries like
`DatabaseAccess` (expanding to `Log` + `Tcp`) or `StorageOps` (expanding to
`Fs` + `Log`).

---

## World System

<a id="world-system"></a>

Sigil treats all effectful behavior as world-dependent. A world is a concrete
implementation of the effect primitives — it specifies how `Http` calls behave,
how `Fs` operations behave, how `Log` messages are collected. Production code
runs in a production world. Tests run in a test world. The runtime's effect
system consults the active world for every effect operation.

This replaces the conventional mocking pattern. Mock-based testing replaces
specific functions with substitutes. World-based testing replaces the entire
effect runtime with an instrumented one that records what happened.

```sigil program tests/logTest.sigil
λmain()=>Unit=()

test "write is observed" =>!Log world {
  c log=(†log.capture():†log.LogEntry)
} {
  l _=(§io.println("hello"):Unit);
  ※check::log.contains("hello")
}
```

The `world { ... }` block in a test is a local derivation from the project's
baseline test world. The test overrides the `Log` effect with a capturing
implementation, runs the test body, and then uses `※check::log.contains` to
assert on what was captured. The `Log` effect in the test body is the same `Log`
effect that production code uses. Nothing was mocked. The world changed.

For code generation, this has a clean structural property: the model generates
test code that uses the same effect names as production code. There is no
separate mock API to learn. There is no `jest.fn()` or `sinon.stub()`. There is
one surface — the effect system — and the world determines what it does.

The test surface also exposes `※observe::http.requests`, `※observe::log.entries`,
`※check::http.calledOnce`, `※check::log.contains`, and similar helpers. These
are not generic assertion helpers. They are observations over recorded effect
traces that the runtime world collected during the test.

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

For code generation, contracts are a precise specification surface. A model
generating a function that divides two numbers can express `requires divisor≠0`
and have the compiler enforce it at every call site. A model generating a
recursive function can express its termination measure and have the compiler
verify it. These are not documentation strings. They are checked claims.

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

This encourages a discipline that matters for correctness in generated code:
early boundary conversion. Raw user input arrives as `String`. The model
generates code that decodes and validates that string into `Email` at the
system boundary, using `§decode` helpers. Once the value crosses into the
interior of the application as an `Email`, its invariant is a compiler-checked
fact, not a hope.

The refinement lives in the type, not in a validation function. A
`NonEmptyString` is not a `String` that happens to be non-empty at the point
where it was checked. It is a `String` that the compiler requires to be
provably non-empty at every promotion site. Any function that accepts
`NonEmptyString` receives a value the compiler has already verified.

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

Sigil gives that question one answer. External dependencies live in
`src/topology.lib.sigil`. Their concrete values for each environment live in
`config/<env>.lib.sigil`. Application code references them through the
`•topology` root. That is the complete list of places to look.

Topology-aware projects declare all external HTTP, TCP, and filesystem
dependencies as named handles in `src/topology.lib.sigil`. Application code
then refers to those handles through the `•topology` root. Concrete URLs, hosts,
and ports are bound in `config/<env>.lib.sigil` and are never visible to
application code.

This means a model generating application code cannot hard-code a URL. The
compiler rejects raw endpoints in topology-aware project code. The model must
use a named handle: `•topology.searchService`, `•topology.database`,
`•topology.reportStore`. The actual URL bound to each handle is an
environment-level concern, verified when the project is compiled with `--env`.

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

For an AI coding agent working on a topology-aware project, this collapses the
search space for every configuration question to two files. Where is this
endpoint declared? `src/topology.lib.sigil`. What is it set to in production?
`config/production.lib.sigil`. How do I configure a test double? Derive a local
world in the test file. The model never needs to invent, guess, or remember a
URL — and it can never accidentally read one from `process.env` in the middle
of business logic, because the compiler rejects that too.

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

For a language model, this matters because token prediction probability
concentrates. When `userId`, `user_id`, `UserId`, `user_Id`, and `USERID` are
all valid representations of the same concept in a language, the model
distributes probability across all of them at every token boundary. In Sigil,
there is only `userId`. The model generates it because it is the only valid
spelling, and the compiler confirms it.

The naming rules have stable error codes: `SIGIL-CANON-IDENTIFIER-FORM`,
`SIGIL-CANON-TYPE-NAME-FORM`, `SIGIL-CANON-CONSTRUCTOR-NAME-FORM`, and
`SIGIL-CANON-TYPE-VAR-FORM`. Each one fires with a corrective message that
states what was found and what is required.

This extends to filenames. A file named `UserService.lib.sigil` is rejected
with `SIGIL-CANON-FILENAME-CASE`. Only `userService.lib.sigil` is accepted.
Filenames participate in module path resolution, so filename canonicalization
eliminates an entire class of case-sensitivity bugs on case-insensitive
filesystems.

---

## Alphabetical Ordering

<a id="alphabetical-ordering"></a>

Sigil requires alphabetical ordering in three places: function parameters,
record field declarations and usage, and declared effects. Each ordering is
enforced with a distinct error code: `SIGIL-CANON-PARAM-ORDER`,
`SIGIL-CANON-RECORD-TYPE-FIELD-ORDER`, `SIGIL-CANON-RECORD-LITERAL-FIELD-ORDER`,
`SIGIL-CANON-RECORD-PATTERN-FIELD-ORDER`, and `SIGIL-CANON-EFFECT-ORDER`.

The machine-first implication of this rule is often underestimated. When
parameter order is alphabetical, a model generating a call site does not need
to read the function definition to know the correct argument order. Given a
function `λsend(body:String,headers:{String↦String},to:Email)`, the parameter
order is derivable from the names alone: `b` before `h` before `t`.

The same holds for record literals. A model creating a `User` value knows the
field order without reading the type declaration: alphabetical by field name,
always. The field that comes first in the alphabet comes first in the literal,
the pattern, and the type definition.

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

This eliminates the "which order do the arguments go in" problem for every
function call a model generates. The order is derivable from the grammar, not
from memory.

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

The module a function belongs to is visible at every call site. There is no
`from foo import bar as baz` to trace. There is no aliased import that shadows
a different module with the same local name. There is no star import that
introduces an unknown set of names into scope.

For a model reading unfamiliar code, this means module attribution is always
explicit. For a model generating code, there is no import block to maintain.
The model writes the rooted reference at the call site, and the compiler
resolves it. Adding a new stdlib call does not require a corresponding import
statement in a different part of the file.

The root sigils themselves are part of the canonical naming convention. They are
not user-configurable aliases. `§` always means the standard library. `•` always
means project configuration. `☴` always means external packages. `†` always
means the runtime world. A model that learns these four sigils understands the
full module resolution story for Sigil.

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

The `protocol` declaration names the state machine and its transitions. Functions
listed in `via` carry matching `requires`/`ensures` state annotations — those are
the same contract clauses that exist for value contracts, extended to state.

The solver tracks state through the proof context. After `open(conn)`, the proof
context contains `conn.state = Open`. A subsequent call to `send` that requires
`Open` succeeds. After `close(conn)`, the proof context contains
`conn.state = Closed`. Any subsequent call to `send` — which requires `Open` —
produces a compile error with a counterexample from the solver.

Double-close, use-after-close, and wrong-order operations are all type errors.
The programmer does not need to remember protocol state; the compiler tracks it.
For a language model, this is especially powerful: the model generates calls in
a sequence, and the compiler tells it which calls are valid given the current
protocol state. There is no runtime consequence of getting it wrong — the
program does not compile.

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

The facts object contains the full structured change data: each changed
declaration, its before and after signatures, effects, contracts, and the
inferred `changeKinds` list (`"effects"`, `"requires"`, `"ensures"`,
`"implementation"`, `"signature"`, `"trustSurface"`, etc.). The preamble
instructs the model to work only from what is listed, not from inference about
the surrounding codebase.

`--json` emits the same fact envelope without the preamble, for agent loops
that parse and route the data themselves.

---

## JSON-First CLI

<a id="json-first-cli"></a>

Every Sigil compiler command that produces diagnostic or structural output does
so in JSON. There is no human-readable output mode for `compile`, `test`,
`inspect`, `validate`, or `featureFlag audit`. JSON is the output format, not
an opt-in flag.

This is the opposite of most language toolchains, where human-readable text is
the default and `--json` adds a machine-readable mode as an afterthought.
Sigil's design starts from the assumption that the primary consumer of compiler
output is an agent loop, and human readers can run `jq`, `fx`, or any JSON
viewer to inspect it.

The output structure is stable and versioned with `"formatVersion": 1`. Every
error includes:

- `"code"` — the stable `SIGIL-*` error code
- `"phase"` — which compiler phase produced the error
- `"message"` — a human-readable description that states what was found and
  what is canonical
- `"details"` — structured fields with file path, source location (line,
  column, offset), expected and found types, and any additional context

An agent loop can parse every compiler output with a single JSON decode. There
is no text scraping, no regex on error messages, no need to detect whether the
output is a success or an error by its shape — the `"ok"` field on the envelope
handles that.

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

Sigil is new enough that installed language models do not have its syntax in
their weights. A model asked to write Sigil code without context is working from
zero prior knowledge. This is a genuinely different situation from generating
Python or TypeScript, where the model's training data contains millions of
examples.

Sigil's answer is that the binary teaches the language. Every `sigil` installation
ships an embedded corpus containing:

- the syntax reference
- the stdlib specification
- the formal type system spec
- the canonical forms documentation
- design articles explaining the rationale behind each decision

These are accessible without a network request:

```bash
sigil docs list
sigil docs search "feature flags"
sigil docs context syntax
sigil docs show docs/syntax-reference --start-line 1 --end-line 100
```

The retrieval commands return JSON. An agent loop can call `sigil docs search`
with a keyword, receive a list of matching documents with their locations, call
`sigil docs show` to read the relevant section, and generate code with accurate
prior knowledge of the language — all without a web lookup.

The embedded docs match the installed binary exactly. There is no version drift
between the compiler the user is running and the docs the model is reading.
If the user upgrades from one Sigil release to another, the docs upgrade with
the binary.

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

## 50+ Canonical Rules

<a id="50-canonical-rules"></a>

The canonical constraint hypothesis works because the rules are numerous,
specific, and compiler-enforced. Together, they define one valid shape for
every programming construct. The following is the complete enumerated list of
`SIGIL-CANON-*` error codes.

**Duplication**

- `SIGIL-CANON-DUPLICATE-TYPE` — two type declarations with the same name
- `SIGIL-CANON-DUPLICATE-EXTERN` — two extern declarations with the same name
- `SIGIL-CANON-DUPLICATE-CONST` — two const declarations with the same name
- `SIGIL-CANON-DUPLICATE-FUNCTION` — two function declarations with the same name
- `SIGIL-CANON-DUPLICATE-TEST` — two test blocks with the same description

**Source shape**

- `SIGIL-CANON-EOF-NEWLINE` — file must end with a newline
- `SIGIL-CANON-TRAILING-WHITESPACE` — no trailing whitespace on any line
- `SIGIL-CANON-BLANK-LINES` — no blank lines within a declaration

**File kind**

- `SIGIL-CANON-LIB-NO-MAIN` — library files may not declare `main`
- `SIGIL-CANON-EXEC-NEEDS-MAIN` — executable files must declare `main`
- `SIGIL-CANON-TEST-NEEDS-MAIN` — test files must declare `main()=>Unit=()`
- `SIGIL-CANON-TEST-LOCATION` — tests may only appear in `tests/` directories
- `SIGIL-CANON-TEST-PATH` — test file path must match expected pattern

**Filenames**

- `SIGIL-CANON-FILENAME-CASE` — filename must start with a lowercase letter
- `SIGIL-CANON-FILENAME-INVALID-CHAR` — filename may not contain `_`, `-`, or other non-alphanumeric characters
- `SIGIL-CANON-FILENAME-FORMAT` — filename must be lowerCamelCase

**Naming**

- `SIGIL-CANON-IDENTIFIER-FORM` — functions, parameters, locals, fields, and filenames must be lowerCamelCase
- `SIGIL-CANON-TYPE-NAME-FORM` — types must be UpperCamelCase
- `SIGIL-CANON-CONSTRUCTOR-NAME-FORM` — sum type constructors must be UpperCamelCase
- `SIGIL-CANON-TYPE-VAR-FORM` — type variables must be UpperCamelCase
- `SIGIL-CANON-RECORD-FIELD-FORM` — record field names must be lowerCamelCase
- `SIGIL-CANON-MODULE-PATH-FORM` — module path segments must follow canonical form

**Recursion**

- `SIGIL-CANON-RECURSION-ACCUMULATOR` — accumulator-passing style is rejected; use canonical iterative helpers
- `SIGIL-CANON-RECURSION-COLLECTION-NONSTRUCTURAL` — non-structural collection recursion rejected
- `SIGIL-CANON-RECURSION-CPS` — continuation-passing style is rejected
- `SIGIL-CANON-RECURSION-APPEND-RESULT` — appending to recursive result is rejected; use a helper with a final reverse
- `SIGIL-CANON-RECURSION-ALL-CLONE` — hand-rolled reimplementation of `§list.all` rejected
- `SIGIL-CANON-RECURSION-ANY-CLONE` — hand-rolled reimplementation of `§list.any` rejected
- `SIGIL-CANON-RECURSION-MAP-CLONE` — hand-rolled reimplementation of `map` rejected
- `SIGIL-CANON-RECURSION-FILTER-CLONE` — hand-rolled reimplementation of `filter` rejected
- `SIGIL-CANON-RECURSION-FIND-CLONE` — hand-rolled reimplementation of `§list.find` rejected
- `SIGIL-CANON-RECURSION-FLATMAP-CLONE` — hand-rolled reimplementation of `§list.flatMap` rejected
- `SIGIL-CANON-RECURSION-REVERSE-CLONE` — hand-rolled reimplementation of `§list.reverse` rejected
- `SIGIL-CANON-RECURSION-FOLD-CLONE` — hand-rolled reimplementation of `reduce ... from ...` rejected
- `SIGIL-CANON-BRANCHING-SELF-RECURSION` — multiple sibling self-calls each reducing the same parameter rejected (e.g., naive fibonacci)
- `SIGIL-CANON-RECURSION-MISSING-DECREASES` — total self-recursive function must provide a `decreases` measure
- `SIGIL-CANON-ORDINARY-DECREASES` — ordinary functions may not declare `decreases`
- `SIGIL-CANON-MUTUAL-RECURSION` — top-level mutual recursion within a module is rejected
- `SIGIL-CANON-TRAVERSAL-FILTER-COUNT` — `#(filter(...))` is rejected; use `§list.countIf`

**Ordering**

- `SIGIL-CANON-PARAM-ORDER` — function parameters must be in alphabetical order
- `SIGIL-CANON-EFFECT-ORDER` — declared effects must be in alphabetical order
- `SIGIL-CANON-RECORD-TYPE-FIELD-ORDER` — record type fields must be alphabetical
- `SIGIL-CANON-RECORD-LITERAL-FIELD-ORDER` — record literals must list fields alphabetically
- `SIGIL-CANON-RECORD-PATTERN-FIELD-ORDER` — record patterns must list fields alphabetically
- `SIGIL-CANON-DECL-CATEGORY-ORDER` — declarations must follow `t → e → i → c → λ → test` order
- `SIGIL-CANON-DECL-EXPORT-ORDER` — exported declarations must precede non-exported ones within each category
- `SIGIL-CANON-DECL-ALPHABETICAL` — declarations within each category must be alphabetical
- `SIGIL-CANON-EXTERN-MEMBER-ORDER` — extern members must be ordered

**Bindings**

- `SIGIL-CANON-NO-SHADOWING` — a name in scope may not be reused in an inner binding
- `SIGIL-CANON-LET-UNTYPED` — `let` bindings that produce effects must be typed
- `SIGIL-CANON-DEAD-PURE-DISCARD` — pure expressions whose values are discarded are rejected

These rules collectively mean that every dimension of source structure — from
the character case of a filename to the order in which declarations appear to
the recursion pattern used to traverse a list — has exactly one canonical form.
There is no creative latitude that the model needs to exercise for any of these
choices, because the compiler exercises it for you.

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
