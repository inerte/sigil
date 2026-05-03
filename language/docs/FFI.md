# Sigil FFI (Foreign Function Interface)

## Overview

Sigil can call external modules (including TypeScript/JavaScript packages) using `e` (extern) declarations.

## Syntax

```sigil module
e module::path
```

That's it. Exactly ONE way to do FFI (canonical form).

## Examples

### Console Output

```sigil program
e console:{log:λ(String)=>!Log Unit}

λmain()=>!Log Unit=console.log("Hello from Sigil!")
```

### Node.js Built-ins

```sigil program
e fs::promises:{writeFile:λ(String,String)=>!Fs Unit}

λmain()=>!Fs Unit=writeFile(
  "output.txt",
  "Hello, Sigil!"
)

λwriteFile(content:String,path:String)=>!Fs Unit=fs::promises.writeFile(
  path,
  content
)
```

### NPM Packages

First install the package:
```bash
npm install axios
```

Then use it:
```sigil program
e axios:{get:λ(String)=>!Http String}

λfetchUser(id:Int)=>!Http String=axios.get("https://api.example.com/users/"+id)

λmain()=>!Http String=fetchUser(123)
```

### Project-Local Bridges

Project-local JS/TS bridge modules use the reserved `bridge::...` namespace.

```sigil program
e bridge::subscriptionProbe:{tick: subscribes λ()=>String}

λmain()=>!Stream String={
  using source=bridge::subscriptionProbe.tick(){
    match §stream.next(source){
      §stream.Item(text)=>text|
      §stream.Done()=>"done"
    }
  }
}
```

`bridge::foo` resolves to `bridges/foo.(js|mjs)` relative to the owning Sigil
project root. Nested bridge paths map the same way:

- `e bridge::ptyAdapter` => `bridges/ptyAdapter.(js|mjs)`
- `e bridge::ws::client` => `bridges/ws/client.(js|mjs)`

This is the canonical project-local boundary surface for app-owned foreign code.
Do not use the project package name as an extern namespace.

## How It Works

### 1. Declaration

```sigil module
e module::path
```

Declares that you'll use an external module.

### 2. Usage

```sigil expr
module::path.member(args)
```

Access members using full namespace path + dot + member name.

### 3. Validation

The compiler validates externals at **link-time**:
- Loads the module (requires `npm install` first)
- Checks if accessed members exist
- Fails BEFORE writing generated output if member not found

This catches typos WITHOUT needing type annotations!

### 4. Code Generation

```sigil program
e fs::promises

λmain()=>Unit=fs::promises.readFile(
  "file.txt",
  "utf-8"
)
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

- Full path becomes namespace: `e fs::promises` => use as `fs::promises.readFile`
- No conflicts possible: `moduleA/utils` and `moduleB/utils` are different namespaces
- Slash visible in Sigil source (machines don't care about syntax aesthetics)
- Converted to underscores in generated TypeScript: `fs_promises.readFile`

## Validation Examples

### ✅ Works - Correct member

```sigil program
e console:{log:λ(String)=>!Log Unit}

λmain()=>!Log Unit=console.log("works!")
```

### ❌ Fails - Typo in member

```text
e console:{log:λ(String)=>!Log Unit}

λmain()=>!Log Unit=console.logg("typo!")
```

```
Error: Member 'logg' does not exist on module 'console'
Available members: log, error, warn, info, debug, ...
Check for typos or see module documentation.
```

### ❌ Fails - Module not installed

```text
e axios:{get:λ(String)=>!Http String}

λmain()=>!Http String=axios.get("url")
```

```
Error: Cannot load external module 'axios':
  Cannot find module 'axios'
Make sure it's installed: npm install axios
```

## Type System Integration

Sigil supports both **untyped** and **typed** FFI declarations.

### Untyped FFI (Trust Mode)

```sigil module
e console

e fs::promises
```

Uses `any` type for FFI calls. Member validation is **structural** (does it exist?) not type-based.
This trust-mode `any` is an internal compiler escape hatch for untyped externs, not a
general-purpose surface type you should write in Sigil source.
Effectful wrappers should prefer typed extern members. Under exact effect
checking, a non-stdlib untyped extern call does not justify a declared effect
because the compiler only knows the member exists, not what effects it may
perform. Some internal stdlib shims are compiler-known and may still attach
effects during checking, but that fallback is an implementation detail rather
than a general FFI contract.

### Typed FFI (Type-Safe Mode)

You can optionally provide type signatures for extern members:

```sigil module
t MkdirOptions={recursive:Bool}

e fs::promises:{mkdir:λ(String,MkdirOptions)=>!Fs Unit}

λensureDir(dir:String)=>!Fs Unit=fs::promises.mkdir(
  dir,
  ({recursive:true}:MkdirOptions)
)
```

**Benefits:**
- Compile-time type checking of FFI calls
- Can reference named Sigil types in FFI signatures
- Better IDE/LSP support
- Self-documenting external APIs

Typed FFI relies on the same canonical structural equality rule used throughout the
checker: unconstrained aliases and unconstrained named product types normalize before
compatibility checks. That means `MkdirOptions` and `{recursive:Bool}` are treated as the
same explicit type meaning when validating the `mkdir` call. Constrained user-defined
types use refinement checking over their underlying type instead of plain structural
equality. This is canonical semantic comparison, not type inference.

### Foreign Subscriptions

Typed extern members may also declare foreign event subscriptions:

```sigil module
e nodePty:{onData: subscribes λ(Session)=>String,onExit: subscribes λ(Session)=>ExitEvent}
```

`subscribes λ(A...)=>T` is the canonical typed-extern surface for foreign
callback/listener ingress.

It does **not** expose raw JavaScript callbacks to Sigil source. Instead, the
member elaborates as if it had type:

```text
λ(A...)=>!Stream Owned[§stream.Source[T]]
```

This gives Sigil one canonical model for host events:
- foreign adapters stay in JS/TS
- Sigil receives owned `§stream.Source[...]` handles
- callers use those handles with `using`
- in projects, local adapters should be declared under `bridge::...`

Nullary foreign callbacks map to `Unit`.

In project code, named user-defined types live in `src/types.lib.sigil` and are
referenced elsewhere through `µ...`. The local same-file `t MkdirOptions=...`
form shown here is still valid for standalone non-project snippets.

When modeling JavaScript data:
- fixed-shape objects should use records like `{recursive:Bool}`
- dynamic dictionaries should use core maps like `{String↦String}`

Example: HTTP headers are maps, not records.

**Syntax:**
```text
e module::path : {
  member1 : λ(ParamType1, ParamType2) => ReturnType,
  member2 : λ(ParamType3) => ReturnType
}
```

### Declaration Ordering Requirement

**IMPORTANT:** Because typed extern declarations can reference named types, **types must be declared before externs** in Sigil's canonical ordering:

```sigil module
t MkdirOptions={recursive:Bool}

e fs::promises:{mkdir:λ(String,MkdirOptions)=>Unit}
```

```sigil invalid-module
e fs::promises:{mkdir:λ(String,MkdirOptions)=>Unit}
t MkdirOptions={recursive:Bool}
```

This is why Sigil's canonical declaration ordering is: **`t => e => c => λ => test`**

See [Canonical Declaration Ordering](/articles/canonical-declaration-ordering) for more details.

## Concurrent Behavior

Sigil uses one promise-shaped runtime model for FFI too. Promise-returning FFI calls are started automatically and joined only when a strict consumer needs their values:

```sigil program
e fs::promises:{readFile:λ(String,String)=>!Fs String}

λmain()=>!Fs String=readFile("data.txt")

λreadFile(path:String)=>!Fs String=fs::promises.readFile(
  path,
  "utf8"
)
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

See [ASYNC.md](./ASYNC.md) for the full async runtime and concurrent-region model.

## Canonical Form

FFI has exactly **TWO declaration forms**:

✅ ONLY: `e module::path` (untyped)
✅ ONLY: `e module::path : { member : memberType }` (typed)
❌ NO: `extern module::path` (no full keyword)
❌ NO: `e module::path as alias` (no aliasing)
❌ NO: `e module::path{member1,member2}` (no destructuring)

Within typed extern member lists, the canonical member type forms are:

- `λ(...)=>...` for ordinary foreign calls
- `subscribes λ(...)=>...` for foreign subscription ingress

This ensures deterministic, unambiguous code generation for LLMs.

## Limitations

### No Direct Object Construction

```text
❌ Cannot: new Date()
❌ Cannot: new RegExp(pattern)
```

Must use factory functions or FFI wrappers.

### No Method Chaining (Yet)

```sigil invalid-expr
❌ Cannot: axios.get(url).then(fn)
```

Each FFI call is a single member access.

Future: Expression-level member access.

### No Class Interop (Yet)

```text
❌ Cannot: class instances
❌ Cannot: this binding
```

Use functional APIs or wrapper functions.

## Best Practices

### 1. Wrap FFI in Sigil Functions

```sigil program
e console:{log:λ(String)=>!Log Unit}

λlog(msg:String)=>!Log Unit=console.log(msg)

λmain()=>!Log Unit=log("Info message")
```

### 2. Use Semantic Names

```sigil module
e fs::promises:{readFile:λ(String,String)=>!Fs String,writeFile:λ(String,String)=>!Fs Unit}

λreadFile(path:String)=>!Fs String=fs::promises.readFile(
  path,
  "utf-8"
)

λwriteFile(content:String,path:String)=>!Fs Unit=fs::promises.writeFile(
  path,
  content
)
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

- richer extern validation and adapters
- Method chaining syntax
- Class/object interop
- Callback conversions (JS => Sigil functions)

---

FFI unlocks the TypeScript/JavaScript ecosystem for Sigil programs.
