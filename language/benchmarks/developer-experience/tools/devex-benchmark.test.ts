import test from 'node:test';
import assert from 'node:assert/strict';
import os from 'node:os';
import path from 'node:path';
import { promises as fs } from 'node:fs';

import { CodexExecutor, MockExecutor } from './lib/executor.js';
import { loadTaskManifest, loadTaskManifests } from './lib/manifests.js';
import { publishCompareRun } from './lib/publish.js';
import { compareRefRuns, compareReferences, runTasksForReference } from './lib/runner.js';
import { ensureDir, execShellCommand, writeJsonFile } from './lib/util.js';
import { createWorkingTreeSnapshot } from './lib/workspace.js';
import type { ExecutorResult, TaskManifest, TaskRunResult } from './lib/types.js';

const benchmarkRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), '..');
const tasksDir = path.join(benchmarkRoot, 'tasks');

async function makeExecutionArtifact(stdout = '', stderr = ''): Promise<ExecutorResult['artifact']> {
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'sigil-devex-artifact-'));
  const stdoutPath = path.join(tempDir, 'executor.stdout.log');
  const stderrPath = path.join(tempDir, 'executor.stderr.log');
  await fs.writeFile(stdoutPath, stdout, 'utf8');
  await fs.writeFile(stderrPath, stderr, 'utf8');

  return {
    tempDir,
    stdoutPath,
    stderrPath,
    stdoutTail: stdout.slice(-2048),
    stderrTail: stderr.slice(-2048)
  };
}

function makeTaskRunResult(overrides: Partial<TaskRunResult> & Pick<TaskRunResult, 'taskId' | 'refLabel' | 'ref'>): TaskRunResult {
  return {
    taskId: overrides.taskId,
    refLabel: overrides.refLabel,
    ref: overrides.ref,
    status: overrides.status ?? 'passed',
    sampleCount: overrides.sampleCount ?? 3,
    statusCounts: overrides.statusCounts ?? { passed: 3, failed: 0, error: 0 },
    rawPassCount: overrides.rawPassCount ?? 3,
    rawPassRate: overrides.rawPassRate ?? 1,
    commandBudgetPassCount: overrides.commandBudgetPassCount ?? 2,
    commandBudgetPassRate: overrides.commandBudgetPassRate ?? 0.6667,
    tokenBudgetPassCount: overrides.tokenBudgetPassCount ?? 2,
    tokenBudgetPassRate: overrides.tokenBudgetPassRate ?? 0.6667,
    budgetPassCount: overrides.budgetPassCount ?? 2,
    budgetPassRate: overrides.budgetPassRate ?? 0.6667,
    medianElapsedMs: overrides.medianElapsedMs ?? 100,
    medianEffectiveTokens: overrides.medianEffectiveTokens ?? 1000,
    medianCommandExecutionCount: overrides.medianCommandExecutionCount ?? 10,
    medianPhaseTimings: overrides.medianPhaseTimings ?? {
      workspacePrepMs: 10,
      setupMs: 5,
      executorMs: 60,
      stateCollectionMs: 5,
      oracleMs: 10,
      artifactWriteMs: 5,
      overheadMs: 5
    },
    sampleResultPaths: overrides.sampleResultPaths ?? ['/tmp/sample-1.json', '/tmp/sample-2.json', '/tmp/sample-3.json']
  };
}

function makeSimpleTask(overrides: Partial<TaskManifest> = {}): TaskManifest {
  return {
    id: overrides.id ?? 'simple-pass',
    title: overrides.title ?? 'Simple pass task',
    goal: overrides.goal ?? 'Write a file that satisfies the oracle.',
    initialPrompt: overrides.initialPrompt ?? 'Create fixed.txt in the workspace.',
    fixture: overrides.fixture ?? 'simple-pass',
    setupCommands: overrides.setupCommands ?? [],
    oracleCommands: overrides.oracleCommands ?? [{ command: 'test -f fixed.txt' }],
    successCriteria: overrides.successCriteria ?? ['fixed.txt exists'],
    allowedEditPaths: overrides.allowedEditPaths ?? ['fixed.txt'],
    forbiddenEditPaths: overrides.forbiddenEditPaths ?? ['.local'],
    budgets: overrides.budgets ?? {
      maxCommandExecutions: 5,
      maxEffectiveTokens: 15,
      maxWallClockMs: 60_000
    },
    rootCauseTags: overrides.rootCauseTags ?? ['missing_output']
  };
}

test('current task manifests validate', async () => {
  const tasks = await loadTaskManifests(tasksDir);

  assert.equal(tasks.length, 12);
  assert.ok(tasks.some((task) => task.id === 'canonical-record-order-repair'));
  assert.ok(tasks.some((task) => task.id === 'canonical-stdlib-helper-repair'));
  assert.ok(tasks.some((task) => task.id === 'homebrew-formula-test-repair'));
  assert.ok(tasks.some((task) => task.id === 'repair-ingest-received-timestamp'));
  assert.ok(tasks.some((task) => task.id === 'repair-feed-published-timestamp'));
  assert.ok(tasks.some((task) => task.id === 'stats-summary-implementation'));
  assert.ok(tasks.some((task) => task.id === 'todo-domain-test-repair'));
  assert.ok(tasks.some((task) => task.id === 'todo-json-roundtrip-repair'));
});

test('manifest validation rejects legacy maxTurns-only budgets', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'sigil-devex-manifest-'));
  const manifestPath = path.join(root, 'legacy.json');
  await writeJsonFile(manifestPath, {
    ...makeSimpleTask(),
    budgets: {
      maxTurns: 5,
      maxWallClockMs: 60_000
    }
  });

  await assert.rejects(
    loadTaskManifest(manifestPath),
    /maxTurns.*no longer supported/
  );
});

test('runner aggregates raw and budget pass counts for repeated samples', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'sigil-devex-runner-'));
  const fixturesDir = path.join(root, 'fixtures');
  const fixtureDir = path.join(fixturesDir, 'simple-pass');
  const runDir = path.join(root, '.local', 'runs', 'sample-run');
  await ensureDir(fixtureDir);
  await fs.writeFile(path.join(fixtureDir, 'note.txt'), 'broken\n', 'utf8');
  let invocationCount = 0;

  const task = makeSimpleTask();
  const executor = new MockExecutor(async (context): Promise<ExecutorResult> => {
    invocationCount += 1;
    await fs.writeFile(path.join(context.workspacePath, 'fixed.txt'), 'ok\n', 'utf8');

    const usage = invocationCount === 1
      ? { inputTokens: 10, cachedInputTokens: 0, outputTokens: 5 }
      : { inputTokens: 18, cachedInputTokens: 0, outputTokens: 6 };
    const toolCounts = invocationCount === 1
      ? { 'item:command_execution': 3 }
      : { 'item:command_execution': 7 };

    return {
      exitCode: 0,
      finalResponse: {
        summary: 'Created fixed.txt.',
        diagnosis: 'The fixture was missing fixed.txt.',
        diagnosisTags: ['missing_output'],
        filesChanged: ['fixed.txt']
      },
      usage,
      toolCounts,
      artifact: await makeExecutionArtifact('', '')
    };
  });

  const summary = await runTasksForReference(root, fixturesDir, executor, [task], runDir, {
    repoRoot: root,
    runsLocalDir: path.join(root, '.local', 'runs'),
    refLabel: 'candidate',
    ref: 'HEAD',
    sourceKind: 'ref',
    sigilBinOverride: '/usr/bin/true'
  }, 2);

  const taskResult = summary.taskResults[0];
  assert.equal(taskResult.status, 'passed');
  assert.equal(taskResult.sampleCount, 2);
  assert.equal(taskResult.rawPassCount, 2);
  assert.equal(taskResult.commandBudgetPassCount, 1);
  assert.equal(taskResult.tokenBudgetPassCount, 1);
  assert.equal(taskResult.budgetPassCount, 1);
  assert.equal(taskResult.medianEffectiveTokens, 20);
  assert.equal(taskResult.medianCommandExecutionCount, 5);
  assert.equal(summary.rawPassTotal, 2);
  assert.equal(summary.budgetPassTotal, 1);
  await fs.access(taskResult.sampleResultPaths[0]);
  await fs.access(taskResult.sampleResultPaths[1]);

  const sampleResult = JSON.parse(await fs.readFile(taskResult.sampleResultPaths[0], 'utf8'));
  assert.equal(sampleResult.withinAllBudgets, true);
  assert.equal(sampleResult.withinCommandBudget, true);
  assert.equal(sampleResult.withinTokenBudget, true);
});

test('compare defaults to three repeats and runs repeat pairs in bounded parallel batches', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'sigil-devex-compare-batches-'));
  const fixturesDir = path.join(root, 'fixtures');
  const fixtureDir = path.join(fixturesDir, 'simple-pass');
  const runDir = path.join(root, '.local', 'runs', 'compare-batches');
  await ensureDir(fixtureDir);
  await fs.writeFile(path.join(fixtureDir, 'note.txt'), 'broken\n', 'utf8');

  let activeRuns = 0;
  let maxActiveRuns = 0;
  const executor = new MockExecutor(async (context): Promise<ExecutorResult> => {
    activeRuns += 1;
    maxActiveRuns = Math.max(maxActiveRuns, activeRuns);
    await new Promise((resolve) => setTimeout(resolve, 250));
    await fs.writeFile(path.join(context.workspacePath, 'fixed.txt'), 'ok\n', 'utf8');
    activeRuns -= 1;

    return {
      exitCode: 0,
      finalResponse: {
        summary: 'Created fixed.txt.',
        diagnosis: 'The fixture was missing fixed.txt.',
        diagnosisTags: ['missing_output'],
        filesChanged: ['fixed.txt']
      },
      usage: {
        inputTokens: 10,
        outputTokens: 5
      },
      toolCounts: {
        'item:command_execution': 1
      },
      artifact: await makeExecutionArtifact('', '')
    };
  });

  const compare = await compareReferences(root, fixturesDir, executor, [makeSimpleTask()], runDir, {
    repoRoot: root,
    runsLocalDir: path.join(root, '.local', 'runs'),
    refLabel: 'base',
    ref: 'HEAD',
    sourceKind: 'ref',
    sigilBinOverride: '/usr/bin/true'
  }, {
    repoRoot: root,
    runsLocalDir: path.join(root, '.local', 'runs'),
    refLabel: 'candidate',
    ref: 'HEAD',
    sourceKind: 'ref',
    sigilBinOverride: '/usr/bin/true'
  });

  assert.equal(compare.repeats, 3);
  assert.equal(compare.base.taskResults[0].sampleCount, 3);
  assert.equal(compare.candidate.taskResults[0].sampleCount, 3);
  assert.ok(maxActiveRuns > 2);
  assert.ok(maxActiveRuns <= 6);
});

test('compare keeps two tasks in flight by starting the next task when one finishes', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'sigil-devex-task-pool-'));
  const fixturesDir = path.join(root, 'fixtures');
  const fixtureDir = path.join(fixturesDir, 'simple-pass');
  const runDir = path.join(root, '.local', 'runs', 'task-pool');
  await ensureDir(fixtureDir);
  await fs.writeFile(path.join(fixtureDir, 'note.txt'), 'broken\n', 'utf8');

  const activeTaskCounts = new Map<string, number>();
  let task3StartedWhileTask1Active = false;
  const executor = new MockExecutor(async (context): Promise<ExecutorResult> => {
    const currentCount = activeTaskCounts.get(context.task.id) ?? 0;
    activeTaskCounts.set(context.task.id, currentCount + 1);

    if (context.task.id === 'task-3' && (activeTaskCounts.get('task-1') ?? 0) > 0) {
      task3StartedWhileTask1Active = true;
    }

    const delayMs = context.task.id === 'task-1' ? 1000 : 40;
    await new Promise((resolve) => setTimeout(resolve, delayMs));
    await fs.writeFile(path.join(context.workspacePath, 'fixed.txt'), 'ok\n', 'utf8');

    const remaining = (activeTaskCounts.get(context.task.id) ?? 1) - 1;
    if (remaining <= 0) {
      activeTaskCounts.delete(context.task.id);
    } else {
      activeTaskCounts.set(context.task.id, remaining);
    }

    return {
      exitCode: 0,
      finalResponse: {
        summary: 'Created fixed.txt.',
        diagnosis: 'The fixture was missing fixed.txt.',
        diagnosisTags: ['missing_output'],
        filesChanged: ['fixed.txt']
      },
      usage: {
        inputTokens: 10,
        outputTokens: 5
      },
      toolCounts: {
        'item:command_execution': 1
      },
      artifact: await makeExecutionArtifact('', '')
    };
  });

  await compareReferences(root, fixturesDir, executor, [
    makeSimpleTask({ id: 'task-1', title: 'Task 1' }),
    makeSimpleTask({ id: 'task-2', title: 'Task 2' }),
    makeSimpleTask({ id: 'task-3', title: 'Task 3' })
  ], runDir, {
    repoRoot: root,
    runsLocalDir: path.join(root, '.local', 'runs'),
    refLabel: 'base',
    ref: 'HEAD',
    sourceKind: 'ref',
    sigilBinOverride: '/usr/bin/true'
  }, {
    repoRoot: root,
    runsLocalDir: path.join(root, '.local', 'runs'),
    refLabel: 'candidate',
    ref: 'HEAD',
    sourceKind: 'ref',
    sigilBinOverride: '/usr/bin/true'
  }, 1);

  assert.equal(task3StartedWhileTask1Active, true);
});

test('working tree snapshots preserve uncommitted changes without copying ignored outputs', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'sigil-devex-snapshot-'));
  const localRoot = path.join(root, '.local', 'runs');
  await execShellCommand('git init -q', root, {});
  await execShellCommand('git config user.email "benchmarks@sigil.local"', root, {});
  await execShellCommand('git config user.name "Sigil Benchmarks"', root, {});
  await fs.writeFile(path.join(root, '.gitignore'), '.local/\n', 'utf8');
  await fs.writeFile(path.join(root, 'tracked.txt'), 'before\n', 'utf8');
  await fs.writeFile(path.join(root, 'keep.txt'), 'keep\n', 'utf8');
  await execShellCommand('git add .', root, {});
  await execShellCommand('git commit -qm "baseline"', root, {});

  await fs.writeFile(path.join(root, 'tracked.txt'), 'after\n', 'utf8');
  await fs.writeFile(path.join(root, 'added.txt'), 'new\n', 'utf8');
  await fs.rm(path.join(root, 'keep.txt'));
  await fs.mkdir(path.join(root, '.local'), { recursive: true });
  await fs.writeFile(path.join(root, '.local', 'ignored.txt'), 'ignored\n', 'utf8');
  const snapshot = await createWorkingTreeSnapshot(root, localRoot, 'candidate');

  assert.match(snapshot.resolvedRef, /\+worktree$/);
  assert.equal(await fs.readFile(path.join(snapshot.snapshotPath, 'tracked.txt'), 'utf8'), 'after\n');
  assert.equal(await fs.readFile(path.join(snapshot.snapshotPath, 'added.txt'), 'utf8'), 'new\n');
  await assert.rejects(fs.readFile(path.join(snapshot.snapshotPath, 'keep.txt'), 'utf8'));
  await assert.rejects(fs.readFile(path.join(snapshot.snapshotPath, '.local', 'ignored.txt'), 'utf8'));
});

test('compare summary keeps a one-sample budget swing neutral at the default three repeats', () => {
  const compare = compareRefRuns(
    {
      refLabel: 'base',
      sourceKind: 'ref',
      requestedRef: 'HEAD',
      resolvedRef: 'aaa111',
      taskResults: [
        makeTaskRunResult({ taskId: 'demo', refLabel: 'base', ref: 'aaa111', rawPassCount: 3, budgetPassCount: 1, budgetPassRate: 0.3333 })
      ],
      passed: 1,
      failed: 0,
      errors: 0,
      rawPassTotal: 3,
      budgetPassTotal: 1,
      medianElapsedMs: 100,
      medianEffectiveTokens: 1000,
      medianCommandExecutionCount: 10
    },
    {
      refLabel: 'candidate',
      sourceKind: 'worktree',
      requestedRef: 'WORKTREE',
      resolvedRef: 'bbb222+worktree',
      taskResults: [
        makeTaskRunResult({ taskId: 'demo', refLabel: 'candidate', ref: 'bbb222+worktree', rawPassCount: 2, budgetPassCount: 2, budgetPassRate: 0.6667 })
      ],
      passed: 1,
      failed: 0,
      errors: 0,
      rawPassTotal: 2,
      budgetPassTotal: 2,
      medianElapsedMs: 120,
      medianEffectiveTokens: 1200,
      medianCommandExecutionCount: 12
    },
    { repeats: 3 }
  );

  assert.equal(compare.status, 'neutral');
  assert.deepEqual(compare.taskIds, ['demo']);
  assert.equal(compare.minDecisiveBudgetPassDelta, 2);
  assert.equal(compare.taskComparisons[0].direction, 'neutral');
  assert.equal(compare.taskComparisons[0].decisionBasis, 'neutral');
  assert.equal(compare.taskComparisons[0].budgetPassDelta, 1);
  assert.equal(compare.taskComparisons[0].minDecisiveBudgetPassDelta, 2);
  assert.equal(compare.taskComparisons[0].baseRawPassCount, 3);
  assert.equal(compare.taskComparisons[0].candidateBudgetPassCount, 2);
});

test('raw pass differences stay diagnostic when budget pass counts are tied', () => {
  const compare = compareRefRuns(
    {
      refLabel: 'base',
      sourceKind: 'ref',
      requestedRef: 'HEAD',
      resolvedRef: 'aaa111',
      taskResults: [
        makeTaskRunResult({ taskId: 'demo', refLabel: 'base', ref: 'aaa111', rawPassCount: 3, budgetPassCount: 1, budgetPassRate: 0.3333 })
      ],
      passed: 1,
      failed: 0,
      errors: 0,
      rawPassTotal: 3,
      budgetPassTotal: 1,
      medianElapsedMs: 100,
      medianEffectiveTokens: 1000,
      medianCommandExecutionCount: 10
    },
    {
      refLabel: 'candidate',
      sourceKind: 'worktree',
      requestedRef: 'WORKTREE',
      resolvedRef: 'bbb222+worktree',
      taskResults: [
        makeTaskRunResult({ taskId: 'demo', refLabel: 'candidate', ref: 'bbb222+worktree', rawPassCount: 1, budgetPassCount: 1, budgetPassRate: 0.3333 })
      ],
      passed: 0,
      failed: 1,
      errors: 0,
      rawPassTotal: 1,
      budgetPassTotal: 1,
      medianElapsedMs: 140,
      medianEffectiveTokens: 1500,
      medianCommandExecutionCount: 14
    },
    { repeats: 3 }
  );

  assert.equal(compare.status, 'neutral');
  assert.equal(compare.taskComparisons[0].direction, 'neutral');
  assert.equal(compare.taskComparisons[0].decisionBasis, 'neutral');
  assert.equal(compare.taskComparisons[0].budgetPassDelta, 0);
});

test('compare summary uses a larger budget-pass margin as the decisive signal at three repeats', () => {
  const compare = compareRefRuns(
    {
      refLabel: 'base',
      sourceKind: 'ref',
      requestedRef: 'HEAD',
      resolvedRef: 'aaa111',
      taskResults: [
        makeTaskRunResult({ taskId: 'demo', refLabel: 'base', ref: 'aaa111', rawPassCount: 3, budgetPassCount: 1, budgetPassRate: 0.3333 })
      ],
      passed: 1,
      failed: 0,
      errors: 0,
      rawPassTotal: 3,
      budgetPassTotal: 1,
      medianElapsedMs: 100,
      medianEffectiveTokens: 1000,
      medianCommandExecutionCount: 10
    },
    {
      refLabel: 'candidate',
      sourceKind: 'worktree',
      requestedRef: 'WORKTREE',
      resolvedRef: 'bbb222+worktree',
      taskResults: [
        makeTaskRunResult({ taskId: 'demo', refLabel: 'candidate', ref: 'bbb222+worktree', rawPassCount: 3, budgetPassCount: 3, budgetPassRate: 1 })
      ],
      passed: 1,
      failed: 0,
      errors: 0,
      rawPassTotal: 3,
      budgetPassTotal: 3,
      medianElapsedMs: 120,
      medianEffectiveTokens: 1200,
      medianCommandExecutionCount: 12
    },
    { repeats: 3 }
  );

  assert.equal(compare.status, 'improved');
  assert.equal(compare.minDecisiveBudgetPassDelta, 2);
  assert.equal(compare.taskComparisons[0].direction, 'improved');
  assert.equal(compare.taskComparisons[0].decisionBasis, 'budget_margin');
  assert.equal(compare.taskComparisons[0].budgetPassDelta, 2);
});

test('single-sample smoke compares still allow a one-sample budget delta to decide direction', () => {
  const compare = compareRefRuns(
    {
      refLabel: 'base',
      sourceKind: 'ref',
      requestedRef: 'HEAD',
      resolvedRef: 'aaa111',
      taskResults: [
        makeTaskRunResult({
          taskId: 'demo',
          refLabel: 'base',
          ref: 'aaa111',
          sampleCount: 1,
          statusCounts: { passed: 1, failed: 0, error: 0 },
          rawPassCount: 1,
          rawPassRate: 1,
          commandBudgetPassCount: 0,
          commandBudgetPassRate: 0,
          tokenBudgetPassCount: 0,
          tokenBudgetPassRate: 0,
          budgetPassCount: 0,
          budgetPassRate: 0,
          sampleResultPaths: ['/tmp/sample-1.json']
        })
      ],
      passed: 1,
      failed: 0,
      errors: 0,
      rawPassTotal: 1,
      budgetPassTotal: 0,
      medianElapsedMs: 100,
      medianEffectiveTokens: 1000,
      medianCommandExecutionCount: 10
    },
    {
      refLabel: 'candidate',
      sourceKind: 'worktree',
      requestedRef: 'WORKTREE',
      resolvedRef: 'bbb222+worktree',
      taskResults: [
        makeTaskRunResult({
          taskId: 'demo',
          refLabel: 'candidate',
          ref: 'bbb222+worktree',
          sampleCount: 1,
          statusCounts: { passed: 1, failed: 0, error: 0 },
          rawPassCount: 1,
          rawPassRate: 1,
          commandBudgetPassCount: 1,
          commandBudgetPassRate: 1,
          tokenBudgetPassCount: 1,
          tokenBudgetPassRate: 1,
          budgetPassCount: 1,
          budgetPassRate: 1,
          sampleResultPaths: ['/tmp/sample-1.json']
        })
      ],
      passed: 1,
      failed: 0,
      errors: 0,
      rawPassTotal: 1,
      budgetPassTotal: 1,
      medianElapsedMs: 80,
      medianEffectiveTokens: 900,
      medianCommandExecutionCount: 8
    },
    { repeats: 1 }
  );

  assert.equal(compare.status, 'improved');
  assert.equal(compare.minDecisiveBudgetPassDelta, 1);
  assert.equal(compare.taskComparisons[0].direction, 'improved');
  assert.equal(compare.taskComparisons[0].decisionBasis, 'budget_margin');
  assert.equal(compare.taskComparisons[0].budgetPassDelta, 1);
});

test('publish writes history and latest summary files with raw and budget pass totals', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'sigil-devex-publish-'));
  const resultsDir = path.join(root, 'results');
  const runDir = path.join(root, '.local', 'runs', 'publish-sample');
  await ensureDir(runDir);

  const base = {
    refLabel: 'base',
    sourceKind: 'ref' as const,
    requestedRef: 'main',
    resolvedRef: 'aaa111',
    taskResults: [makeTaskRunResult({ taskId: 'demo', refLabel: 'base', ref: 'aaa111' })],
    passed: 1,
    failed: 0,
    errors: 0,
    rawPassTotal: 3,
    budgetPassTotal: 2,
    medianElapsedMs: 100,
    medianEffectiveTokens: 1000,
    medianCommandExecutionCount: 10
  };
  const candidate = {
    refLabel: 'candidate',
    sourceKind: 'worktree' as const,
    requestedRef: 'WORKTREE',
    resolvedRef: 'bbb222+worktree',
    taskResults: [makeTaskRunResult({ taskId: 'demo', refLabel: 'candidate', ref: 'bbb222+worktree', budgetPassCount: 3, budgetPassRate: 1 })],
    passed: 1,
    failed: 0,
    errors: 0,
    rawPassTotal: 3,
    budgetPassTotal: 3,
    medianElapsedMs: 80,
    medianEffectiveTokens: 900,
    medianCommandExecutionCount: 8
  };
  const compare = compareRefRuns(base, candidate, { repeats: 3 });
  await writeJsonFile(path.join(runDir, 'compare.json'), compare);

  const published = await publishCompareRun(resultsDir, runDir, 'smoke-sample');

  assert.equal(published.label, 'smoke-sample');
  assert.equal(published.rawPassTotals?.base, 3);
  assert.equal(published.budgetPassTotals?.candidate, 3);
  assert.match(await fs.readFile(path.join(resultsDir, 'history.jsonl'), 'utf8'), /smoke-sample/);
  assert.match(await fs.readFile(path.join(resultsDir, 'LATEST.md'), 'utf8'), /budget passes/);
});

test('codex executor streams large logs without buffering them into one giant string', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'sigil-devex-executor-'));
  const fakeCodexPath = path.join(root, 'fake-codex');
  const script = `#!/usr/bin/env node
const fs = require('fs');
const args = process.argv.slice(2);
const outputIndex = args.indexOf('-o');
const outputPath = outputIndex === -1 ? null : args[outputIndex + 1];
if (!outputPath) process.exit(2);
fs.writeFileSync(outputPath, JSON.stringify({
  summary: 'ok',
  diagnosis: 'ok',
  diagnosisTags: ['test'],
  filesChanged: []
}));
process.stdout.write(JSON.stringify({ type: 'item.completed', item: { type: 'command_execution' } }) + '\\n');
process.stdout.write(JSON.stringify({ type: 'turn.completed', usage: { input_tokens: 11, cached_input_tokens: 1, output_tokens: 7 } }) + '\\n');
process.stderr.write('x'.repeat(200000));
process.exit(0);
`;
  await fs.writeFile(fakeCodexPath, script, { encoding: 'utf8', mode: 0o755 });

  const executor = new CodexExecutor({ codexBin: fakeCodexPath });
  const result = await executor.run({
    task: makeSimpleTask(),
    workspacePath: root,
    runLabel: 'sample-1',
    prompt: 'noop',
    env: { SIGIL_BIN: '/usr/bin/true' },
    timeoutMs: 60_000
  });

  assert.equal(result.exitCode, 0);
  assert.equal(result.toolCounts['item:command_execution'], 1);
  assert.equal(result.usage?.inputTokens, 11);
  assert.equal(result.usage?.cachedInputTokens, 1);
  assert.equal(result.usage?.outputTokens, 7);
  assert.ok(result.artifact.stderrTail.length <= 16_384);
  await fs.access(result.artifact.stdoutPath);
  await fs.access(result.artifact.stderrPath);
  await fs.rm(result.artifact.tempDir, { recursive: true, force: true });
});
