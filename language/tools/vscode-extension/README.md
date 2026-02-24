# Mint Language for VS Code

Official VS Code extension for the Sigil programming language.

## Features

### 🎨 Syntax Highlighting
Beautiful syntax highlighting for Sigil code with support for:
- Unicode symbols (λ, →, ≡, ℤ, 𝕊, 𝔹, etc.)
- Function definitions
- Pattern matching
- Type annotations
- Comments (⟦ ... ⟧)
- Built-in operations (↦, ⊳, ⊕)

### ⚡ Real-time Diagnostics
Instant error checking as you type:
- **Syntax errors** - Invalid tokens, malformed code
- **Type errors** - Type mismatches, undefined functions
- **Canonical form violations** - Accumulator patterns, non-canonical forms
- **Mutability errors** - Illegal mutations, aliasing issues

All errors show with precise locations and helpful messages.

### 💡 Intelligent Hover
Hover over any Sigil code to see AI-generated documentation:
- Function explanations
- Type signatures
- Complexity analysis (Big-O)
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

### ✨ Unicode Completions
Type simple names and autocomplete to Unicode symbols:

| Type       | Get | Symbol               |
|------------|-----|----------------------|
| `lambda`   | λ   | Lambda function      |
| `arrow`    | →   | Function return      |
| `match`    | ≡   | Pattern matching     |
| `int`      | ℤ   | Integer type         |
| `bool`     | 𝔹   | Boolean type         |
| `string`   | 𝕊   | String type          |
| `map`      | ↦   | List map             |
| `filter`   | ⊳   | List filter          |
| `fold`     | ⊕   | List fold            |
| `true`     | ⊤   | Boolean true         |
| `false`    | ⊥   | Boolean false        |

### 📑 Document Outline
Navigate your code with the outline view showing:
- All functions with type signatures
- Type declarations
- Hierarchical structure

## Installation

### Development Mode (Testing/Debugging)
1. Open VS Code to `tools/vscode-extension` folder
2. Open **Run and Debug** panel (Cmd+Shift+D)
3. Click the **green play button ▶️** next to "Run Extension"
4. In the Extension Development Host window, open a folder with `.sigil` files

### From VSIX (Installation)
1. Build the extension: `cd tools/vscode-extension && pnpm package`
2. Open VS Code
3. Go to Extensions (`Cmd+Shift+X` / `Ctrl+Shift+X`)
4. Click "..." menu → "Install from VSIX..."
5. Select `sigil-language-0.1.0.vsix`

### From Marketplace (Future)
Search for "Mint Language" in the VS Code Extensions marketplace.

## Requirements

- VS Code 1.75.0 or higher
- Node.js 16.0 or higher (for LSP server)

The extension includes a bundled Mint Language Server (LSP).

## Extension Settings

This extension contributes the following settings:

* `sigil.trace.server`: Enable/disable tracing of communication with the language server
  - `off` (default) - No tracing
  - `messages` - Trace messages
  - `verbose` - Verbose tracing

* `sigil.lsp.path`: Custom path to Mint LSP server
  - Leave empty to use bundled server
  - Set to custom path for development

## Quick Start

1. Create a new file with `.sigil` extension
2. Start writing Sigil code:

```sigil
λfactorial(n:ℤ)→ℤ≡n{
  0→1|
  1→1|
  n→n*factorial(n-1)
}

λmain()→𝕊="factorial(5) = " + factorial(5)
```

3. Enjoy:
   - Syntax highlighting
   - Error checking as you type
   - Hover for documentation
   - Unicode completions

## Troubleshooting

### Language server not starting

Check the Output panel (View → Output) and select "Mint Language Server" to see error messages.

### No syntax highlighting

Ensure the file has `.sigil` extension. VS Code detects the language by file extension.

### Unicode symbols not showing

Ensure your editor font supports Unicode characters. Recommended fonts:
- JetBrains Mono
- Fira Code
- Cascadia Code

## Development

See the main Sigil repository for development instructions:
https://github.com/sigil-lang/mint

### Building from source

```bash
cd tools/vscode-extension
pnpm install
pnpm build
pnpm package  # Creates .vsix file
```

## Release Notes

### 0.1.0 (Initial Release)

- ✅ Syntax highlighting for Mint
- ✅ Real-time diagnostics (syntax, type, canonical, mutability)
- ✅ Hover tooltips with AI-generated documentation
- ✅ Unicode symbol completions
- ✅ Document outline
- ✅ Language Server Protocol integration

## More Information

- [Mint Language Website](https://github.com/sigil-lang/mint)
- [Language Specification](https://github.com/sigil-lang/mint/tree/main/spec)
- [Examples](https://github.com/sigil-lang/mint/tree/main/examples)

## License

MIT - See LICENSE file

---

**Mint** - Fresh code for AI 🌿
