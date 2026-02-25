---
title: "Machine-First JSON Diagnostics: Making Errors Actionable for AI"
date: February 25, 2026
author: Sigil Language Team
slug: 006-machine-first-json-diagnostics
---

# Machine-First JSON Diagnostics: Making Errors Actionable for AI

**TL;DR:** Sigil CLI commands (`lex`, `parse`, `compile`, `run`, `test`) now output structured JSON by default. The `--human` flag renders human-readable output from those JSON payloads. We also built a typed diagnostics system with specific error codes, location data, and fixits. This shift makes the compiler a reliable machine API for AI coding agents.

## The Problem: Prose Errors Hurt Automation

When a compiler emits errors as prose, it optimizes for human readers:

```
Error: unexpected token
  at line 42
```

This is fine for humans debugging interactively. But for **AI coding agents**, prose errors create friction:

1. **Parsing uncertainty** - Is the file path before or after the error message? What format?
2. **No stable codes** - Generic "unexpected token" could mean dozens of different issues
3. **No actionable fixits** - Agent must guess the correction
4. **Command-specific formats** - Each CLI command returns data differently

When Claude Code runs a compile command, it needs to:
- Parse the result reliably
- Identify the exact error category
- Extract location data for precise edits
- Apply suggested fixes when available

Prose output forces the agent to guess. Structured output eliminates guessing.

## The Solution: JSON-First CLI Output

All Sigil CLI commands now default to JSON output:

```bash
$ sigilc compile example.sigil
{"formatVersion":1,"command":"sigilc compile","ok":true,"phase":"codegen","data":{...}}

$ sigilc parse broken.sigil
{"formatVersion":1,"command":"sigilc parse","ok":false,"phase":"parser","error":{...}}
```

Every command returns a **CommandEnvelope**:

```typescript
type CommandEnvelope<TData = unknown> = {
  formatVersion: 1;
  command: string;           // e.g., "sigilc compile"
  ok: boolean;               // success/failure
  phase?: SigilPhase;        // which compiler phase (lexer/parser/etc)
  data?: TData;              // success payload
  error?: Diagnostic;        // failure details
};
```

The `--human` flag renders a human-readable view **derived from the JSON**:

```bash
$ sigilc parse broken.sigil --human
SIGIL-PARSE-LOCAL-BINDING broken.sigil:12:3 invalid local binding keyword (found "let", expected "l")
```

This design follows Sigil's **"no optionality" principle**: ONE canonical output format (JSON), with an opt-in human view.

## Why This Matters for AI Agents

Claude Code's workflow is:

1. Run command (e.g., `sigilc compile`)
2. Parse result
3. If error, extract location + diagnostic code
4. Apply fix (patch source, retry)
5. Loop until success

### Before: Prose Output

```
Parse Error: unexpected token 'let' at line 12
Expected local binding syntax
```

Agent must:
- Parse prose to extract line number
- Guess what "unexpected token" means
- Infer the correct fix from vague hint
- Hope file path is available somewhere

**Result:** Slow convergence, lots of trial-and-error.

### After: Structured Diagnostics

```json
{
  "ok": false,
  "phase": "parser",
  "error": {
    "code": "SIGIL-PARSE-LOCAL-BINDING",
    "message": "invalid local binding keyword",
    "location": {
      "file": "broken.sigil",
      "start": {"line": 12, "column": 3}
    },
    "found": "let",
    "expected": "l",
    "fixits": [{
      "kind": "replace",
      "range": {"file": "broken.sigil", "start": {"line": 12, "column": 3}},
      "text": "l"
    }]
  }
}
```

Agent:
1. Sees `SIGIL-PARSE-LOCAL-BINDING` code
2. Extracts exact location (`broken.sigil:12:3`)
3. Reads fixit: replace `let` with `l`
4. Applies patch automatically

**Result:** Fast convergence, deterministic recovery.

## The Typed Diagnostics System

We built a diagnostics infrastructure in `language/compiler/src/diagnostics/`:

### Core Types (`types.ts`)

```typescript
export type Diagnostic = {
  code: string;              // e.g., "SIGIL-PARSE-LOCAL-BINDING"
  phase: SigilPhase;         // lexer/parser/canonical/typecheck/etc
  message: string;           // human-readable summary
  location?: SourceSpan;     // file + line/column
  found?: unknown;           // what was encountered
  expected?: unknown;        // what was expected
  details?: Record<string, unknown>;  // extra context
  fixits?: Fixit[];          // suggested corrections
  suggestions?: Suggestion[]; // machine-readable guidance
};

export type Fixit = {
  kind: 'replace' | 'insert' | 'delete';
  range: SourceSpan;
  text?: string;
};

export type Suggestion =
  | { kind: 'replace_symbol'; message: string; replacement: string; target?: string; }
  | { kind: 'export_member'; message: string; targetFile?: string; member?: string; }
  | { kind: 'use_operator'; message: string; operator: string; replaces?: string; }
  | { kind: 'reorder_declaration'; message: string; category?: string; name?: string; before?: string; }
  | { kind: 'generic'; message: string; action?: string; };
```

### Error Wrapper (`error.ts`)

```typescript
export class SigilDiagnosticError extends Error {
  constructor(public readonly diagnostic: Diagnostic) {
    super(diagnostic.message);
    this.name = 'SigilDiagnosticError';
  }
}
```

All compiler phases throw `SigilDiagnosticError` with structured diagnostics. The CLI catches these and embeds them in the JSON envelope.

### Helper Functions (`helpers.ts`)

```typescript
export function diagnostic(
  code: string,
  phase: Diagnostic['phase'],
  message: string,
  extras: Omit<Diagnostic, 'code' | 'phase' | 'message'> = {}
): Diagnostic {
  return { code, phase, message, ...extras };
}

export function replaceTokenFixit(file: string, token: Token, text: string): Fixit {
  return { kind: 'replace', range: tokenToSpan(file, token), text };
}

export function suggestReplaceSymbol(
  message: string,
  replacement: string,
  target?: 'namespace_separator' | 'local_binding_keyword'
): Suggestion {
  return { kind: 'replace_symbol', message, replacement, target };
}

export function suggestExportMember(message: string, member?: string, targetFile?: string): Suggestion {
  return { kind: 'export_member', message, member, targetFile };
}

export function suggestReorderDeclaration(
  message: string,
  category?: string,
  name?: string,
  before?: string
): Suggestion {
  return { kind: 'reorder_declaration', message, category, name, before };
}
```

These helpers make it easy to create well-formed diagnostics throughout the compiler.

## Formal Schema: The Machine Contract

The JSON envelope format isn't just documented in prose. It's defined by a **formal JSON Schema**:

- **Schema file:** `language/spec/cli-json.schema.json`
- **Companion spec:** `language/spec/cli-json.md`

The schema is the **canonical definition** of the CLI output contract. It covers:

- All commands (`sigilc`, `lex`, `parse`, `compile`, `run`, `test`)
- The envelope structure (`formatVersion`, `command`, `ok`, `phase`, `data`, `error`)
- All `$defs` for reusable types:
  - `diagnostic` (code, phase, message, location, fixits, suggestions)
  - `fixit` (replace/insert/delete with source span)
  - `suggestion` (typed variants for different guidance patterns)
  - `sourceSpan` and `sourcePoint` (location data)
  - Per-command payloads (`lexData`, `parseData`, `compileData`, etc.)

### formatVersion Policy

The schema includes a `formatVersion` field (currently `1`) to enable evolution:

- **Backward-incompatible changes** (removing fields, changing types) require incrementing `formatVersion`
- **Backward-compatible changes** (adding optional fields) can keep the same version
- Consumers should branch on `formatVersion` to handle different output formats

This ensures the compiler can evolve its output format without breaking existing tooling.

### Why a Formal Schema Matters

**Before (prose-only docs):**
- Tool authors guess the structure from examples
- Edge cases (optional fields, error conditions) are unclear
- Changes to output format break tools silently
- Validation is manual and error-prone

**After (JSON Schema):**
- Tools validate output programmatically
- Edge cases are explicit in the schema
- Breaking changes are caught at schema-validation time
- TypeScript types can be auto-generated from the schema

This shift makes the compiler output a **testable, versioned contract** instead of informal documentation.

## Beyond Fixits: Suggestions for Complex Recovery

Fixits are great for **deterministic text edits** where the compiler knows exactly how to fix the problem:

```json
{
  "fixits": [{
    "kind": "replace",
    "range": {"file": "demo.sigil", "start": {"line": 5, "column": 3}},
    "text": "l"
  }]
}
```

But not all errors have a single, obvious fix. What about:

- **Canonical ordering violations** - The compiler knows declarations are out of order, but reordering requires understanding declaration boundaries, categories, and alphabetical ordering
- **Type errors** - Missing exports, wrong types, namespace issues - the fix might require changes to a different file
- **Semantic guidance** - Using the wrong operator, violating a language invariant - the fix is conceptual, not textual

For these cases, we added **suggestions**: machine-readable guidance that helps AI agents understand recovery strategies without providing exact text edits.

### The Suggestion Type

```typescript
export type Suggestion =
  | {
      kind: 'replace_symbol';
      message: string;
      replacement: string;
      target?: 'namespace_separator' | 'local_binding_keyword';
    }
  | {
      kind: 'export_member';
      message: string;
      targetFile?: string;
      member?: string;
    }
  | {
      kind: 'use_operator';
      message: string;
      operator: string;
      replaces?: string;
    }
  | {
      kind: 'reorder_declaration';
      message: string;
      category?: string;
      name?: string;
      before?: string;
    }
  | {
      kind: 'generic';
      message: string;
      action?: string;
    };
```

Each variant has a stable `kind` discriminator and structured fields for machine consumption.

### When to Use Fixits vs Suggestions

**Fixits:** Deterministic, safe, single correct answer

- Replacing `let` with `l` - always safe
- Replacing `/` with `⋅` in namespace separator - always correct
- Inserting missing semicolons (if required) - deterministic

**Suggestions:** Guidance-oriented, multiple approaches, or cross-file changes

- "Export this member from the module" - requires editing a different file
- "Reorder declarations" - complex transformation, agent should verify
- "Use the `#` operator for list length" - conceptual guidance, not a direct replacement

### Example: Parser Error with Both

When the parser encounters `/` instead of the canonical `⋅` separator:

```json
{
  "code": "SIGIL-PARSE-NS-SEP",
  "phase": "parser",
  "message": "invalid namespace separator",
  "location": {"file": "demo.sigil", "start": {"line": 3, "column": 18}},
  "found": "/",
  "expected": "⋅",
  "fixits": [{
    "kind": "replace",
    "range": {"file": "demo.sigil", "start": {"line": 3, "column": 18}},
    "text": "⋅"
  }],
  "suggestions": [{
    "kind": "replace_symbol",
    "message": "Use canonical namespace separator ⋅",
    "replacement": "⋅",
    "target": "namespace_separator"
  }]
}
```

Why both?

- **Fixit:** Can be applied immediately by a tool
- **Suggestion:** Explains the broader pattern ("always use ⋅ for namespace separation") for the AI's learning model

### Example: Canonical Ordering Error

When declarations are out of canonical order:

```json
{
  "code": "SIGIL-CANON-ORDERING-ALPHA",
  "phase": "canonical",
  "message": "declarations must be in alphabetical order within category",
  "location": {"file": "utils.sigil", "start": {"line": 15, "column": 1}},
  "found": "process_item",
  "expected": "get_config",
  "suggestions": [{
    "kind": "reorder_declaration",
    "message": "Move 'process_item' before 'get_config' (alphabetical order)",
    "category": "function",
    "name": "process_item",
    "before": "get_config"
  }]
}
```

No fixit here because:
- The compiler doesn't know the exact boundaries of each declaration (multi-line functions, comments, etc.)
- The agent needs to understand canonical ordering rules to apply the fix correctly
- The suggestion provides structured guidance instead

### Example: Type Error with Export Suggestion

When importing a non-exported module member:

```json
{
  "code": "SIGIL-TYPE-MODULE-NOT-EXPORTED",
  "phase": "typecheck",
  "message": "Module stdlib⋅list does not export 'length'",
  "location": {"file": "app.sigil", "start": {"line": 8, "column": 5}},
  "found": "length",
  "expected": "(one of the exported members)",
  "suggestions": [
    {
      "kind": "export_member",
      "message": "Export 'length' from stdlib⋅list",
      "targetFile": "language/stdlib/list.sigil",
      "member": "length"
    },
    {
      "kind": "use_operator",
      "message": "Use the # operator for list length",
      "operator": "#",
      "replaces": "length"
    }
  ]
}
```

Two suggestions because there are two valid recovery paths:

1. **Export the member** - Edit `stdlib/list.sigil` to add `export` keyword
2. **Use the operator** - Replace `length(xs)` with `#xs` in current file

The agent can choose based on context (user intent, stdlib vs user code, etc.).

## Specific Error Codes by Phase

We replaced generic errors with **specific, stable codes**. Many now include **suggestions** for non-trivial recovery:

### Lexer Errors

- `SIGIL-LEXER-TAB-CHARACTER` - Source contains tabs (surface-form violation)
- `SIGIL-LEXER-INVALID-ESCAPE` - Unknown escape sequence in string
- `SIGIL-LEXER-UNTERMINATED-STRING` - Missing closing quote

### Surface-Form Errors

- `SIGIL-SURFACE-NEWLINE-COMMENT` - Comment lacks newline terminator
- `SIGIL-SURFACE-TRAILING-WHITESPACE` - Line has trailing whitespace
- `SIGIL-SURFACE-BLANK-LINE` - File has blank lines (canonically forbidden)

### Canonical Validator Errors (with Suggestions)

- `SIGIL-CANON-ORDERING-CATEGORY` - Wrong category order (e.g., functions before types)
  - **Suggestion:** `reorder_declaration` with category and target name
- `SIGIL-CANON-ORDERING-ALPHA` - Wrong alphabetical order within category
  - **Suggestion:** `reorder_declaration` with names to swap
- `SIGIL-CANON-ORDERING-EXPORT` - Exported declaration before non-exported
  - **Suggestion:** `reorder_declaration` guidance
- `SIGIL-CANON-MATCH-REDUNDANT` - Redundant pattern in match expression
- `SIGIL-CANON-MATCH-NON-EXHAUSTIVE` - Pattern match missing cases
- `SIGIL-CANON-RECURSION-INVALID` - Invalid recursive function structure

### Parser Errors (with Fixits and Suggestions)

- `SIGIL-PARSE-LOCAL-BINDING` - Used `let` instead of `l`
  - **Fixit:** Replace `let` with `l`
  - **Suggestion:** `replace_symbol` explaining canonical keyword
- `SIGIL-PARSE-NS-SEP` - Used `/` or `.` instead of `⋅`
  - **Fixit:** Replace separator with `⋅`
  - **Suggestion:** `replace_symbol` explaining canonical separator
- `SIGIL-PARSE-CONST-NAME` - Const name not uppercase
  - Message shows expected uppercase form
- `SIGIL-PARSE-UNEXPECTED-TOKEN` - Generic fallback for other parse failures

### Typechecker Errors (with Suggestions)

- `SIGIL-TYPE-MODULE-NOT-EXPORTED` - Importing non-exported module member
  - **Suggestion:** `export_member` (edit target module) OR `use_operator` (use canonical operator instead)
- `SIGIL-TYPE-ERROR` - Generic type mismatch (fallback)

## Real-World Examples: Fixits and Suggestions in Action

### Example 1: Parser Fixit (Deterministic)

When the parser encounters `let` instead of Sigil's canonical `l`:

```typescript
// In parser.ts
if (this.peek().value === 'let') {
  const bad = this.advance();
  throw this.diagError('SIGIL-PARSE-LOCAL-BINDING', 'invalid local binding keyword', bad, {
    found: 'let',
    expected: 'l',
    fixits: [replaceTokenFixit(this.filename, bad, 'l')],
    suggestions: [suggestReplaceSymbol('Use canonical keyword "l" for local bindings', 'l', 'local_binding_keyword')]
  });
}
```

This produces:

```json
{
  "code": "SIGIL-PARSE-LOCAL-BINDING",
  "phase": "parser",
  "message": "invalid local binding keyword",
  "location": {"file": "demo.sigil", "start": {"line": 5, "column": 3}},
  "found": "let",
  "expected": "l",
  "fixits": [{
    "kind": "replace",
    "range": {"file": "demo.sigil", "start": {"line": 5, "column": 3}},
    "text": "l"
  }],
  "suggestions": [{
    "kind": "replace_symbol",
    "message": "Use canonical keyword \"l\" for local bindings",
    "replacement": "l",
    "target": "local_binding_keyword"
  }]
}
```

Claude Code can:
1. Apply the fixit immediately (deterministic correction)
2. Learn the pattern from the suggestion (always use `l`)

### Example 2: Canonical Ordering (Guidance-Only)

When functions are out of alphabetical order:

```json
{
  "code": "SIGIL-CANON-ORDERING-ALPHA",
  "phase": "canonical",
  "message": "declarations must be in alphabetical order within category",
  "location": {"file": "utils.sigil", "start": {"line": 22, "column": 1}},
  "found": "validate_input",
  "expected": "sort_items",
  "details": {
    "category": "function",
    "expectedOrder": ["process_data", "sort_items", "validate_input"]
  },
  "suggestions": [{
    "kind": "reorder_declaration",
    "message": "Move 'validate_input' before 'sort_items'",
    "category": "function",
    "name": "validate_input",
    "before": "sort_items"
  }]
}
```

No fixit because:
- Reordering requires understanding multi-line function boundaries
- Comments and whitespace must be preserved
- The agent needs to parse and restructure, not just replace text

The suggestion provides:
- Structured guidance (which declaration, where to move it)
- Enough context for the agent to implement the fix correctly
- Pattern learning (Sigil requires alphabetical order)

### Example 3: Type Error with Multiple Suggestions

When trying to use a non-exported stdlib function:

```json
{
  "code": "SIGIL-TYPE-MODULE-NOT-EXPORTED",
  "phase": "typecheck",
  "message": "Module stdlib⋅list does not export 'len'",
  "location": {"file": "app.sigil", "start": {"line": 12, "column": 10}},
  "found": "len",
  "expected": "(exported member)",
  "suggestions": [
    {
      "kind": "use_operator",
      "message": "Use the # operator for list length",
      "operator": "#",
      "replaces": "len"
    },
    {
      "kind": "export_member",
      "message": "Export 'len' from stdlib⋅list (if appropriate)",
      "targetFile": "language/stdlib/list.sigil",
      "member": "len"
    }
  ]
}
```

Two recovery paths:
1. **Preferred:** Use the canonical `#` operator (idiomatic Sigil)
2. **Alternative:** Export the function (if it's meant to be public)

The agent can choose based on:
- User intent (using stdlib vs modifying stdlib)
- Module ownership (stdlib is framework code)
- Canonical patterns (operators over functions when available)

No guessing. No prompt engineering. Just structured, machine-readable recovery guidance.

## JSON Envelopes for `run` Command

The `run` command previously printed child process stdout/stderr directly to the terminal. Now it captures both streams in the JSON envelope:

```json
{
  "formatVersion": 1,
  "command": "sigilc run",
  "ok": true,
  "phase": "runtime",
  "data": {
    "runtime": {
      "stdout": "Hello, world!\n",
      "stderr": "",
      "exitCode": 0
    }
  }
}
```

The `--human` flag extracts and prints them:

```bash
$ sigilc run demo.sigil --human
Hello, world!
sigilc run OK phase=runtime
```

This makes test automation trivial. A test harness can:
- Run `sigilc run test.sigil`
- Parse JSON envelope
- Check `data.runtime.stdout` for expected output
- Check `ok: true` for success
- No shell redirection, no parsing quirks

## From `<unknown>` to Real File Paths

Before the diagnostics system, many errors showed:

```
Error at <unknown>:12:3
```

Why? Because the lexer/parser didn't always have access to the filename string.

Now, the CLI passes the filename to every phase:

```typescript
function parseCommand(args: string[]) {
  const filename = cleaned[0];
  const source = readFileSync(filename, 'utf-8');
  const tokens = tokenize(source);
  const ast = parse(tokens, filename);  // ← filename passed here
  // ...
}
```

And the diagnostic unwrapper fixes any remaining `<unknown>` references:

```typescript
function unknownToDiagnostic(error: unknown, phase: SigilPhase, filename?: string): Diagnostic {
  if (isSigilDiagnosticError(error)) {
    const d = { ...error.diagnostic };
    if (filename && d.location && d.location.file === '<unknown>') {
      d.location = { ...d.location, file: filename };
    }
    return d;
  }
  // ...
}
```

**Result:** Every diagnostic includes the actual file path. Claude Code can edit the exact file without guessing.

## Empirical Motivation: Error Harvest from Examples

Why did we build this now? Because we ran all the examples and harvested the actual errors that occurred.

Common pain points:
- `let` vs `l` confusion (ultra high frequency)
- `/` or `.` instead of `⋅` in module imports
- Generic "unexpected token" messages with no guidance
- Missing file paths in error messages
- Vague canonical ordering violations

The diagnostics system directly addresses these empirical findings:
- Specific codes for high-frequency errors
- Fixits for mechanical corrections (`let` → `l`)
- File paths in every diagnostic
- Narrow codes instead of generic fallbacks

This wasn't theoretical design. It was **usage-driven improvement** from real AI-assisted development.

## The Compiler as a Machine API

Traditional compilers are designed as **human tools**:
- Pretty-printed errors
- Prose suggestions
- Colorized output
- Helpful hints

Sigil treats the compiler as a **machine API**:
- Structured responses
- Stable error codes
- Location data for automation
- Fixits for automated correction
- Single JSON envelope format

The `--human` flag is a **rendering** of the machine data, not a separate mode.

This inverts the typical priority:
- **Primary:** JSON diagnostics (for automation)
- **Derived:** Human-readable prose (for debugging)

Why? Because in 2026, **93% of code is AI-generated**. The compiler should optimize for the 93%, not the 7%.

## Integration with Claude Code Hooks

Structured JSON output makes hooks trivial:

```bash
#!/usr/bin/env bash
# Hook: run stdlib tests after editing compiler/stdlib

# Read Claude hook event from stdin
event=$(cat)

# Extract edited file path
file=$(echo "$event" | jq -r '.filePath')

# Skip if not relevant
if [[ ! "$file" =~ ^language/(stdlib|compiler/src)/ ]]; then
  exit 0
fi

# Run tests (JSON output by default)
result=$(pnpm sigil:test:stdlib)

# Parse result (trivial with JSON)
ok=$(echo "$result" | jq -r '.ok')

if [[ "$ok" != "true" ]]; then
  echo "Stdlib tests failed after editing $file"
  echo "$result" | jq -r '.error.message'
  exit 1
fi
```

No shell gymnastics. No brittle parsing. Just `jq` and structured data.

## Benefits: Quantifiable

### 1. Faster AI Convergence

**Before:** Generic "parse error" → agent tries 5 different fixes → eventually succeeds

**After:** Specific `SIGIL-PARSE-LOCAL-BINDING` → agent applies fixit → succeeds immediately

### 2. Repeatable Recovery Patterns

**Before:** Agent learns "sometimes `let` works, sometimes it doesn't"

**After:** Agent learns "code `SIGIL-PARSE-LOCAL-BINDING` means replace with `l`"

Stable codes enable **pattern recognition** across edits.

### 3. Tool Integration

**Before:** Each tool must parse prose output differently

**After:** Every tool uses the same JSON envelope schema

### 4. Training Data Quality

**Before:** Error messages vary by context, location format inconsistent

**After:** Every error has same structure, same fields, same format

Clean training data for error recovery.

## Schema-Driven Development: Why It Matters

Having a formal JSON Schema for CLI output isn't just about documentation. It enables a whole ecosystem of tooling and validation:

### 1. Automated Validation

CI pipelines can validate CLI output against the schema:

```bash
# In CI
sigilc compile example.sigil | jq . | ajv validate -s language/spec/cli-json.schema.json
```

If the compiler emits invalid JSON (wrong field types, missing required fields, etc.), the build fails immediately.

### 2. Type Generation

TypeScript types can be generated from the schema:

```bash
json-schema-to-typescript language/spec/cli-json.schema.json > types.ts
```

Tools written in TypeScript get full type safety when consuming CLI output.

### 3. Cross-Language Support

The schema is language-agnostic. Tools written in:
- Python (with `jsonschema` library)
- Rust (with `serde_json` + `jsonschema`)
- Go (with `gojsonschema`)
- Any language with a JSON Schema validator

can all validate Sigil CLI output using the same canonical schema.

### 4. Version Evolution

The `formatVersion` field enables safe schema evolution:

```typescript
function handleOutput(envelope: any) {
  switch (envelope.formatVersion) {
    case 1:
      // Handle version 1 format
      return processV1(envelope);
    case 2:
      // Handle future version 2 format
      return processV2(envelope);
    default:
      throw new Error(`Unsupported format version: ${envelope.formatVersion}`);
  }
}
```

When we need to make breaking changes (remove fields, change types), we increment `formatVersion` and tools can handle both versions during migration.

### 5. Documentation as Code

The schema itself serves as machine-readable documentation:

- Field names and types are authoritative
- Required vs optional fields are explicit
- Enum values are defined
- Relationships (`oneOf`, `allOf`) are formal

No prose ambiguity. The schema is the truth.

## Implementation Cost: Minimal

Core diagnostics infrastructure:
- `types.ts` — 85 lines (type definitions including Suggestion variants)
- `error.ts` — 13 lines (wrapper class)
- `helpers.ts` — 67 lines (helper functions including suggestion builders)

**Total: ~165 lines of diagnostics infrastructure.**

JSON Schema and spec:
- `cli-json.schema.json` — 420 lines (formal schema)
- `cli-json.md` — 91 lines (companion spec doc)

**Total: ~511 lines of schema/spec.**

CLI changes:
- JSON envelope wrapper: ~50 lines
- `--human` renderer: ~40 lines

**Total: ~90 lines of CLI changes.**

Converting existing errors to use diagnostics:
- Lexer: ~20 updates
- Parser: ~25 updates (added suggestions)
- Validator: ~40 updates (added suggestions for ordering/canonical errors)
- Typechecker: ~15 updates (added suggestions for export/operator hints)

**Total: ~100 updates.**

Regression tests:
- `cli-json-diagnostics.test.ts` — ~80 lines (schema validation, suggestion tests)

**Grand total: ~946 lines changed/added** for a complete machine-first diagnostics system with formal schema, suggestions, and regression tests.

Small implementation cost, massive automation benefit.

## Comparison to Other Languages

### Rust (`rustc`) ✅

```
error[E0425]: cannot find value `x` in this scope
 --> src/main.rs:2:5
  |
2 |     x
  |     ^ not found in this scope
```

**Good:**
- Stable error codes (`E0425`)
- Precise location data
- Clear messages

**Missing:**
- JSON output requires `--error-format=json` (opt-in)
- No fixits in standard output
- Human-readable is the default

### TypeScript (`tsc`) ✅

```
error TS2304: Cannot find name 'x'.
  src/main.ts(2,5): error TS2304: Cannot find name 'x'.
```

**Good:**
- Stable error codes (`TS2304`)
- File/line/column location

**Missing:**
- No JSON mode
- No fixits
- Prose is the only format

### Sigil ✅✅

```json
{
  "ok": false,
  "phase": "parser",
  "error": {
    "code": "SIGIL-PARSE-LOCAL-BINDING",
    "location": {"file": "main.sigil", "start": {"line": 2, "column": 5}},
    "message": "invalid local binding keyword",
    "found": "let",
    "expected": "l",
    "fixits": [{"kind": "replace", "range": {...}, "text": "l"}]
  }
}
```

**Better:**
- JSON by default (not opt-in)
- Structured fixits included
- Single envelope format for all commands
- Machine-readable is primary, human-readable is derived

## Status and Verification

**Current status (February 25, 2026):**
- ✅ Diagnostics infrastructure implemented
- ✅ All CLI commands emit JSON envelopes by default
- ✅ `--human` flag working for all commands
- ✅ Parser fixits for high-frequency errors
- ✅ Suggestions system for complex recovery guidance
- ✅ Formal JSON Schema published (`language/spec/cli-json.schema.json`)
- ✅ Companion spec document (`language/spec/cli-json.md`)
- ✅ Regression tests for schema validation and suggestions
- ✅ All tests passing
- ✅ All examples compiling/running
- ✅ Real file paths in all diagnostics

**Commands verified:**
```bash
$ sigilc lex example.sigil              # JSON envelope
$ sigilc lex example.sigil --human      # Human-readable
$ sigilc parse example.sigil            # JSON envelope
$ sigilc compile example.sigil          # JSON envelope
$ sigilc run example.sigil              # JSON with runtime capture
$ sigilc test projects/algorithms/tests # JSON test results
```

**Schema validation verified:**
```bash
# Validate compiler output against schema
$ sigilc compile example.sigil | jq . | ajv validate -s language/spec/cli-json.schema.json
schema language/spec/cli-json.schema.json is valid
validation passed
```

**Regression tests verified:**
```bash
$ pnpm --filter @sigil-lang/compiler test cli-json-diagnostics.test.ts
✓ CLI JSON schema file exists and is valid JSON
✓ Parser separator error includes fixit and suggestion
✓ Canonical ordering error includes reorder suggestion
✓ Type missing-export error includes suggestions
```

## What This Enables (Next Steps)

Now that the compiler speaks structured JSON, we can build:

1. **LSP integration** - Diagnostics feed directly into editor problems panel
2. **Auto-fixers** - Apply fixit suggestions without manual edits
3. **Test runners** - Parse test results programmatically
4. **CI integration** - Structured failure data in build logs
5. **Error analytics** - Track error code frequency across projects
6. **Agent training** - Clean error recovery examples for AI training
7. **Git hooks** - Validate commits with parseable results

All of these require **stable, structured output**. JSON-first diagnostics make them trivial.

## The Bigger Picture: Compiler UX for AI

Sigil's diagnostics system reflects a broader philosophy about **compiler UX in an AI-first world**:

1. **Machine-readable primary, human-readable derived**
   - Not "both are equal"
   - JSON is canonical, prose is rendered

2. **Stable codes enable automation**
   - Not vague messages
   - Specific, narrow error codes

3. **Location data must be exact**
   - Not "somewhere around line 42"
   - Precise file/line/column for automated edits

4. **Fixits and suggestions are first-class**
   - Fixits for deterministic corrections (apply immediately)
   - Suggestions for complex guidance (understand, then act)
   - Not just "here's a hint" in prose

5. **One canonical mode**
   - Not optional `--json`
   - JSON by default, `--human` to opt-in

6. **Formal schema as the contract**
   - Not prose-only docs
   - Machine-validatable, version-aware, language-agnostic

7. **Structured recovery guidance**
   - Not "you might want to..."
   - Typed suggestion variants with machine-readable fields

## Conclusion

Machine-first JSON diagnostics aren't just "better error messages." They're a fundamental shift in how compilers serve AI coding agents.

When Claude Code runs Sigil commands, it gets:
- **Structured responses** (parse once, use everywhere)
- **Stable error codes** (pattern recognition across edits)
- **Exact locations** (precise automated edits)
- **Fixits** (deterministic corrections for mechanical issues)
- **Suggestions** (structured guidance for complex recovery)
- **Formal schema** (validatable contract, not prose guessing)
- **Version-aware output** (`formatVersion` for safe evolution)
- **Single envelope format** (no command-specific parsing)

The implementation cost was minimal (~946 lines total including schema, suggestions, and tests). The automation benefit is massive.

**This is what compiler UX looks like when you optimize for the 93%.**

Vague errors hurt agents. Typed diagnostics help them converge faster. JSON-first output eliminates parsing ambiguity. Fixits enable automated recovery. Suggestions provide recovery guidance even when exact fixes aren't possible. A formal schema makes the compiler a testable, versioned contract.

Sigil treats the compiler as a **reliable machine API**, not a prose generator.

And that makes all the difference.

---

**Try it yourself:**

```bash
# Default: JSON output
$ sigilc compile example.sigil
{"formatVersion":1,"command":"sigilc compile","ok":true,...}

# Opt-in: Human-readable
$ sigilc compile example.sigil --human
sigilc compile OK phase=codegen
```

**Read the implementation:**
- `language/spec/cli-json.schema.json` — Formal JSON Schema (canonical contract)
- `language/spec/cli-json.md` — Companion spec document
- `language/compiler/src/diagnostics/types.ts` — Core types (Diagnostic, Fixit, Suggestion)
- `language/compiler/src/diagnostics/error.ts` — Error wrapper
- `language/compiler/src/diagnostics/helpers.ts` — Helper functions (suggestion builders)
- `language/compiler/src/cli.ts` — JSON envelope handling
- `language/compiler/test/cli-json-diagnostics.test.ts` — Regression tests

**ONE canonical output. Zero ambiguity. Maximum automation. Formal contract.**
