#!/usr/bin/env node

/**
 * Smart validation hook for Sigil development.
 * Runs the narrowest useful Rust compiler checks for each edit.
 */

import { spawnSync } from 'node:child_process';
import path from 'node:path';
import fs from 'node:fs';

const COMPILER_MANIFEST = 'language/compiler/Cargo.toml';
const COMPILER_BINARY = 'language/compiler/target/debug/sigil';

function readStdin() {
  return new Promise((resolve, reject) => {
    let data = '';
    process.stdin.setEncoding('utf8');
    process.stdin.on('data', (chunk) => { data += chunk; });
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

function shouldSkipPath(relPath) {
  const skipPrefixes = [
    '.local/',
    'node_modules/',
    '.git/',
    'dist/',
    'build/',
  ];

  const skipExtensions = [
    '.md',
    '.json',
    '.map',
    '.log',
  ];

  for (const prefix of skipPrefixes) {
    if (relPath.startsWith(prefix)) return true;
  }

  for (const ext of skipExtensions) {
    if (relPath.endsWith(ext)) return true;
  }

  return false;
}

function getValidationPlan(relPath, projectDir) {
  const plan = {
    checks: [],
    description: '',
  };

  if (relPath.startsWith('language/compiler/crates/')) {
    const crateName = relPath.split('/')[3];
    plan.description = `Compiler change: ${crateName}`;
    plan.checks.push({
      label: 'Building compiler',
      cmd: 'cargo',
      args: ['build', '--manifest-path', COMPILER_MANIFEST, '-p', 'sigil-cli'],
      critical: true,
    });
    if (crateName && fs.existsSync(path.join(projectDir, `language/compiler/crates/${crateName}/Cargo.toml`))) {
      plan.checks.push({
        label: `Running ${crateName} tests`,
        cmd: 'cargo',
        args: ['test', '--manifest-path', COMPILER_MANIFEST, '-p', crateName],
      });
    } else {
      plan.checks.push({
        label: 'Running compiler tests',
        cmd: 'cargo',
        args: ['test', '--manifest-path', COMPILER_MANIFEST],
      });
    }
    return plan;
  }

  if (relPath === 'language/compiler/Cargo.toml' || relPath === 'language/compiler/Cargo.lock') {
    plan.description = 'Compiler workspace manifest change';
    plan.checks.push({
      label: 'Building compiler',
      cmd: 'cargo',
      args: ['build', '--manifest-path', COMPILER_MANIFEST, '-p', 'sigil-cli'],
      critical: true,
    });
    plan.checks.push({
      label: 'Running compiler tests',
      cmd: 'cargo',
      args: ['test', '--manifest-path', COMPILER_MANIFEST],
    });
    return plan;
  }

  if (relPath.startsWith('language/stdlib/')) {
    const moduleName = path.basename(relPath, '.sigil');
    plan.description = `Stdlib module: ${moduleName}`;
    plan.checks.push({
      label: 'Building compiler',
      cmd: 'cargo',
      args: ['build', '--manifest-path', COMPILER_MANIFEST, '-p', 'sigil-cli'],
      critical: true,
    });
    plan.checks.push({
      label: `Compiling ${moduleName}`,
      cmd: COMPILER_BINARY,
      args: ['compile', relPath],
      critical: true,
    });
    plan.checks.push({
      label: 'Running stdlib tests',
      cmd: COMPILER_BINARY,
      args: ['test', 'language/stdlib-tests/tests'],
    });
    return plan;
  }

  if (relPath.startsWith('language/examples/') && relPath.endsWith('.sigil')) {
    const exampleName = path.basename(relPath, '.sigil');
    plan.description = `Example: ${exampleName}`;
    plan.checks.push({
      label: 'Building compiler',
      cmd: 'cargo',
      args: ['build', '--manifest-path', COMPILER_MANIFEST, '-p', 'sigil-cli'],
      critical: true,
    });
    plan.checks.push({
      label: `Compiling ${exampleName}`,
      cmd: COMPILER_BINARY,
      args: ['compile', relPath],
      critical: true,
    });
    plan.checks.push({
      label: `Running ${exampleName}`,
      cmd: COMPILER_BINARY,
      args: ['run', relPath],
    });
    return plan;
  }

  if (relPath.startsWith('language/test-fixtures/')) {
    plan.description = 'Test fixture updated';
    plan.checks.push({
      label: 'Building compiler',
      cmd: 'cargo',
      args: ['build', '--manifest-path', COMPILER_MANIFEST, '-p', 'sigil-cli'],
      critical: true,
    });
    plan.checks.push({
      label: 'Checking fixture compiles/fails as expected',
      cmd: COMPILER_BINARY,
      args: ['compile', relPath],
      allowFailure: true,
    });
    plan.checks.push({
      label: 'Running canonical form tests',
      cmd: COMPILER_BINARY,
      args: ['test', 'language/integrationTests/tests/canonical.sigil'],
    });
    return plan;
  }

  if (relPath.startsWith('projects/')) {
    const projectMatch = relPath.match(/^projects\/([^/]+)/);
    const projectName = projectMatch ? projectMatch[1] : null;

    if (projectName && relPath.endsWith('.sigil')) {
      plan.description = `Project: ${projectName}`;
      plan.checks.push({
        label: 'Building compiler',
        cmd: 'cargo',
        args: ['build', '--manifest-path', COMPILER_MANIFEST, '-p', 'sigil-cli'],
        critical: true,
      });
      plan.checks.push({
        label: `Compiling ${relPath}`,
        cmd: COMPILER_BINARY,
        args: ['compile', relPath],
        critical: true,
      });
      const testPath = `projects/${projectName}/tests`;
      if (fs.existsSync(path.join(projectDir, testPath))) {
        plan.checks.push({
          label: `Running ${projectName} tests`,
          cmd: COMPILER_BINARY,
          args: ['test', testPath],
        });
      }
      return plan;
    }
  }

  if (relPath.startsWith('language/docs/') || relPath.startsWith('language/spec/')) {
    plan.description = 'Documentation updated (no tests)';
    return plan;
  }

  return null;
}

async function main() {
  const raw = await readStdin();
  if (!raw.trim()) {
    console.log('[validator] Skipping (no hook payload)');
    return 0;
  }

  let payload;
  try {
    payload = JSON.parse(raw);
  } catch {
    console.log('[validator] Skipping (invalid JSON payload)');
    return 0;
  }

  const toolName = getToolName(payload);
  if (toolName && !['Edit', 'Write'].includes(toolName)) {
    console.log(`[validator] Skipping (${toolName} not Edit/Write)`);
    return 0;
  }

  const projectDir = process.env.CLAUDE_PROJECT_DIR || process.cwd();
  const editedPath = getEditedPath(payload);
  if (!editedPath) {
    console.log('[validator] Skipping (no file path)');
    return 0;
  }

  const relPath = normalizeRelPath(projectDir, editedPath);
  if (shouldSkipPath(relPath)) {
    console.log(`[validator] Skipping (irrelevant file): ${relPath}`);
    return 0;
  }

  const plan = getValidationPlan(relPath, projectDir);
  if (!plan) {
    console.log(`[validator] No validation plan for: ${relPath}`);
    return 0;
  }

  if (plan.checks.length === 0) {
    console.log(`[validator] ${plan.description}`);
    return 0;
  }

  console.log('\n' + '='.repeat(60));
  console.log(`🎯 ${plan.description}`);
  console.log('   File: ' + relPath);
  console.log('='.repeat(60));

  let allPassed = true;

  for (const check of plan.checks) {
    const result = spawnSync(check.cmd, check.args, {
      cwd: projectDir,
      stdio: 'inherit',
      shell: process.platform === 'win32',
    });

    const passed = result.status === 0;
    if (!passed) {
      if (check.allowFailure) {
        console.log(`⚠️  ${check.label} - Failed (expected for some fixtures)`);
      } else {
        console.log(`❌ ${check.label} - FAILED`);
        allPassed = false;
        if (check.critical) {
          console.log('\n⛔ Critical check failed, skipping remaining checks');
          break;
        }
      }
    } else {
      console.log(`✅ ${check.label} - Passed`);
    }
  }

  console.log('='.repeat(60));
  if (allPassed) {
    console.log('✨ All validations passed!');
    return 0;
  }

  console.log('⚠️  Some validations failed');
  return 1;
}

main()
  .then(code => process.exit(code))
  .catch(err => {
    console.error('[validator] Unexpected error:', err);
    process.exit(1);
  });
