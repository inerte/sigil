/**
 * Sigil Semantic Map Enhancement
 *
 * Enhances basic semantic maps using Claude Code CLI
 */

import { execSync } from 'child_process';
import * as path from 'path';

/**
 * Enhance semantic map with Claude Code CLI
 */
export function enhanceWithClaude(sigilFile: string, mapFile: string): void {
  if (process.env.SIGIL_ENABLE_MAP_ENHANCE !== '1') {
    return;
  }

  const prompt = buildEnhancementPrompt(sigilFile, mapFile);

  try {
    // Invoke Claude Code CLI to enhance the semantic map
    execSync(`claude -p "${escapePrompt(prompt)}" --allowedTools Write Read`, {
      stdio: 'pipe',  // Capture output silently
      cwd: path.dirname(sigilFile),
      timeout: 5000,
    });
  } catch (error) {
    // If Claude Code is not available, silently continue
    // The basic map still exists, just not enhanced
    console.warn(`Warning: Could not enhance semantic map (Claude Code CLI not available)`);
  }
}

/**
 * Build enhancement prompt for Claude Code
 */
function buildEnhancementPrompt(sigilFile: string, mapFile: string): string {
  return `
Enhance the semantic map for Sigil source code.

Files:
- Source: ${sigilFile}
- Basic map: ${mapFile}

IMPORTANT - Sigil Language Constraints:
Read AGENTS.md to understand Sigil's canonical form philosophy.

Key constraints to remember:
- Sigil enforces ONE WAY to write code (canonical forms)
- NO tail-call optimization, NO accumulator-passing style, NO iterative patterns
- Only primitive recursion allowed (direct recursive calls)
- Don't suggest "iterative version" or "tail-recursive version" - these are BLOCKED
- Performance warnings should be Mint-appropriate (e.g., "inherent to primitive recursion")

Read the basic semantic map. For each mapping:
1. Extract the code snippet using the range offsets from the source file
2. Analyze what it does, how it works, and performance characteristics
3. Add rich documentation fields:
   - explanation: Detailed markdown explanation of the code
   - complexity: Time/space complexity (e.g., "O(n) time, O(n) space due to recursion")
   - warnings: Edge cases, limitations (Sigil-appropriate - don't suggest impossible alternatives)
   - examples: Usage examples showing input → output
   - related: Related function/type names

Write the enhanced version back to ${mapFile}.

Match the quality and style of examples in examples/fibonacci.sigil.map and examples/list-operations.sigil.map.

Remember: Sigil is a canonical-form language. Warnings should acknowledge this, not fight it.
`.trim();
}

/**
 * Escape prompt for shell execution
 */
function escapePrompt(prompt: string): string {
  return prompt.replace(/"/g, '\\"').replace(/\$/g, '\\$');
}
