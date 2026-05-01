---
title: Why Sigil for Coding Agents
---

# Why Sigil for Coding Agents

Most programming languages were designed for human authors first. Sigil was
designed around a different question: what would the surface look like if you
wanted code generation, code reading, and code review by language models to be
predictable rather than merely possible?

The core idea is simple:

- reduce representational freedom
- make important semantics explicit in syntax
- give the compiler one canonical answer whenever a human language would usually
  allow five

That is Sigil's canonical constraint hypothesis. The goal is not to make code
harder to write. The goal is to shrink the search space an agent has to explore
at every step:

- how do I spell this construct?
- where does this dependency come from?
- which effect does this function use?
- what shape should this recursion take?
- which cases are still uncovered?

Sigil answers those questions structurally, not stylistically.

---

## On This Page

- [Reading the Code](#reading-the-code) - the sigils, operators, and declaration markers
- [One Representation Per AST](#one-representation-per-ast) - canonical source, enforced by the compiler
- [Rooted References, No Imports, Fixed Project Layout](#rooted-references-no-imports-fixed-project-layout) - where names come from and where project structure lives
- [Anti-Slop Canonical Surfaces](#anti-slop-canonical-surfaces) - unused code, wrapper bans, and canonical helper surfaces
- [Match as the Only Branching Surface](#match-as-the-only-branching-surface) - one way to branch, exhaustive by construction
- [Local Determinism](#local-determinism) - naming, ordering, and no shadowing
- [Explicit Effects](#explicit-effects) - side effects declared in signatures
- [World System](#world-system) - explicit test/runtime effect surfaces
- [Topology as Typed Boundaries](#topology-as-typed-boundaries) - named dependency handles instead of raw endpoints
- [Labels, Policies, and Trusted Transforms](#labels-policies-and-trusted-transforms) - boundary policy as checked program structure
- [Contracts](#contracts) - requires, decreases, ensures
- [Refinement Types](#refinement-types) - compiler-checked invariants
- [Protocol State Types](#protocol-state-types) - state machines in the type system
- [Named Concurrent Regions](#named-concurrent-regions) - one widening surface for concurrency
- [Semantic Review](#semantic-review) - declaration-level diffs
- [JSON-First CLI](#json-first-cli) - structured compiler output by default
- [Embedded Docs for Cold Starts](#embedded-docs-for-cold-starts) - the installed binary teaches the language
- [Appendix: Current SIGIL-CANON Families](#appendix-current-sigil-canon-families) - implementation-exported canonical diagnostics

---

## Reading the Code

<a id="reading-the-code"></a>

Sigil uses Unicode sigils as namespace and keyword markers. Each one has a
stable role. The point is not ornament. The point is compression and
disambiguation.

**Rooted surfaces**

| Surface | Meaning | Example |
|---------|---------|---------|
| `§` | standard library | `§list.sum`, `§string.join`, `§numeric.abs` |
| `•` | project-owned modules and project surfaces | `•todoDomain.completedCount`, `•topology.mailerApi`, `•policies.redactSsn`, `•flags.NewCheckout` |
| `¤` | config modules | `¤site.basePath()`, `¤release.outputPath()` |
| `¶` | core helpers | `¶map.empty()`, `¶option.mapOption(...)` |
| `†` | runtime/world builders and entries | `†log.capture()`, `†runtime.World` |
| `※` | test observation and test checks | `※observe::http.requests`, `※check::log.contains` |
| `☴` | external packages declared in `sigil.json` | `☴router.match` |
| `µ` | project-defined named types and project sum constructors | `µTodo`, `µGovBrToken` |

Selected env declarations in topology-aware projects are also projected through
`•config.<name>`, for example `•config.flags`.

**Keyword and operator characters**

| Character | Meaning |
|-----------|---------|
| `λ` | function declaration |
| `!` | effect annotation |
| `#` | length / size |
| `⧺` | list concatenation |
| `↦` | map entry constructor in `{K↦V}` |
| `¬` | logical not |
| `≠` `≥` `≤` | comparison operators |

**Short declaration keywords**

| Keyword | Meaning |
|---------|---------|
| `t` | type declaration |
| `e` | effect declaration |
| `c` | const declaration |
| `l` | local binding |

With that in mind, this becomes readable quickly:

```sigil program tests/observeTest.sigil
λmain()=>Unit=()

test "log is captured" =>!Log world {
  c log=(†log.capture():†log.LogEntry)
} {
  l _=(§io.println("hello"):Unit);
  ※check::log.contains("hello")
}
```

`λmain` is a function. `=>!Log` declares the effect surface. `world { ... }`
derives a test-local world overlay. `†log.capture()` installs a capturing log
entry. `l _=(...)` sequences an effect without keeping a reusable name.
`※check::log.contains` asserts over the recorded trace.

For a coding agent, this matters immediately:

- namespace attribution is visible at the call site
- declaration kind is visible at the first token
- test observation is a separate, unmistakable surface
- there is no import block or implicit scope setup to reconstruct first

---

## One Representation Per AST

<a id="one-representation-per-ast"></a>

Sigil is printer-first. The compiler owns the canonical source printer, and a
parsed program has one accepted printed form. Non-canonical source is rejected
at compile time.

There is no public formatter command. There is no linter-only phase where style
is "recommended". Canonicality is part of validity.

Important examples:

- zero- and one-item aggregates stay flat
- two-or-more-item aggregates print multiline
- repeated `and`, `or`, `++`, and `⧺` chains print vertically
- `requires`, `decreases`, and `ensures` print on successive lines in that
  order
- direct `match` bodies stay `match ...` with no `=`
- each `match` arm begins `pattern=>`

Comments are valid syntax but are not part of canonical source comparison. The
canonical claim is therefore about the program text the compiler owns, not about
comment placement.

For code generation, this is one of Sigil's most important properties. The
model does not need to choose among multiple valid layouts for the same program.
If it generated valid Sigil, it generated canonical Sigil too.

---

## Rooted References, No Imports, Fixed Project Layout

<a id="rooted-references-no-imports-fixed-project-layout"></a>

Sigil has no import declarations. References are written rooted at the use
site.

```sigil module projects/todo-app/src/countTodos.lib.sigil
λtodoCount(todos:[µTodo])=>Int=•todoDomain.completedCount(todos)
```

```sigil expr
§list.last(items)
```

```sigil expr
¤site.basePath()
```

This does two things at once:

- the owning module is visible exactly where the name is used
- adding a new dependency does not require editing a second import surface

Sigil also makes project structure predictable. In a project, important kinds of
information have canonical homes:

- `src/types.lib.sigil` owns project-defined `t` and `label` declarations
- `src/effects.lib.sigil` owns project-defined multi-effect aliases
- `src/topology.lib.sigil` owns named boundary handles and environment names
- `src/policies.lib.sigil` owns `rule` and `transform` declarations
- `config/<env>.lib.sigil` owns the selected environment world plus env-selected
  declarations such as flags

Module scope itself is declaration-only. Top level is not a place for setup
logic or arbitrary statements.

This is unusually helpful for an agent. "Where should this go?" stops being an
open-ended repository search and becomes a small number of deterministic
choices.

External packages are constrained the same way. `☴name` resolves only against
direct dependencies declared in `sigil.json`. Transitive package imports are
rejected.

That means:

- if the code says `☴router`, `sigil.json` must name `router`
- provenance is local rather than inferred through a dependency graph
- an agent never has to guess whether some transitive dependency is "probably"
  available

---

## Anti-Slop Canonical Surfaces

<a id="anti-slop-canonical-surfaces"></a>

Sigil does not only canonicalize punctuation and layout. It also canonicalizes
common low-signal programming habits that models frequently reproduce:

- unused named bindings are rejected
- unused top-level declarations in executable `.sigil` files are rejected
- unused externs in executable `.sigil` files are rejected
- pure single-use locals must be inlined
- wildcard sequencing may not discard pure expressions
- exact direct wrappers around canonical helpers are rejected
- hand-rolled clones of canonical list-processing surfaces are rejected

Sigil already has an opinionated surface for common traversal work:

- `xs map fn`
- `xs filter pred`
- `xs reduce step from seed`
- `§list.find`
- `§list.any`
- `§list.all`
- `§list.flatMap`
- `§list.countIf`
- `§list.reverse`

The language would rather reject a plausible-but-redundant near-duplicate than
let the codebase fill up with slightly different handwritten versions of the
same traversal.

That is valuable for humans, but it is especially valuable for LLMs. Generated
code tends to drift toward:

- unnecessary intermediate names
- dead helper functions
- wrappers that only rename a stdlib function
- recursive list plumbing where a direct canonical helper already exists

Sigil removes those degrees of freedom from the generator.

---

## Match as the Only Branching Surface

<a id="match-as-the-only-branching-surface"></a>

Sigil has one branching construct: `match`.

There is no `if`/`else`, no ternary operator, no `when`, no `cond`.

```sigil module
λclassify(n:Int)=>String match n{
  0=>"zero"|
  value=>match value>0{
    true=>"positive"|
    false=>"negative"
  }
}
```

`match` is also exhaustiveness-checked. The compiler checks coverage for:

- `Bool`
- `Unit`
- tuples
- list shapes
- exact record patterns
- nominal sum constructors

If a sum type gains a new constructor, every non-exhaustive `match` becomes a
compile error.

For agents, this matters because it removes both surface and semantic
ambiguity:

- there is one way to branch
- the compiler tells the model which cases are still missing
- adding a new case in one file generates a precise repair list elsewhere

---

## Local Determinism

<a id="local-determinism"></a>

Several Sigil rules are individually small, but together they make local code
far easier for a model to keep straight.

**Canonical naming**

- types, constructors, and type variables use `UpperCamelCase`
- functions, locals, fields, constants, filenames, and path segments use
  `lowerCamelCase`

**No shadowing**

- a name in scope may not be reused in an inner binding

**Alphabetical ordering**

- function parameters are alphabetical
- record fields are alphabetical in type declarations, literals, and patterns
- declared effects are alphabetical

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

The point is not aesthetics. It is predictability.

An agent generating a call site does not have to remember ad hoc parameter
ordering. An agent reading a function does not have to resolve which `result`
or which `count` an inner block shadowed. A filename that resolves as a module
must already be in canonical case.

This is local determinism: once the model understands the rule, it can apply it
everywhere without rereading the entire repository.

---

## Explicit Effects

<a id="explicit-effects"></a>

Sigil tracks effect usage in signatures and enforces those declarations through
the call graph.

Current primitive effects in the implementation are:

- `Clock`
- `Fs`
- `FsWatch`
- `Http`
- `Log`
- `Process`
- `Pty`
- `Random`
- `Sql`
- `Stream`
- `Task`
- `Tcp`
- `Terminal`
- `Timer`
- `WebSocket`

```sigil module
λfetchUser(id:String)=>!Http Result[
  String,
  String
]=Ok("user:"++id)

λpure(x:Int)=>Int=x*2
```

`pure` cannot call `fetchUser` unless it becomes effectful too. Effect
propagation is explicit and transitive.

Projects may also define reusable multi-effect aliases in `src/effects.lib.sigil`.
These aliases must expand to at least two primitive effects, so they cannot be
used to smuggle a single primitive effect behind a local nickname.

For an agent, explicit effects turn function signatures into reliable summaries.
You do not have to inspect the body to know whether a function can talk to the
network, filesystem, database, terminal, or subprocess layer.

---

## World System

<a id="world-system"></a>

Sigil exposes an explicit runtime world surface for test overlays and
topology-aware execution. Instead of teaching the model a second mocking API,
Sigil lets tests observe or replace runtime entries directly.

```sigil program tests/logTest.sigil
λmain()=>Unit=()

test "write is observed" =>!Log world {
  c log=(†log.capture():†log.LogEntry)
} {
  l _=(§io.println("hello"):Unit);
  ※check::log.contains("hello")
}
```

The test uses the real effect name, `Log`, and changes the world entry. The
observation surface is also explicit:

- `※observe::http.requests`
- `※observe::log.entries`
- `※observe::process.commandsAt(...)`
- `※check::http.calledOnce`
- `※check::log.contains`
- `※check::process.calledOnceAt(...)`

I specifically checked the implementation surface here: `※check::http.calledWith`
is not currently present, so this article does not claim it.

Why this helps agents:

- tests use the same effect vocabulary as production code
- there is one observation surface instead of a zoo of library-specific mocks
- traces are first-class data, not text scraped from logs

---

## Topology as Typed Boundaries

<a id="topology-as-typed-boundaries"></a>

Most codebases let external dependencies leak everywhere:

- raw URLs inside business logic
- hostnames passed by string
- `process.env` reads in arbitrary modules
- tests that patch the wrong layer because there is no single boundary concept

Sigil prefers one explicit model:

- `src/topology.lib.sigil` declares named handles
- `config/<env>.lib.sigil` binds those handles for one environment
- application code uses `•topology...`
- in projects, config modules are the place where `process.env` belongs

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

The handle `mailerApi` is declared in `src/topology.lib.sigil`. The selected
environment config must wire it. If it is missing, the build fails before the
program runs.

For an agent, this collapses a configuration question that usually spans dozens
of files into a short deterministic path:

- boundary declaration: `src/topology.lib.sigil`
- boundary wiring: `config/<env>.lib.sigil`
- application usage: `•topology.handleName`

---

## Labels, Policies, and Trusted Transforms

<a id="labels-policies-and-trusted-transforms"></a>

This is one of Sigil's most unusual and most agent-friendly ideas.

`where` handles value refinement. `label` handles type classification. Boundary
handling then lives in `src/policies.lib.sigil`.

```sigil module projects/labelled-boundaries/src/types.lib.sigil
label Brazil

label Credential

label GovAuth

label Pii

label Usa

t Cpf=String label [Brazil,Pii]

t GovBrToken=String label [Brazil,Credential,GovAuth]

t Ssn=String label [Pii,Usa]
```

```sigil module projects/labelled-boundaries/src/policies.lib.sigil
transform λgovBrCommand(token:µGovBrToken)=>§process.Command=§process.withEnv(
  §process.command(["gov-client"]),
  {"TOKEN"↦token}
)

transform λredactSsn(ssn:µSsn)=>String="***-**-"++(§string.substring(
  #ssn,
  ssn,
  5
):String)

rule [µ.Brazil,µ.Credential,µ.GovAuth] for •topology.govBrCli=Through(•policies.govBrCommand)

rule [µ.Pii,µ.Usa] for •topology.auditLog=Through(•policies.redactSsn)

rule [µ.Brazil,µ.Pii] for •topology.exportsDir=Allow()
```

This is a different design from ordinary refinement types:

- labels are nominal classifications, not value predicates
- rules attach those classifications to named boundaries
- transforms name the trusted conversions that may cross a boundary

Why this is unusually good for agents:

- boundary policy has a canonical home
- "can this value go there?" is answered structurally
- trusted transformations are named program objects, not informal review lore
- the model does not have to infer a data-governance policy from scattered
  comments and conventions

If you want to convince someone that Sigil was designed for machine reasoning,
this section belongs near the top.

---

## Contracts

<a id="contracts"></a>

Sigil's function-contract surface is:

- `requires`
- `decreases`
- `ensures`

They appear in that order when present.

Top-level functions are ordinary by default. `mode total` sets a file default,
and `total` / `ordinary` may override per declaration. `decreases` is reserved
for total self-recursive functions. Total functions may not call ordinary ones.

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

These are checked claims, not comments. Callers must prove `requires`.
`ensures` facts propagate to callers. `decreases` must be solver-provable for
total self-recursion.

For an agent, contracts provide a precise target surface: the model can state
what must hold, and the compiler checks whether the code actually supports it.

---

## Refinement Types

<a id="refinement-types"></a>

Sigil uses `where` for compile-time value refinement.

```sigil module
t Email=String

t NonEmptyString=String where #value>0

t PositiveInt=Int where value>0

t NonEmptyList[T]=[T] where #value>0
```

The important semantic distinction is:

- unconstrained aliases like `Email` are structural aliases
- constrained aliases like `NonEmptyString` act as compile-time refinements over
  the underlying type

So `t Email=String` improves readability, but it does not create nominal
separation from `String` by itself. The real machine-checked power comes from
`where` constraints and from the boundary-policy layer described in the labels
section.

This matters because it keeps the article honest about what Sigil is and is not
doing today. If you need "provably non-empty", use a refinement. If you need
"this boundary may accept PII only after redaction", use labels plus rules plus
transforms.

The practical agent story is still strong:

- invariants live in the type surface
- promotion into a constrained type requires proof
- early boundary conversion is encouraged
- once the value is refined, callers inherit that fact

---

## Protocol State Types

<a id="protocol-state-types"></a>

Many real APIs are state machines. Sigil lets you encode that directly.

```sigil module
t Ticket={id:String}

protocol Ticket
  Open → Closed via resolve
  Open → Open via annotate
  initial = Open
  terminal = Closed

λannotate(note:String,ticket:Ticket)=>Ticket
requires ticket.state=Open
ensures result.state=Open
={id:ticket.id}

λresolve(ticket:Ticket)=>Bool
requires ticket.state=Open
ensures ticket.state=Closed
=true
```

The language tracks protocol state through the proof context. Wrong-order
operations become compile errors:

- use after close
- double close
- operations that require `Open` after a transition to `Closed`

This is especially useful for generated code because LLMs often produce valid
individual calls in the wrong sequence. Protocol types give the compiler enough
structure to reject those sequences early.

---

## Named Concurrent Regions

<a id="named-concurrent-regions"></a>

Sigil widens work through one canonical concurrency surface: named concurrent
regions.

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

The region is named. Width is explicit. Policy is explicit. The result shape is
explicit: `[ConcurrentOutcome[T,E]]`, preserving input order.

There is no alternative concurrency surface to choose among here. Not
`Promise.all`, not goroutines, not callback fan-out, not hidden parallel map.

For agents, that means:

- parallel work has one shape
- concurrency policy is visible in syntax
- result handling is stable across the codebase

---

## Semantic Review

<a id="semantic-review"></a>

`git diff` tells you what lines changed. `sigil review` tells you what those
changes mean at the declaration level.

If a function gained `!Http`, the review says so. If a `requires` clause
changed, the review says so. If a diff changed implementation only, without a
surface-level signature/effect/contract change, the review says that too.

Typical usage:

```bash
sigil review
sigil review --staged
sigil review --base HEAD~1
sigil review --base main --head feature-branch
sigil review --llm --staged
```

`--llm` wraps the same structured facts in a prompt preamble that tells the
model to stay grounded in the listed data.

This is a very strong agent feature because it narrows review work to semantic
changes rather than line noise.

---

## JSON-First CLI

<a id="json-first-cli"></a>

Sigil's compiler and inspection surfaces are designed around machine-readable
output.

The important default is not "there exists a `--json` flag somewhere". The
important default is that structured compiler/introspection output is a first-
class design target.

Commands like these emit stable JSON envelopes:

- `sigil compile`
- `sigil test`
- `sigil validate`
- `sigil inspect validate`
- `sigil inspect types`
- `sigil inspect proof`
- `sigil inspect world`
- `sigil inspect codegen`
- `sigil docs ...`
- `sigil featureFlag audit`

Two notable exceptions have special human defaults:

- `sigil run` passes program output through by default, with `--json` available
- `sigil review` defaults to a human-readable summary, with `--json` and
  `--llm` available

The envelope is stable and versioned with `formatVersion`.

For an agent loop, that means:

- no regex scraping of diagnostic text
- stable error codes
- explicit `ok` / `phase` / `data`
- easier orchestration of compile -> inspect -> repair loops

---

## Embedded Docs for Cold Starts

<a id="embedded-docs-for-cold-starts"></a>

Sigil is new. A model cannot rely on pretraining familiarity the way it can for
Python or TypeScript.

So the binary ships an embedded local docs corpus.

```bash
sigil docs list
sigil docs search "feature flags"
sigil docs context --list
sigil docs context syntax
sigil docs show docs/syntax-reference --start-line 1 --end-line 100
```

Those commands return JSON. The installed tool can teach the installed syntax,
stdlib surface, and reference semantics without a web lookup.

That matters because "what does the language look like?" is part of the
compiler loop for a new language. Sigil makes that loop local and versioned with
the binary rather than punting it to external docs drift.

---

## Appendix: Current SIGIL-CANON Families

<a id="appendix-current-sigil-canon-families"></a>

This appendix reflects the current implementation-exported `SIGIL-CANON-*`
families rather than an older hand-maintained subset.

**Duplicate declarations**

- `SIGIL-CANON-DUPLICATE-TYPE`
- `SIGIL-CANON-DUPLICATE-EXTERN`
- `SIGIL-CANON-DUPLICATE-IMPORT`
- `SIGIL-CANON-DUPLICATE-CONST`
- `SIGIL-CANON-DUPLICATE-FUNCTION`
- `SIGIL-CANON-DUPLICATE-TEST`

**File formatting and source form**

- `SIGIL-CANON-EOF-NEWLINE`
- `SIGIL-CANON-TRAILING-WHITESPACE`
- `SIGIL-CANON-BLANK-LINES`
- `SIGIL-CANON-SOURCE-FORM`
- `SIGIL-CANON-DELIMITER-SPACING`
- `SIGIL-CANON-OPERATOR-SPACING`
- `SIGIL-CANON-MATCH-LAYOUT`
- `SIGIL-CANON-MATCH-ARM-LAYOUT`
- `SIGIL-CANON-REDUNDANT-PARENS`
- `SIGIL-CANON-MATCH-BODY-BLOCK`

**File purpose**

- `SIGIL-CANON-LIB-NO-MAIN`
- `SIGIL-CANON-EXEC-NEEDS-MAIN`
- `SIGIL-CANON-TEST-NEEDS-MAIN`
- `SIGIL-CANON-TEST-LOCATION`
- `SIGIL-CANON-TEST-PATH`

**Naming and path shape**

- `SIGIL-CANON-FILENAME-CASE`
- `SIGIL-CANON-FILENAME-INVALID-CHAR`
- `SIGIL-CANON-FILENAME-FORMAT`
- `SIGIL-CANON-IDENTIFIER-FORM`
- `SIGIL-CANON-TYPE-NAME-FORM`
- `SIGIL-CANON-CONSTRUCTOR-NAME-FORM`
- `SIGIL-CANON-TYPE-VAR-FORM`
- `SIGIL-CANON-RECORD-FIELD-FORM`
- `SIGIL-CANON-MODULE-PATH-FORM`
- `SIGIL-CANON-RECORD-EXACTNESS`

**Recursion and helper surfaces**

- `SIGIL-CANON-RECURSION-ACCUMULATOR`
- `SIGIL-CANON-RECURSION-COLLECTION-NONSTRUCTURAL`
- `SIGIL-CANON-RECURSION-CPS`
- `SIGIL-CANON-RECURSION-APPEND-RESULT`
- `SIGIL-CANON-RECURSION-ALL-CLONE`
- `SIGIL-CANON-RECURSION-ANY-CLONE`
- `SIGIL-CANON-RECURSION-FILTER-CLONE`
- `SIGIL-CANON-RECURSION-FIND-CLONE`
- `SIGIL-CANON-RECURSION-FLATMAP-CLONE`
- `SIGIL-CANON-RECURSION-FOLD-CLONE`
- `SIGIL-CANON-RECURSION-MAP-CLONE`
- `SIGIL-CANON-RECURSION-REVERSE-CLONE`
- `SIGIL-CANON-BRANCHING-SELF-RECURSION`
- `SIGIL-CANON-TRAVERSAL-FILTER-COUNT`
- `SIGIL-CANON-HELPER-DIRECT-WRAPPER`
- `SIGIL-CANON-RECURSION-MISSING-DECREASES`
- `SIGIL-CANON-MUTUAL-RECURSION`
- `SIGIL-CANON-ORDINARY-DECREASES`

**Ordering and local scope**

- `SIGIL-CANON-PARAM-ORDER`
- `SIGIL-CANON-EFFECT-ORDER`
- `SIGIL-CANON-RECORD-TYPE-FIELD-ORDER`
- `SIGIL-CANON-RECORD-LITERAL-FIELD-ORDER`
- `SIGIL-CANON-RECORD-PATTERN-FIELD-ORDER`
- `SIGIL-CANON-NO-SHADOWING`

**Bindings and reachability**

- `SIGIL-CANON-LET-UNTYPED`
- `SIGIL-CANON-SINGLE-USE-PURE-BINDING`
- `SIGIL-CANON-DEAD-PURE-DISCARD`
- `SIGIL-CANON-UNUSED-IMPORT`
- `SIGIL-CANON-UNUSED-EXTERN`
- `SIGIL-CANON-UNUSED-BINDING`
- `SIGIL-CANON-UNUSED-DECLARATION`

**Declaration ordering**

- `SIGIL-CANON-DECL-CATEGORY-ORDER`
- `SIGIL-CANON-DECL-EXPORT-ORDER`
- `SIGIL-CANON-DECL-ALPHABETICAL`

Current declaration category order in the implementation docs is:

- types
- externs
- consts
- functions
- tests

**Other canonical surfaces**

- `SIGIL-CANON-EXTERN-MEMBER-ORDER`
- `SIGIL-CANON-FEATURE-FLAG-DECL`

---

## The Combinatorial Effect

Each Sigil rule removes one dimension of uncertainty:

- one branching surface instead of several
- one canonical print form instead of style freedom
- explicit effects instead of hidden side effects
- fixed project files instead of ambiguous placement
- named boundaries instead of raw endpoints
- labelled boundary policy instead of convention-only data handling
- direct-only package resolution instead of transitive guesswork

The value is not any single rule in isolation. The value is that the rules
compose. Every time the model asks "which valid way should I choose?", Sigil
tries to replace that question with "there is one accepted way; here it is".

That is why Sigil is unusually well-shaped for coding agents. It does not ask
the model to be stylistically disciplined. It moves discipline into the language
surface and the compiler.
