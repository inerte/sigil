import test from 'node:test';
import assert from 'node:assert/strict';
import os from 'node:os';
import path from 'node:path';
import { promises as fs } from 'node:fs';

import { CodexExecutor, MockExecutor, MockJudgeExecutor } from './lib/executor.js';
import { loadTaskManifest, loadTaskManifests } from './lib/manifests.js';
import { publishCompareRun } from './lib/publish.js';
import { compareRefRuns, compareReferences, runTasksForReference } from './lib/runner.js';
import { ensureDir, execShellCommand, writeJsonFile } from './lib/util.js';
import { createWorkingTreeSnapshot } from './lib/workspace.js';
import type { ExecutorResult, JudgeExecutorResult, TaskManifest, TaskRunResult } from './lib/types.js';

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

async function makeJudgeExecutionArtifact(stdout = '', stderr = ''): Promise<JudgeExecutorResult['artifact']> {
  return makeExecutionArtifact(stdout, stderr);
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

function makeJudgeResponse(winner: 'A' | 'B' | 'TIE') {
  return {
    winner,
    confidence: 'medium' as const,
    summary: `Winner: ${winner}`,
    task_completion: {
      A: 'completed' as const,
      B: 'completed' as const
    },
    diagnosis_quality: {
      A: 4,
      B: 3
    },
    edit_quality: {
      A: 4,
      B: 3
    },
    evidence_use: {
      A: 4,
      B: 3
    },
    key_reasons: ['kept edits focused'],
    evidence_citations: [
      {
        run: winner === 'B' ? 'B' as const : 'A' as const,
        artifact: 'changes.diff',
        fact: 'The winning run made the correct focused edit.'
      }
    ]
  };
}

test('current task manifests validate', async () => {
  const tasks = await loadTaskManifests(tasksDir);

  assert.equal(tasks.length, 16);
  assert.ok(tasks.some((task) => task.id === 'canonical-record-order-repair'));
  assert.ok(tasks.some((task) => task.id === 'canonical-stdlib-helper-repair'));
  assert.ok(tasks.some((task) => task.id === 'feed-description-propagation'));
  assert.ok(tasks.some((task) => task.id === 'event-import-pipeline-repair'));
  assert.ok(tasks.some((task) => task.id === 'homebrew-formula-test-repair'));
  assert.ok(tasks.some((task) => task.id === 'repair-ingest-received-timestamp'));
  assert.ok(tasks.some((task) => task.id === 'repair-feed-published-timestamp'));
  assert.ok(tasks.some((task) => task.id === 'site-route-canonicalization-repair'));
  assert.ok(tasks.some((task) => task.id === 'stats-summary-implementation'));
  assert.ok(tasks.some((task) => task.id === 'todo-domain-test-repair'));
  assert.ok(tasks.some((task) => task.id === 'todo-json-roundtrip-repair'));
  assert.ok(tasks.some((task) => task.id === 'topology-status-client-feature'));
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

test('compare defaults to three repeats, runs repeat pairs in bounded parallel batches, and stores one judge result per pair', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'sigil-devex-compare-batches-'));
  const fixturesDir = path.join(root, 'fixtures');
  const fixtureDir = path.join(fixturesDir, 'simple-pass');
  const runDir = path.join(root, '.local', 'runs', 'compare-batches');
  await ensureDir(fixtureDir);
  await fs.writeFile(path.join(fixtureDir, 'note.txt'), 'broken\n', 'utf8');

  let activeRuns = 0;
  let maxActiveRuns = 0;
  let judgeInvocations = 0;
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
  const judgeExecutor = new MockJudgeExecutor(async (): Promise<JudgeExecutorResult> => {
    judgeInvocations += 1;
    return {
      exitCode: 0,
      finalResponse: makeJudgeResponse('A'),
      usage: null,
      toolCounts: {},
      artifact: await makeJudgeExecutionArtifact('', '')
    };
  });

  const compare = await compareReferences(root, fixturesDir, executor, judgeExecutor, [makeSimpleTask()], runDir, {
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
  assert.equal(compare.taskJudgments[0].repeatJudgments.length, 3);
  assert.equal(judgeInvocations, 3);
  assert.ok(maxActiveRuns > 2);
  assert.ok(maxActiveRuns <= 6);
  await fs.access(path.join(runDir, 'tasks', 'simple-pass', 'judgments', '1', 'judge-result.json'));
  await fs.access(path.join(runDir, 'tasks', 'simple-pass', 'judgments', '1', 'judge-input.json'));
});

test('compare records a tied repeat when a judge run fails instead of aborting the compare', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'sigil-devex-judge-fallback-'));
  const fixturesDir = path.join(root, 'fixtures');
  const fixtureDir = path.join(fixturesDir, 'simple-pass');
  const runDir = path.join(root, '.local', 'runs', 'judge-fallback');
  await ensureDir(fixtureDir);
  await fs.writeFile(path.join(fixtureDir, 'note.txt'), 'broken\n', 'utf8');

  const executor = new MockExecutor(async (context): Promise<ExecutorResult> => {
    await fs.writeFile(path.join(context.workspacePath, 'fixed.txt'), 'ok\n', 'utf8');
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
  const judgeExecutor = new MockJudgeExecutor(async (): Promise<JudgeExecutorResult> => ({
    exitCode: 1,
    finalResponse: null,
    usage: null,
    toolCounts: {},
    artifact: await makeJudgeExecutionArtifact('judge wandered\n', ''),
    errorMessage: 'judge exited with code 1'
  }));

  const compare = await compareReferences(root, fixturesDir, executor, judgeExecutor, [makeSimpleTask()], runDir, {
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

  assert.equal(compare.taskJudgments[0].repeatTies, 3);
  assert.equal(compare.taskJudgments[0].taskLean, 'tie');
  assert.equal(compare.taskJudgments[0].repeatJudgments[0].judgeStatus, 'error');
  assert.match(compare.taskJudgments[0].repeatJudgments[0].summary, /Judge failed/);
  await fs.access(path.join(runDir, 'tasks', 'simple-pass', 'judgments', '1', 'judge-result.json'));
});

test('compare keeps two tasks in flight by starting the next task when one finishes', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'sigil-devex-task-pool-'));
  const fixturesDir = path.join(root, 'fixtures');
  const fixtureDir = path.join(fixturesDir, 'simple-pass');
  const runDir = path.join(root, '.local', 'runs', 'task-pool');
  await ensureDir(fixtureDir);
  await fs.writeFile(path.join(fixtureDir, 'note.txt'), 'broken\n', 'utf8');

  const activeTaskCounts = new Map<string, number>();
  let task3StartedWhileAnotherTaskActive = false;
  const executor = new MockExecutor(async (context): Promise<ExecutorResult> => {
    const currentCount = activeTaskCounts.get(context.task.id) ?? 0;
    activeTaskCounts.set(context.task.id, currentCount + 1);

    const otherActiveCount = Array.from(activeTaskCounts.entries())
      .filter(([taskId]) => taskId !== context.task.id)
      .reduce((sum, [, count]) => sum + count, 0);

    if (context.task.id === 'task-3' && otherActiveCount > 0) {
      task3StartedWhileAnotherTaskActive = true;
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
  const judgeExecutor = new MockJudgeExecutor(async (): Promise<JudgeExecutorResult> => ({
    exitCode: 0,
    finalResponse: makeJudgeResponse('A'),
    usage: null,
    toolCounts: {},
    artifact: await makeJudgeExecutionArtifact('', '')
  }));

  await compareReferences(root, fixturesDir, executor, judgeExecutor, [
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

  assert.equal(task3StartedWhileAnotherTaskActive, true);
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

test('compare summary is judge-first and rolls up task leans from repeat wins', () => {
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
    [
      {
        taskId: 'demo',
        baseStatus: 'passed',
        candidateStatus: 'passed',
        baseRawPassCount: 3,
        candidateRawPassCount: 2,
        baseBudgetPassCount: 1,
        candidateBudgetPassCount: 2,
        baselineRepeatWins: 0,
        compareRepeatWins: 2,
        repeatTies: 1,
        taskLean: 'candidate',
        repeatJudgments: [
          { repeatIndex: 1, resolvedWinner: 'candidate', judgeStatus: 'completed', confidence: 'medium', summary: 'candidate better', resultPath: '/tmp/j1.json' },
          { repeatIndex: 2, resolvedWinner: 'candidate', judgeStatus: 'completed', confidence: 'medium', summary: 'candidate better', resultPath: '/tmp/j2.json' },
          { repeatIndex: 3, resolvedWinner: 'TIE', judgeStatus: 'completed', confidence: 'low', summary: 'tie', resultPath: '/tmp/j3.json' }
        ]
      }
    ],
    { repeats: 3 }
  );

  assert.deepEqual(compare.taskIds, ['demo']);
  assert.equal(compare.taskJudgments[0].taskLean, 'candidate');
  assert.equal(compare.taskJudgments[0].compareRepeatWins, 2);
  assert.equal(compare.taskJudgments[0].repeatTies, 1);
  assert.equal(compare.suiteJudgment.compareTaskLeans, 1);
  assert.equal(compare.suiteJudgment.baselineTaskLeans, 0);
  assert.equal(compare.suiteJudgment.taskTies, 0);
});

test('task lean stays tied when repeat wins split evenly', () => {
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
    [
      {
        taskId: 'demo',
        baseStatus: 'passed',
        candidateStatus: 'failed',
        baseRawPassCount: 3,
        candidateRawPassCount: 1,
        baseBudgetPassCount: 1,
        candidateBudgetPassCount: 1,
        baselineRepeatWins: 1,
        compareRepeatWins: 1,
        repeatTies: 1,
        taskLean: 'tie',
        repeatJudgments: [
          { repeatIndex: 1, resolvedWinner: 'base', judgeStatus: 'completed', confidence: 'medium', summary: 'base better', resultPath: '/tmp/j1.json' },
          { repeatIndex: 2, resolvedWinner: 'candidate', judgeStatus: 'completed', confidence: 'medium', summary: 'candidate better', resultPath: '/tmp/j2.json' },
          { repeatIndex: 3, resolvedWinner: 'TIE', judgeStatus: 'completed', confidence: 'low', summary: 'tie', resultPath: '/tmp/j3.json' }
        ]
      }
    ],
    { repeats: 3 }
  );

  assert.equal(compare.taskJudgments[0].taskLean, 'tie');
  assert.equal(compare.suiteJudgment.taskTies, 1);
});

test('publish writes history and latest summary files with task lean totals', async () => {
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
  const compare = compareRefRuns(base, candidate, [
    {
      taskId: 'demo',
      baseStatus: 'passed',
      candidateStatus: 'passed',
      baseRawPassCount: 3,
      candidateRawPassCount: 3,
      baseBudgetPassCount: 2,
      candidateBudgetPassCount: 3,
      baselineRepeatWins: 0,
      compareRepeatWins: 2,
      repeatTies: 1,
      taskLean: 'candidate',
      repeatJudgments: [
        { repeatIndex: 1, resolvedWinner: 'candidate', judgeStatus: 'completed', confidence: 'medium', summary: 'candidate better', resultPath: '/tmp/j1.json' },
        { repeatIndex: 2, resolvedWinner: 'candidate', judgeStatus: 'completed', confidence: 'medium', summary: 'candidate better', resultPath: '/tmp/j2.json' },
        { repeatIndex: 3, resolvedWinner: 'TIE', judgeStatus: 'completed', confidence: 'low', summary: 'tie', resultPath: '/tmp/j3.json' }
      ]
    }
  ], { repeats: 3 });
  await writeJsonFile(path.join(runDir, 'compare.json'), compare);

  const published = await publishCompareRun(resultsDir, runDir, 'smoke-sample');

  assert.equal(published.label, 'smoke-sample');
  assert.equal(published.taskLeanTotals?.compare, 1);
  assert.equal(published.rawPassTotals?.base, 3);
  assert.equal(published.budgetPassTotals?.candidate, 3);
  assert.match(await fs.readFile(path.join(resultsDir, 'history.jsonl'), 'utf8'), /smoke-sample/);
  assert.match(await fs.readFile(path.join(resultsDir, 'LATEST.md'), 'utf8'), /Compare-leaning tasks/);
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
