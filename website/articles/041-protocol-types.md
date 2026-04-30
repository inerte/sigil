---
title: "Protocol Types: Compile-Time State Machine Enforcement"
date: 2026-04-23
author: Sigil Language Team
slug: protocol-types
---

# Protocol Types: Compile-Time State Machine Enforcement

`Transaction = {id:String}` is a plain record. Nothing in Sigil's type system
prevents calling `execInsertIn` on a committed transaction, calling `commit`
twice, or inserting rows after `rollback`. These are runtime errors at best,
data corruption at worst.

The same class of bug exists for WebSocket connections (`send` after `close`)
and PTY sessions (`writeManaged` before `eventsManaged`). In every case, the
protocol lives in documentation rather than types.

## The Problem

Handles — database connections, network clients, terminal sessions — are not
arbitrary values. They follow a state machine. The SQL transaction state
machine is:

```text
begin → [Open] → execInsertIn, execUpdateIn, execDeleteIn → [Open]
                → commit, rollback → [Closed]
```

A `Transaction = {id:String}` has no way to express this. The type system
sees two functions, both accepting `Transaction`, and can say nothing about
which one is valid to call at any point in time.

## The Protocol Declaration

A `protocol` declaration attaches a state machine to an existing type:

```sigil module
t Transaction={id:String}

protocol Transaction
  Open → Closed via commit, rollback
  initial = Open
  terminal = Closed

λcommit(transaction:Transaction)=>Bool
requires transaction.state=Open
ensures transaction.state=Closed
=true

λrollback(transaction:Transaction)=>Bool
requires transaction.state=Open
ensures transaction.state=Closed
=true
```

This is a complete minimal module. Real transaction APIs usually add more
`Open → Open` operations such as `execInsertIn`, `execUpdateIn`, and
`execDeleteIn`, but the core shape is already visible here.

This is a first-class declaration: it names the type it describes, lists the
valid state transitions, identifies which functions cause each transition, and
declares which state is initial and which is terminal.

State names are UpperCamelCase. Transitions are sorted by `(from, to)`. The
`via` list within each transition is alphabetical. One canonical form, enforced
by the printer.

## State in Function Contracts

Functions listed in `via` must carry matching `requires`/`ensures` state
annotations. These are the same `requires`/`ensures` clauses that exist for
value contracts — they extend naturally to state:

```sigil module
t Transaction={id:String}

protocol Transaction
  Open → Closed via commit
  Open → Open via execInsertIn
  initial = Open
  terminal = Closed

λcommit(transaction:Transaction)=>!Sql Result[
  Unit,
  §sql.SqlFailure
]
requires transaction.state=Open
ensures transaction.state=Closed
=Err({
  kind:§sql.Unsupported(),
  message:"sql intrinsic unavailable"
})

λexecInsertIn(statement:String,transaction:Transaction)=>!Sql Result[
  Int,
  §sql.SqlFailure
]
requires transaction.state=Open
ensures transaction.state=Open
=Err({
  kind:§sql.Unsupported(),
  message:"sql intrinsic unavailable"
})
```

`handle.state` is a virtual field on protocol-typed values. It only exists in
contract clauses, not in ordinary expressions. The compiler knows its type is
`Bool` (it's the left side of a state comparison), and the solver knows how to
reason about it.

## How Z3 Tracks State

Sigil already uses Z3 to prove `requires`/`ensures` clauses at call sites. The
`ensures result>1800` clause on `normalizeYear` becomes a fact in the proof
context for every caller — the compiler knows the return value is greater than
1800 without re-proving it.

Protocol state works the same way, but with a new kind of solver atom:
`StateEq { path, state_index }`. State names are encoded as integers (sorted
alphabetically), and `StateEq` is just an integer equality constraint. Z3 can
prove or refute it like any other constraint.

When a protocol-typed handle is bound inside a function:

```sigil module
t Transaction={id:String}

protocol Transaction
  Open → Closed via commit
  initial = Open
  terminal = Closed

λcommit(transaction:Transaction)=>Bool
requires transaction.state=Open
ensures transaction.state=Closed
=true

λworkflow(id:String)=>Bool={
  l tx=({id:id}:Transaction);
  tx.id=id and commit(tx)
}
```

the proof context gains `tx.state = Open` as an assumption — injected
automatically because `Transaction`'s protocol declares `initial = Open`.
After a call to `commit(tx)`, the `ensures tx.state = Closed` clause is
propagated into the proof context. Any subsequent call with
`requires tx.state = Open` fails with a counterexample.

No linear types. No move semantics. No phantom type parameters. The existing
Z3 infrastructure handles it.

## What the Compiler Catches

**Post-commit mutation.** Calling `execInsertIn` after `commit` is a type
error. The proof context knows `tx.state = Closed`; the requires clause needs
`Open`. Z3 returns a counterexample and the build fails.

**Double-commit.** Calling `commit` twice produces the same failure — after
the first `commit`, the proof context has `tx.state = Closed`, and the second
`commit` requires `Open`.

**Use-after-close.** WebSocket `send` after `close` fails for the same reason.
The `Client` protocol marks `close` as `Open → Closed`, so any subsequent
`send` (which requires `Open`) is rejected.

**Wrong-order PTY operations.** `writeManaged` requires `Open` state on a
`SessionRef`. The PTY protocol marks `eventsManaged` and `writeManaged` as
`Open → Open` transitions. Calling `writeManaged` on a newly created
`SessionRef` works; calling it after `closeManaged` (which transitions to
`Closed`) fails.

## What It Replaces

Previously, these invariants lived in documentation, runtime checks, or
convention. The SQL fixture test verified that a committed transaction doesn't
persist its changes — a runtime property. With protocol types, the incorrect
usage pattern is rejected before runtime exists.

The `Owned[T]` + `using` block pattern already handled resource scoping at
runtime. Protocol types handle the intermediate state — the sequence of
operations between creation and close — at compile time.

## The Machine-First Angle

Sigil is written by machines. The protocol declaration is a schema that a
machine can generate correctly from a state machine definition. The
`requires`/`ensures` annotations are mechanical — given a transition
`Open → Closed via commit`, the annotation on `commit` is exactly
`requires transaction.state=Open` and `ensures transaction.state=Closed`.

The solver does the hard work. The machine generates the annotations. The
compiler enforces them. No human needs to remember which state a handle is in
at any point in the program.
