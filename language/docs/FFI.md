# Sigil FFI (Foreign Function Interface)

## Overview

Sigil can call external modules (including TypeScript/JavaScript packages) using `e` (extern) declarations.

## Syntax

```sigil
e module⋅path
```

That's it. Exactly ONE way to do FFI (canonical form).

## Examples

### Console Output

```sigil
e console

λmain()→𝕌=console.log("Hello from Sigil!")
```

### Node.js Built-ins

```sigil
e fs⋅promises

λwriteFile(path:𝕊,content:𝕊)→𝕌=fs⋅promises.writeFile(path,content)

λmain()→𝕌=writeFile("output.txt","Hello, Sigil!")
```

### NPM Packages

First install the package:
```bash
npm install axios
```

Then use it:
```sigil
e axios

λfetchUser(id:ℤ)→𝕌=axios.get("https://api.example.com/users/" + id)

λmain()→𝕌=fetchUser(123)
```

## How It Works

### 1. Declaration

```sigil
e module⋅path
```

Declares that you'll use an external module.

### 2. Usage

```sigil
module⋅path.member(args)
```

Access members using full namespace path + dot + member name.

### 3. Validation

The compiler validates externals at **link-time**:
- Loads the module (requires `npm install` first)
- Checks if accessed members exist
- Fails BEFORE writing generated output if member not found

This catches typos WITHOUT needing type annotations!

### 4. Code Generation

```sigil
e fs⋅promises
λmain()→𝕌=fs⋅promises.readFile("file.txt","utf-8")
```

Compiles to:

```ts
import * as fs_promises from 'fs/promises';

export async function main() {
  return await __sigil_call("extern:fs/promises.readFile",
    fs_promises.readFile, ["file.txt", "utf-8"]);
}
```

## Namespace Rules

- Full path becomes namespace: `e fs⋅promises` → use as `fs⋅promises.readFile`
- No conflicts possible: `moduleA/utils` and `moduleB/utils` are different namespaces
- Slash visible in Sigil source (machines don't care about syntax aesthetics)
- Converted to underscores in generated TypeScript: `fs_promises.readFile`

## Validation Examples

### ✅ Works - Correct member

```sigil
e console
λmain()→𝕌=console.log("works!")
```

### ❌ Fails - Typo in member

```sigil
e console
λmain()→𝕌=console.logg("typo!")
```

```
Error: Member 'logg' does not exist on module 'console'
Available members: log, error, warn, info, debug, ...
Check for typos or see module documentation.
```

### ❌ Fails - Module not installed

```sigil
e axios
λmain()→𝕌=axios.get("url")
```

```
Error: Cannot load external module 'axios':
  Cannot find module 'axios'
Make sure it's installed: npm install axios
```

## Type System Integration

Sigil supports both **untyped** and **typed** FFI declarations.

### Untyped FFI (Trust Mode)

```sigil
e console
e fs⋅promises
```

Uses `any` type for FFI calls. Member validation is **structural** (does it exist?) not type-based.
This trust-mode `any` is an internal compiler escape hatch for untyped externs, not a
general-purpose surface type you should write in Sigil source.

### Typed FFI (Type-Safe Mode)

You can optionally provide type signatures for extern members:

```sigil
t MkdirOptions = { recursive: 𝔹 }

e fs⋅promises : {
  mkdir : λ(𝕊, MkdirOptions) → 𝕌
}

λensureDir(dir:𝕊)→𝕌={
  l opts=({recursive:true}:MkdirOptions);
  fs⋅promises.mkdir(dir, opts)
}
```

**Benefits:**
- Compile-time type checking of FFI calls
- Can reference named Sigil types in FFI signatures
- Better IDE/LSP support
- Self-documenting external APIs

Typed FFI relies on the same canonical structural equality rule used throughout the
checker: aliases and named product types normalize before compatibility checks.
That means `MkdirOptions` and `{recursive:𝔹}` are treated as the same explicit type
meaning when validating the `mkdir` call. This is canonical semantic comparison, not
type inference.

When modeling JavaScript data:
- fixed-shape objects should use records like `{recursive:𝔹}`
- dynamic dictionaries should use core maps like `{𝕊↦𝕊}`

Example: HTTP headers are maps, not records.

**Syntax:**
```sigil
e module⋅path : {
  member1 : λ(ParamType1, ParamType2) → ReturnType,
  member2 : λ(ParamType3) → ReturnType
}
```

### Declaration Ordering Requirement

**IMPORTANT:** Because typed extern declarations can reference named types, **types must be declared before externs** in Sigil's canonical ordering:

```sigil
✅ VALID: Type before extern
t MkdirOptions = { recursive: 𝔹 }
e fs⋅promises : { mkdir : λ(𝕊, MkdirOptions) → 𝕌 }

❌ INVALID: Extern before type (compiler error)
e fs⋅promises : { mkdir : λ(𝕊, MkdirOptions) → 𝕌 }
t MkdirOptions = { recursive: 𝔹 }
```

This is why Sigil's canonical declaration ordering is: **`t → e → i → c → λ → test`**

See [Canonical Declaration Ordering](/articles/canonical-declaration-ordering) for more details.

## Concurrent Behavior

Sigil uses one promise-shaped runtime model for FFI too. Promise-returning FFI calls are started automatically and joined only when a strict consumer needs their values:

```sigil
e fs⋅promises

λread_file(path:𝕊)→!IO 𝕊=fs⋅promises.readFile(path,"utf8")

λmain()→!IO 𝕊=read_file("data.txt")
```

Compiles to:

```typescript
import * as fs_promises from 'fs/promises';

function read_file(path) {
  return __sigil_call("extern:fs/promises.readFile",
    fs_promises.readFile, [path, "utf8"]);
}

export function main() {
  return read_file("data.txt");
}
```

**No Promise wrapping needed** - it just works. The compiler keeps FFI results pending until something strict needs them.

See [ASYNC.md](./ASYNC.md) for the full concurrentByDefault model.

## Canonical Form

FFI has exactly **TWO syntactic forms**:

✅ ONLY: `e module⋅path` (untyped)
✅ ONLY: `e module⋅path : { member : λ(...) → ... }` (typed)
❌ NO: `extern module⋅path` (no full keyword)
❌ NO: `e module⋅path as alias` (no aliasing)
❌ NO: `e module⋅path{member1,member2}` (no destructuring)

This ensures deterministic, unambiguous code generation for LLMs.

## Limitations

### No Direct Object Construction

```sigil
❌ Cannot: new Date()
❌ Cannot: new RegExp(pattern)
```

Must use factory functions or FFI wrappers.

### No Method Chaining (Yet)

```sigil
❌ Cannot: axios.get(url).then(fn)
```

Each FFI call is a single member access.

Future: Expression-level member access.

### No Class Interop (Yet)

```sigil
❌ Cannot: class instances
❌ Cannot: this binding
```

Use functional APIs or wrapper functions.

## Best Practices

### 1. Wrap FFI in Sigil Functions

```sigil
e console

λlog(msg:𝕊)→𝕌=console.log(msg)
λerror(msg:𝕊)→𝕌=console.error(msg)

λmain()→𝕌={
  log("Info message")
  error("Error message")
}
```

### 2. Use Semantic Names

```sigil
e fs⋅promises

λreadFile(path:𝕊)→𝕌=fs⋅promises.readFile(path,"utf-8")
λwriteFile(path:𝕊,content:𝕊)→𝕌=fs⋅promises.writeFile(path,content)
```

### 3. Validate at Boundaries

Use contracts (future feature) to validate FFI inputs/outputs.

### 4. React and Browser Apps (Bridge Pattern)

Recommended frontend integration:

- Put deterministic domain policy in Sigil (`.sigil`)
- Compile Sigil to generated TypeScript (`.ts`)
- Use a separate `bridge.ts` / `bridge.tsx` for React hooks, JSX, browser events, and localStorage

Why keep a separate bridge?

- Linting/prettier/typechecking work normally
- React stays idiomatic
- Sigil stays canonical and machine-first
- UI/runtime glue is isolated from core logic

## Future Extensions

- Type annotations for FFI declarations
- Method chaining syntax
- Class/object interop
- Callback conversions (JS → Sigil functions)

---

**FFI unlocks the TypeScript/JavaScript ecosystem for Sigil programs!** 🚀
