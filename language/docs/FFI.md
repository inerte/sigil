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

Currently uses `any` type for FFI calls (trust mode).

Member validation is **structural** (does it exist?) not type-based.

Future: Optional type declarations for better safety.

## Async Behavior

**ALL Sigil functions are async**, including FFI calls. This means Promise-returning FFI calls are automatically awaited:

```sigil
e fs⋅promises

λread_file(path:𝕊)→!IO 𝕊=fs⋅promises.readFile(path,"utf8")

λmain()→!IO 𝕊=read_file("data.txt")
```

Compiles to:

```typescript
import * as fs_promises from 'fs/promises';

async function read_file(path) {
  return await __sigil_call("extern:fs/promises.readFile",
    fs_promises.readFile, [path, "utf8"]);
}

export async function main() {
  return await read_file("data.txt");
}
```

**No Promise wrapping needed** - it just works! The `await` is added automatically by the compiler.

See [ASYNC.md](./ASYNC.md) for complete details on Sigil's async-by-default design.

## Canonical Form

FFI has exactly **ONE syntactic form**:

✅ ONLY: `e module⋅path`
❌ NO: `extern module⋅path` (no full keyword)
❌ NO: `e module⋅path as alias` (no aliasing)
❌ NO: `e module⋅path{member1,member2}` (no member lists)
❌ NO: Type annotations on extern declarations

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
