# Mint Language Server (LSP)

Language Server Protocol implementation for the Mint programming language.

## Features

### ✅ Real-time Diagnostics
- **Syntax errors** from lexer/parser
- **Type errors** from bidirectional type checker
- **Canonical form violations** from validator
- **Mutability errors** from mutability checker

Errors appear as you type with precise source locations.

### ✅ Hover Tooltips (Semantic Maps!)
Hover over any Mint code to see AI-generated documentation:
- Function explanations
- Type signatures
- Complexity analysis
- Warnings and edge cases
- Usage examples

**Example:** Hover over `factorial` shows:
```
Function: factorial

Computes the factorial of n recursively using pattern matching.
Base cases: 0! = 1 and 1! = 1. Recursive case: n! = n × (n-1)!.

Type: λ(ℤ)→ℤ
Complexity: O(n) time, O(n) space

⚠️ Warnings:
- Stack overflow for large n
- O(n) stack depth is inherent to primitive recursion
```

### ✅ Unicode Symbol Completion
Type simple names and autocomplete to Unicode symbols:

| Type | Get | Symbol |
|------|-----|--------|
| `lambda` | λ | Lambda function |
| `arrow` | → | Function return arrow |
| `match` | ≡ | Pattern matching |
| `int` | ℤ | Integer type |
| `bool` | 𝔹 | Boolean type |
| `string` | 𝕊 | String type |
| `map` | ↦ | List map operation |
| `filter` | ⊳ | List filter operation |
| `fold` | ⊕ | List fold operation |
| `true` | ⊤ | Boolean true |
| `false` | ⊥ | Boolean false |

### ✅ Document Symbols
Outline view showing:
- All functions with type signatures
- Type declarations
- Hierarchical structure

## Building

```bash
cd tools/lsp
pnpm install
pnpm build
```

Output: `dist/server.js` - the LSP server

## Testing

The LSP server communicates via stdin/stdout using the LSP protocol.

**Manual test:**
```bash
node dist/server.js
```

The server will wait for LSP messages on stdin.

**With an LSP client:**
Configure your editor to use `/path/to/tools/lsp/dist/server.js` as the Mint language server.

## VS Code Extension

See `tools/vscode-extension/` (upcoming) for the official VS Code integration.

## Architecture

```
server.ts          Main LSP server, connection setup
diagnostics.ts     Real-time error checking via compiler
hover.ts           Load .mint.map files, format as markdown
completion.ts      Unicode symbol autocomplete
symbols.ts         Document outline from AST
types.ts           TypeScript type definitions
```

All features integrate with the existing Mint compiler:
- `compiler/dist/lexer/lexer.js` - Tokenization
- `compiler/dist/parser/parser.js` - Parsing
- `compiler/dist/typechecker/index.js` - Type checking
- `compiler/dist/mutability/index.js` - Mutability checking
- `compiler/dist/mapgen/index.js` - Semantic maps (for hover)

## Status

✅ **Phase 1 Complete** (2026-02-23)
- Basic server infrastructure
- Document tracking
- Real-time diagnostics
- Hover with semantic maps
- Unicode completions
- Document symbols

⏳ **Phase 2 Upcoming**
- VS Code extension
- Go to definition
- Find references
- Code formatting

## License

MIT - See root LICENSE file
