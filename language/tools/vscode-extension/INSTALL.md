# Installing the Mint VS Code Extension

## Quick Install (Recommended)

### Step 1: Build the LSP Server
```bash
cd tools/lsp
pnpm install
pnpm build
```

### Step 2: Build the Extension
```bash
cd ../vscode-extension
pnpm install
pnpm build
```

### Step 3: Install in VS Code

**Option A: Development Mode (Recommended for testing)**

1. **Open VS Code to the extension folder**:
   - File → Open Folder...
   - Navigate to: `/path/to/ai-pl/tools/vscode-extension`
   - Click **Open**

2. **Open the Run and Debug panel**:
   - Click the **Run and Debug** icon in the left sidebar (▶️ with bug icon)
   - Or press **Cmd+Shift+D** (Ctrl+Shift+D on Windows/Linux)

3. **Launch the Extension Development Host**:
   - At the top of the panel, you'll see "Run Extension" in the dropdown
   - Click the **green play button ▶️** next to it
   - This opens a new "Extension Development Host" window
   - The debug toolbar will stay visible at the top

4. **In the Extension Development Host window**:
   - Open a folder with Mint files (File → Open Folder...)
   - Or create a new `.mint` file (File → New File → Save as `test.mint`)
   - The extension will activate automatically for `.mint` files

**Option B: Install from VSIX**
1. Package the extension:
   ```bash
   pnpm package
   ```
   This creates `mint-language-0.1.0.vsix`

2. Install in VS Code:
   - Open VS Code
   - Go to Extensions (`Cmd+Shift+X` / `Ctrl+Shift+X`)
   - Click "..." menu → "Install from VSIX..."
   - Select `mint-language-0.1.0.vsix`

3. Reload VS Code when prompted

## Testing the Extension

### 1. Create a Test File

Create `test.mint`:
```mint
⟦ Factorial function using primitive recursion ⟧
λfactorial(n:ℤ)→ℤ≡n{
  0→1|
  1→1|
  n→n*factorial(n-1)
}

λmain()→𝕊="factorial(5) = " + factorial(5)
```

### 2. Verify Features

**✅ Syntax Highlighting**
- Keywords (`λ`) should be highlighted
- Operators (`→`, `≡`) should be highlighted
- Types (`ℤ`, `𝕊`) should be highlighted
- Comments (`⟦ ... ⟧`) should be highlighted

**✅ Real-time Diagnostics**
Try adding an error:
```mint
λbad()→ℤ="hello"
```
You should see a red squiggle with error: "Literal type mismatch: expected ℤ, got 𝕊"

**✅ Hover Tooltips**
Hover over `factorial` to see:
- Function explanation
- Type signature
- Complexity analysis
- Warnings

**✅ Unicode Completions**
Type `lambda` and you should see completion suggestion for `λ`

**✅ Document Symbols**
Open the Outline view (View → Outline) to see:
- factorial function
- main function

## Troubleshooting

### Extension not activating
1. Check the Output panel: View → Output
2. Select "Mint Language Server" from dropdown
3. Look for error messages

### LSP server not found
The extension looks for the LSP server at:
`../lsp/dist/server.js` (relative to extension)

Make sure you built the LSP server first:
```bash
cd tools/lsp && pnpm build
```

### No syntax highlighting
- Ensure file has `.mint` extension
- Reload VS Code: Cmd+Shift+P → "Reload Window"

### Unicode symbols not showing
Install a font that supports Unicode:
- JetBrains Mono (recommended)
- Fira Code
- Cascadia Code

## Development

### Watch Mode
Terminal 1 - LSP Server:
```bash
cd tools/lsp
pnpm watch
```

Terminal 2 - Extension:
```bash
cd tools/vscode-extension
pnpm watch
```

Then launch Extension Development Host:
1. Open **Run and Debug** panel (Cmd+Shift+D)
2. Click the **green play button ▶️** next to "Run Extension"

### Debugging

**Extension:**
- Set breakpoints in `src/extension.ts`
- Open Run and Debug panel (Cmd+Shift+D)
- Click green play button ▶️ to launch Extension Development Host
- Breakpoints will hit when the extension code executes

**LSP Server:**
1. In Extension Development Host, open Output panel
2. Select "Mint Language Server"
3. Add `console.log()` statements in LSP server code
4. Logs appear in Output panel

## Next Steps

Once the extension is working:
1. Test with real Mint files in `examples/` directory
2. Verify all LSP features work
3. Report any issues or bugs
4. Consider publishing to VS Code Marketplace

## Publishing (Future)

To publish to VS Code Marketplace:
1. Get a publisher account
2. Update `package.json` with correct publisher
3. Run `pnpm publish`

See: https://code.visualstudio.com/api/working-with-extensions/publishing-extension
