import { randomUUID } from 'node:crypto';
import { promises as fs } from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';

import type { PatchStats, ShellCommandResult } from './types.js';

export async function ensureDir(dirPath: string): Promise<void> {
  await fs.mkdir(dirPath, { recursive: true });
}

export async function readJsonFile<T>(filePath: string): Promise<T> {
  return JSON.parse(await fs.readFile(filePath, 'utf8')) as T;
}

export async function writeJsonFile(filePath: string, value: unknown): Promise<void> {
  await ensureDir(path.dirname(filePath));
  await fs.writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}

export async function writeTextFile(filePath: string, text: string): Promise<void> {
  await ensureDir(path.dirname(filePath));
  await fs.writeFile(filePath, text, 'utf8');
}

export async function copyDirectory(sourceDir: string, targetDir: string): Promise<void> {
  await ensureDir(path.dirname(targetDir));
  await fs.cp(sourceDir, targetDir, { recursive: true });
}

export function runTimestamp(): string {
  return new Date().toISOString().replace(/[:.]/g, '-');
}

export function runId(): string {
  return `${runTimestamp()}-${randomUUID().slice(0, 8)}`;
}

export function median(values: number[]): number {
  if (values.length === 0) {
    return 0;
  }

  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);

  return sorted.length % 2 === 0
    ? Math.round((sorted[middle - 1] + sorted[middle]) / 2)
    : sorted[middle];
}

export function normalizeRelativePath(value: string): string {
  return value.replace(/\\/g, '/').replace(/^\.?\//, '').replace(/\/+$/, '');
}

export function pathMatchesPrefix(relativePath: string, prefixes: string[]): boolean {
  const normalizedPath = normalizeRelativePath(relativePath);

  return prefixes.some((prefix) => {
    const normalizedPrefix = normalizeRelativePath(prefix);
    return normalizedPath === normalizedPrefix || normalizedPath.startsWith(`${normalizedPrefix}/`);
  });
}

export function summarizePatchStats(diffNumstat: string): PatchStats {
  const lines = diffNumstat
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean);

  let additions = 0;
  let deletions = 0;

  for (const line of lines) {
    const [added, removed] = line.split(/\s+/);
    additions += Number.parseInt(added, 10) || 0;
    deletions += Number.parseInt(removed, 10) || 0;
  }

  return {
    additions,
    deletions,
    filesChanged: lines.length
  };
}

export async function execShellCommand(
  command: string,
  cwd: string,
  env: Record<string, string>,
  timeoutMs = 120_000
): Promise<ShellCommandResult> {
  const startedAt = Date.now();

  return new Promise((resolve) => {
    const child = spawn('/bin/zsh', ['-lc', command], {
      cwd,
      env: {
        ...process.env,
        ...env
      }
    });

    let stdout = '';
    let stderr = '';
    let settled = false;

    const timer = setTimeout(() => {
      if (!settled) {
        child.kill('SIGKILL');
      }
    }, timeoutMs);

    child.stdout.on('data', (chunk) => {
      stdout += chunk.toString();
    });

    child.stderr.on('data', (chunk) => {
      stderr += chunk.toString();
    });

    child.on('close', (code) => {
      settled = true;
      clearTimeout(timer);
      resolve({
        command,
        cwd,
        stdout,
        stderr,
        exitCode: code ?? 1,
        durationMs: Date.now() - startedAt
      });
    });
  });
}

export function humanJson(value: unknown): string {
  return `${JSON.stringify(value, null, 2)}\n`;
}

