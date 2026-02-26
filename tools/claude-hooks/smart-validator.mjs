#!/usr/bin/env node

/**
 * Smart validation hook for Sigil development
 * Runs only the most relevant tests/checks for each file change
 */

import { spawnSync } from 'node:child_process';
import path from 'node:path';
import fs from 'node:fs';

// ===== Helpers =====

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

function run(cmd, args, cwd, label) {
  console.log(`\n🔍 ${label}`);
  const result = spawnSync(cmd, args, {
    cwd,
    stdio: 'inherit',
    shell: process.platform === 'win32',
  });
  return result.status === 0;
}

// ===== Validation Strategies =====

function shouldSkipPath(relPath) {
  // Skip these paths entirely
  const skipPrefixes = [
    '.local/',
    'node_modules/',
    '.git/',
    'dist/',
    'build/',
  ];

  const skipExtensions = [
    '.md',      // Markdown docs
    '.json',    // Config files (usually)
    '.map',     // Source maps
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

  // ===== COMPILER SOURCE =====
  if (relPath.startsWith('language/compiler/src/')) {
    const component = relPath.split('/')[3]; // lexer, parser, validator, etc.

    plan.description = `Compiler change: ${component}`;

    // Quick syntax check first
    plan.checks.push({
      label: 'Checking TypeScript compilation',
      cmd: 'pnpm',
      args: ['--filter', '@sigil-lang/compiler', 'build'],
      critical: true, // If this fails, skip other tests
    });

    // Run unit tests for the specific component if possible
    if (component && fs.existsSync(path.join(projectDir, `language/compiler/test/${component}.test.ts`))) {
      plan.checks.push({
        label: `Running ${component} unit tests`,
        cmd: 'pnpm',
        args: ['--filter', '@sigil-lang/compiler', 'exec', 'node', '--import', 'tsx', '--test', `test/${component}.test.ts`],
      });
    } else {
      plan.checks.push({
        label: 'Running all compiler unit tests',
        cmd: 'pnpm',
        args: ['--filter', '@sigil-lang/compiler', 'test:unit'],
      });
    }

    return plan;
  }

  // ===== STDLIB =====
  if (relPath.startsWith('language/stdlib/')) {
    const moduleName = path.basename(relPath, '.sigil');

    plan.description = `Stdlib module: ${moduleName}`;
    plan.checks.push({
      label: `Compiling ${moduleName}`,
      cmd: 'node',
      args: ['language/compiler/dist/cli.js', 'compile', relPath],
      critical: true,
    });

    plan.checks.push({
      label: 'Running stdlib tests',
      cmd: 'pnpm',
      args: ['sigil:test:stdlib'],
    });

    return plan;
  }

  // ===== EXAMPLES =====
  if (relPath.startsWith('language/examples/') && relPath.endsWith('.sigil')) {
    const exampleName = path.basename(relPath, '.sigil');

    plan.description = `Example: ${exampleName}`;
    plan.checks.push({
      label: `Compiling ${exampleName}`,
      cmd: 'node',
      args: ['language/compiler/dist/cli.js', 'compile', relPath],
      critical: true,
    });

    plan.checks.push({
      label: `Running ${exampleName}`,
      cmd: 'node',
      args: ['language/compiler/dist/cli.js', 'run', relPath],
    });

    return plan;
  }

  // ===== TEST FIXTURES =====
  if (relPath.startsWith('language/test-fixtures/')) {
    plan.description = 'Test fixture updated';

    // Try to compile it first (may intentionally fail for negative tests)
    plan.checks.push({
      label: 'Checking fixture compiles/fails as expected',
      cmd: 'node',
      args: ['language/compiler/dist/cli.js', 'compile', relPath],
      allowFailure: true, // Some fixtures are meant to fail
    });

    plan.checks.push({
      label: 'Running canonical form tests',
      cmd: 'bash',
      args: ['language/test-canonical.sh'],
    });

    return plan;
  }

  // ===== PROJECT CODE =====
  if (relPath.startsWith('projects/')) {
    const projectMatch = relPath.match(/^projects\/([^\/]+)/);
    const projectName = projectMatch ? projectMatch[1] : null;

    if (projectName && relPath.endsWith('.sigil')) {
      plan.description = `Project: ${projectName}`;

      plan.checks.push({
        label: `Compiling ${relPath}`,
        cmd: 'node',
        args: ['language/compiler/dist/cli.js', 'compile', relPath],
        critical: true,
      });

      // If project has tests, run them
      const testPath = `projects/${projectName}/tests`;
      if (fs.existsSync(path.join(projectDir, testPath))) {
        plan.checks.push({
          label: `Running ${projectName} tests`,
          cmd: 'node',
          args: ['language/compiler/dist/cli.js', 'test', testPath],
        });
      }

      return plan;
    }
  }

  // ===== DOCS/SPEC =====
  if (relPath.startsWith('language/docs/') || relPath.startsWith('language/spec/')) {
    plan.description = 'Documentation updated (no tests)';
    plan.checks = []; // No validation needed for docs
    return plan;
  }

  // Default: unknown file
  return null;
}

// ===== Main =====

async function main() {
  const raw = await readStdin();
  if (!raw.trim()) {
    console.log('[validator] Skipping (no hook payload)');
    return 0;
  }

  let payload;
  try {
    payload = JSON.parse(raw);
  } catch (err) {
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

  // Check if we should skip this path
  if (shouldSkipPath(relPath)) {
    console.log(`[validator] Skipping (irrelevant file): ${relPath}`);
    return 0;
  }

  // Get validation plan
  const plan = getValidationPlan(relPath, projectDir);

  if (!plan) {
    console.log(`[validator] No validation plan for: ${relPath}`);
    return 0;
  }

  if (plan.checks.length === 0) {
    console.log(`[validator] ${plan.description}`);
    return 0;
  }

  // Execute validation plan
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
  } else {
    console.log('⚠️  Some validations failed');
    return 1;
  }
}

main()
  .then(code => process.exit(code))
  .catch(err => {
    console.error('[validator] Unexpected error:', err);
    process.exit(1);
  });
