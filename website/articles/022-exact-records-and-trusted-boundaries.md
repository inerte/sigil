---
title: Exact Records and Trusted Boundaries
date: 2026-03-07
author: Sigil Language Team
slug: exact-records-and-trusted-boundaries
---

# Exact Records and Trusted Boundaries

This change started from a very ordinary bug pattern in mainstream app code:

```ts
if (!msg.createdAt) {
  ...
}
```

Sometimes that check is correct.
Sometimes it is cargo-cult defensive coding against a state the schema and types
already ruled out.

Sigil wants to make that distinction mechanically obvious.

## Practical Rule First

If internal business logic has a value like:

```sigil
t Message={createdAt:stdlib⋅time.Instant,text:𝕊}
```

then `Message` means:
- it has exactly `createdAt` and `text`
- both fields are present
- there are no hidden extra fields
- there are no hidden missing fields

So once code has a `Message`, it should just use `message.createdAt`.

If `createdAt` might actually be absent, Sigil wants that fact in the type:

```sigil
t MaybeMessage={createdAt:Option[stdlib⋅time.Instant],text:𝕊}
```

That is the practical rule:
- exact internal records for trusted values
- `Option[T]` for actual absence
- no fuzzy “object probably has this field” semantics

## The Boundary Story

The right place for uncertainty is the boundary.

Raw JSON is uncertain:

```json
{"createdAt":"2026-03-07T00:00:00.000Z","text":"hello"}
```

Internal business logic should not keep carrying that raw shape around.
The canonical Sigil flow is:

```text
raw JSON text
→ stdlib⋅json.parse
→ stdlib⋅decode.parse / run
→ trusted internal record
```

Example:

```sigil
i stdlib⋅decode
i stdlib⋅json
i stdlib⋅time

t Message={createdAt:stdlib⋅time.Instant,text:𝕊}

λinstant(value:stdlib⋅json.JsonValue)→Result[stdlib⋅time.Instant,stdlib⋅decode.DecodeError] match stdlib⋅decode.string(value){
  Ok(text)→
    match stdlib⋅time.parseIso(text){
      Ok(instant)→Ok(instant)|
      Err(error)→Err({message:error.message,path:[]})
    }|
  Err(error)→Err(error)
}

λmessage(value:stdlib⋅json.JsonValue)→Result[Message,stdlib⋅decode.DecodeError] match stdlib⋅decode.field(instant,"createdAt")(value){
  Ok(createdAt)→
    match stdlib⋅decode.field(stdlib⋅decode.string,"text")(value){
      Ok(text)→Ok({createdAt:createdAt,text:text})|
      Err(error)→Err(error)
    }|
  Err(error)→Err(error)
}
```

After that step, the rest of the program gets `Message`, not raw `JsonValue`.

That is the whole point:
- parse once
- validate once
- trust the result

## Validated Values Should Stop Looking Raw

Sometimes plain `𝕊` or `ℤ` is too weak for internal code.

For example:

```sigil
t Email=Email(𝕊)
t UserId=UserId(ℤ)
```

These are not aliases.
They are wrapper-backed domain values.

That means:
- raw email text is one thing
- validated `Email` is another
- raw integer is one thing
- `UserId` is another

This matters because it preserves proof.
Once code has `Email`, it should stop re-asking whether the string “looks like an email.”

## What Sigil Enforces

This is not just a style note.
The toolchain now pushes this model directly.

### Records Are Exact

Sigil records are **closed exact products**.

That means:
- missing fields are rejected
- extra fields are rejected
- record types do not width-subtype
- open records and partial records are not part of the language

If you try to write row-tail or open-record style syntax, the frontend rejects it
with `SIGIL-CANON-RECORD-EXACTNESS`.

### Uncertainty Must Be Explicit

Sigil does not want “maybe missing” hidden in object conventions.

The canonical tools are:
- `Option[T]` for optional presence
- `Result[T,E]` for failure

Not:
- ambient nullability
- optional-field bag semantics
- open-record tricks

### JSON Has Two Layers Now

- `stdlib⋅json` is the raw layer
- `stdlib⋅decode` is the trust-building layer

That split is deliberate.
It keeps “parse raw bytes” separate from “turn this into a trusted internal value.”

## The PL Version

If you prefer the more formal language:

- Sigil records are **exact closed products**
- Sigil does **not** support row polymorphism for records
- Sigil does **not** use width subtyping for records
- uncertainty is represented explicitly through sum types like `Option[T]` and `Result[T,E]`
- validated boundary values should become named internal types instead of remaining raw primitives

That design is intentionally machine-friendly.

Large models tend to over-defend when they see loose object shapes, widened types,
or ambiguous boundaries. Sigil is trying to reduce the number of plausible
interpretations the model can hold at once.

## Why This Matters

The goal is not “make defensive coding illegal.”

The goal is narrower and more useful:
- boundary uncertainty should be real and explicit
- internal trusted data should be exact
- the compiler should enforce that distinction wherever possible

When that works, business logic stops looking like it is apologizing for its own
types.
