import { promises as fs } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { copyDirectory, ensureDir, execShellCommand, summarizePatchStats } from './util.js';
import type { PatchStats, PathPolicyResult, TaskManifest } from './types.js';

export async function prepareTaskWorkspace(task: TaskManifest, fixturesDir: string): Promise<string> {
  const fixtureDir = path.join(fixturesDir, task.fixture);
  const workspaceRoot = await fs.mkdtemp(path.join(os.tmpdir(), `sigil-devex-${task.id}-`));

  await copyDirectory(fixtureDir, workspaceRoot);
  await execShellCommand('git init -q', workspaceRoot, {});
  await execShellCommand('git config user.email "benchmarks@sigil.local"', workspaceRoot, {});
  await execShellCommand('git config user.name "Sigil Benchmarks"', workspaceRoot, {});
  await execShellCommand('git add .', workspaceRoot, {});
  await execShellCommand('git commit -qm "fixture baseline"', workspaceRoot, {});

  return workspaceRoot;
}

export async function cleanupWorkspace(workspacePath: string): Promise<void> {
  await fs.rm(workspacePath, { recursive: true, force: true });
}

export async function collectModifiedPaths(workspacePath: string): Promise<string[]> {
  const status = await execShellCommand('git status --porcelain', workspacePath, {});

  return status.stdout
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => line.slice(3).trim())
    .filter((relativePath) => !relativePath.startsWith('.local/'))
    .sort();
}

export async function collectPatch(workspacePath: string): Promise<{ diff: string; stats: PatchStats }> {
  const patch = await execShellCommand('git diff --binary --no-ext-diff', workspacePath, {});
  const numstat = await execShellCommand('git diff --numstat --no-ext-diff', workspacePath, {});

  return {
    diff: patch.stdout,
    stats: summarizePatchStats(numstat.stdout)
  };
}

export function evaluatePathPolicy(task: TaskManifest, modifiedPaths: string[]): PathPolicyResult {
  const normalize = (value: string) => value.replace(/\\/g, '/').replace(/^\.?\//, '').replace(/\/+$/, '');
  const matches = (value: string, prefixes: string[]) => {
    const normalizedValue = normalize(value);
    return prefixes.some((prefix) => {
      const normalizedPrefix = normalize(prefix);
      return normalizedValue === normalizedPrefix || normalizedValue.startsWith(`${normalizedPrefix}/`);
    });
  };

  const forbiddenMatches = modifiedPaths.filter((modifiedPath) => matches(modifiedPath, task.forbiddenEditPaths));
  const outOfBoundsMatches = modifiedPaths.filter((modifiedPath) => !matches(modifiedPath, task.allowedEditPaths));

  return {
    allowed: forbiddenMatches.length === 0 && outOfBoundsMatches.length === 0,
    forbiddenMatches,
    outOfBoundsMatches
  };
}

export async function createWorktree(repoRoot: string, localRoot: string, refLabel: string, ref: string): Promise<{ worktreePath: string; resolvedRef: string }> {
  await ensureDir(localRoot);
  const worktreePath = path.join(localRoot, `${refLabel}-${ref.replace(/[^A-Za-z0-9._-]/g, '_')}`);
  await fs.rm(worktreePath, { recursive: true, force: true });
  await execShellCommand(`git worktree add --detach "${worktreePath}" "${ref}"`, repoRoot, {}, 600_000);

  const resolved = await execShellCommand('git rev-parse HEAD', worktreePath, {});
  return {
    worktreePath,
    resolvedRef: resolved.stdout.trim()
  };
}

export async function removeWorktree(repoRoot: string, worktreePath: string): Promise<void> {
  await execShellCommand(`git worktree remove --force "${worktreePath}"`, repoRoot, {}, 600_000);
  await fs.rm(worktreePath, { recursive: true, force: true });
}
