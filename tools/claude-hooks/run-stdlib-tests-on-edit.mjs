#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import path from 'node:path';

function readStdin() {
  return new Promise((resolve, reject) => {
    let data = '';
    process.stdin.setEncoding('utf8');
    process.stdin.on('data', (chunk) => {
      data += chunk;
    });
    process.stdin.on('end', () => resolve(data));
    process.stdin.on('error', reject);
  });
}

function getEditedPath(payload) {
  const candidates = [
    payload?.tool_input?.file_path,
    payload?.toolInput?.file_path,
    payload?.tool_input?.path,
    payload?.toolInput?.path,
    payload?.file_path,
    payload?.path,
  ];
  for (const c of candidates) {
    if (typeof c === 'string' && c.length > 0) return c;
  }
  return null;
}

function getToolName(payload) {
  const candidates = [
    payload?.tool_name,
    payload?.toolName,
    payload?.tool,
    payload?.name,
  ];
  for (const c of candidates) {
    if (typeof c === 'string' && c.length > 0) return c;
  }
  return '';
}

function normalizeRelPath(projectDir, filePath) {
  const abs = path.isAbsolute(filePath) ? filePath : path.resolve(projectDir, filePath);
  let rel = path.relative(projectDir, abs);
  if (rel === '') rel = '.';
  return rel.split(path.sep).join('/');
}

function shouldRunForPath(relPath) {
  if (relPath.startsWith('.local/')) return false;
  if (relPath.startsWith('node_modules/')) return false;
  if (relPath.startsWith('language/stdlib/')) return true;
  if (relPath.startsWith('language/compiler/src/')) return true;
  return false;
}

const raw = await readStdin();
if (!raw.trim()) {
  console.log('[stdlib-hook] Skipping (no hook payload on stdin)');
  process.exit(0);
}

let payload;
try {
  payload = JSON.parse(raw);
} catch (err) {
  console.log('[stdlib-hook] Skipping (invalid hook JSON payload)');
  process.exit(0);
}

const toolName = getToolName(payload);
if (toolName && !['Edit', 'Write'].includes(toolName)) {
  console.log(`[stdlib-hook] Skipping (${toolName} is not Edit/Write)`);
  process.exit(0);
}

const projectDir = process.env.CLAUDE_PROJECT_DIR || process.cwd();
const editedPath = getEditedPath(payload);
if (!editedPath) {
  console.log('[stdlib-hook] Skipping (no edited file path in hook payload)');
  process.exit(0);
}

const relPath = normalizeRelPath(projectDir, editedPath);
if (!shouldRunForPath(relPath)) {
  console.log(`[stdlib-hook] Skipping stdlib tests (path not relevant): ${relPath}`);
  process.exit(0);
}

console.log(`[stdlib-hook] Running stdlib tests (changed: ${relPath})`);
const result = spawnSync('pnpm', ['sigil:test:stdlib'], {
  cwd: projectDir,
  stdio: 'inherit',
  shell: process.platform === 'win32',
});

if (typeof result.status === 'number') {
  process.exit(result.status);
}

console.error('[stdlib-hook] Failed to run pnpm sigil:test:stdlib');
process.exit(1);
