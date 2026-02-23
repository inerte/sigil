/**
 * Mint Semantic Map Enhancement
 *
 * Enhances basic semantic maps using Claude Code CLI
 */

import { execSync } from 'child_process';
import * as path from 'path';

/**
 * Enhance semantic map with Claude Code CLI
 */
export function enhanceWithClaude(mintFile: string, mapFile: string): void {
  const prompt = buildEnhancementPrompt(mintFile, mapFile);

  try {
    // Invoke Claude Code CLI to enhance the semantic map
    execSync(`claude -p "${escapePrompt(prompt)}" --allowedTools Write Read`, {
      stdio: 'pipe',  // Capture output silently
      cwd: path.dirname(mintFile)
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
function buildEnhancementPrompt(mintFile: string, mapFile: string): string {
  return `
Enhance the semantic map for Mint source code.

Files:
- Source: ${mintFile}
- Basic map: ${mapFile}

Read the basic semantic map. For each mapping:
1. Extract the code snippet using the range offsets from the source file
2. Analyze what it does, how it works, and performance characteristics
3. Add rich documentation fields:
   - explanation: Detailed markdown explanation of the code
   - complexity: Time/space complexity if applicable (e.g., "O(n) time, O(1) space")
   - warnings: Array of edge cases, performance issues, or limitations
   - examples: Array of usage examples showing input → output
   - related: Array of related function/type names

Write the enhanced version back to ${mapFile}.

Match the quality and style of examples in examples/fibonacci.mint.map and examples/list-operations.mint.map.
`.trim();
}

/**
 * Escape prompt for shell execution
 */
function escapePrompt(prompt: string): string {
  return prompt.replace(/"/g, '\\"').replace(/\$/g, '\\$');
}
