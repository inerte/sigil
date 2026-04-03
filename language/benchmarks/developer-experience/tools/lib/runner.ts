import { promises as fs } from 'node:fs';
import path from 'node:path';

import {
  createWorkingTreeSnapshot,
  createWorktree,
  removeWorktree,
  collectModifiedPaths,
  collectPatch,
  evaluatePathPolicy,
  prepareTaskWorkspace,
  cleanupWorkspace
} from './workspace.js';
import { ensureDir, execShellCommand, median, writeJsonFile, writeTextFile } from './util.js';
import type {
  CompareSummary,
  Executor,
  ExecutorUsage,
  PhaseTimings,
  ReferenceSourceKind,
  RefPreparation,
  RefRunSummary,
  ShellCommandResult,
  TaskComparison,
  TaskManifest,
  TaskRunResult,
  TaskSampleResult
} from './types.js';

type PrepareRefOptions = {
  repoRoot: string;
  runsLocalDir: string;
  refLabel: string;
  ref?: string;
  sourceKind?: Extract<ReferenceSourceKind, 'ref' | 'worktree'>;
  sigilBinOverride?: string;
};

type CompareRefRunsOptions = {
  repeats?: number;
};

const MAX_CONCURRENT_REPEAT_PAIRS = 3;
const MAX_CONCURRENT_TASKS = 2;

function minimumDecisiveBudgetPassDelta(repeats: number): number {
  return repeats <= 2 ? 1 : 2;
}

function emptyPhaseTimings(): PhaseTimings {
  return {
    workspacePrepMs: 0,
    setupMs: 0,
    executorMs: 0,
    stateCollectionMs: 0,
    oracleMs: 0,
    artifactWriteMs: 0,
    overheadMs: 0
  };
}

async function prepareReference(options: PrepareRefOptions): Promise<RefPreparation> {
  const sourceKind = options.sourceKind ?? 'ref';

  if (options.sigilBinOverride) {
    return {
      refLabel: options.refLabel,
      sourceKind: 'binary',
      requestedRef: options.ref ?? 'SIGIL_BIN',
      resolvedRef: options.ref ?? 'SIGIL_BIN',
      preparationPath: null,
      sigilBin: options.sigilBinOverride
    };
  }

  if (sourceKind === 'worktree') {
    const snapshotRoot = path.join(options.runsLocalDir, 'snapshots');
    const { snapshotPath, resolvedRef } = await createWorkingTreeSnapshot(options.repoRoot, snapshotRoot, options.refLabel);
    const build = await execShellCommand('cargo build --quiet --manifest-path language/compiler/Cargo.toml -p sigil-cli', snapshotPath, {}, 1_800_000);

    if (build.exitCode !== 0) {
      await fs.rm(snapshotPath, { recursive: true, force: true });
      throw new Error(`failed to build current working tree: ${build.stderr || build.stdout}`);
    }

    return {
      refLabel: options.refLabel,
      sourceKind,
      requestedRef: 'WORKTREE',
      resolvedRef,
      preparationPath: snapshotPath,
      sigilBin: path.join(snapshotPath, 'language/compiler/target/debug/sigil')
    };
  }

  const requestedRef = options.ref ?? 'HEAD';
  const worktreeRoot = path.join(options.runsLocalDir, 'worktrees');
  const { worktreePath, resolvedRef } = await createWorktree(options.repoRoot, worktreeRoot, options.refLabel, requestedRef);
  const build = await execShellCommand('cargo build --quiet --manifest-path language/compiler/Cargo.toml -p sigil-cli', worktreePath, {}, 1_800_000);

  if (build.exitCode !== 0) {
    await removeWorktree(options.repoRoot, worktreePath);
    throw new Error(`failed to build ref '${requestedRef}': ${build.stderr || build.stdout}`);
  }

  return {
    refLabel: options.refLabel,
    sourceKind,
    requestedRef,
    resolvedRef,
    preparationPath: worktreePath,
    sigilBin: path.join(worktreePath, 'language/compiler/target/debug/sigil')
  };
}

async function cleanupReference(repoRoot: string, preparation: RefPreparation): Promise<void> {
  if (!preparation.preparationPath) {
    return;
  }

  if (preparation.sourceKind === 'ref') {
    await removeWorktree(repoRoot, preparation.preparationPath);
  } else {
    await fs.rm(preparation.preparationPath, { recursive: true, force: true });
  }
}

async function runCommands(commands: TaskManifest['setupCommands'], workspacePath: string, env: Record<string, string>): Promise<ShellCommandResult[]> {
  const results: ShellCommandResult[] = [];

  for (const command of commands) {
    const result = await execShellCommand(command.command, workspacePath, env, command.timeoutMs);
    results.push(result);
    if (result.exitCode !== 0) {
      break;
    }
  }

  return results;
}

async function saveOracleLogs(baseDir: string, setupResults: ShellCommandResult[], oracleResults: ShellCommandResult[]): Promise<void> {
  await ensureDir(baseDir);
  await writeJsonFile(path.join(baseDir, 'setup-results.json'), setupResults);
  await writeJsonFile(path.join(baseDir, 'oracle-results.json'), oracleResults);
}

function computeEffectiveTokens(usage: ExecutorUsage | null): number | null {
  if (!usage) {
    return null;
  }

  const inputTokens = usage.inputTokens ?? 0;
  const cachedInputTokens = usage.cachedInputTokens ?? 0;
  const outputTokens = usage.outputTokens ?? 0;
  return Math.max(inputTokens - cachedInputTokens, 0) + outputTokens;
}

function computeCommandExecutionCount(toolCounts: Record<string, number>): number | null {
  return Object.keys(toolCounts).length === 0
    ? null
    : (toolCounts['item:command_execution'] ?? 0);
}

function medianNullable(values: Array<number | null | undefined>): number | null {
  const numericValues = values.filter((value): value is number => typeof value === 'number' && Number.isFinite(value));
  return numericValues.length === 0 ? null : median(numericValues);
}

function medianPhaseTimings(samples: TaskSampleResult[]): PhaseTimings {
  return {
    workspacePrepMs: median(samples.map((sample) => sample.phaseTimings.workspacePrepMs)),
    setupMs: median(samples.map((sample) => sample.phaseTimings.setupMs)),
    executorMs: median(samples.map((sample) => sample.phaseTimings.executorMs)),
    stateCollectionMs: median(samples.map((sample) => sample.phaseTimings.stateCollectionMs)),
    oracleMs: median(samples.map((sample) => sample.phaseTimings.oracleMs)),
    artifactWriteMs: median(samples.map((sample) => sample.phaseTimings.artifactWriteMs)),
    overheadMs: median(samples.map((sample) => sample.phaseTimings.overheadMs))
  };
}

function finalizePhaseTimings(
  phaseTimings: Omit<PhaseTimings, 'overheadMs'>,
  elapsedMs: number
): PhaseTimings {
  const accountedMs = phaseTimings.workspacePrepMs
    + phaseTimings.setupMs
    + phaseTimings.executorMs
    + phaseTimings.stateCollectionMs
    + phaseTimings.oracleMs
    + phaseTimings.artifactWriteMs;

  return {
    ...phaseTimings,
    overheadMs: Math.max(elapsedMs - accountedMs, 0)
  };
}

function aggregateTaskStatus(samples: TaskSampleResult[]): TaskRunResult['status'] {
  const counts = {
    passed: samples.filter((sample) => sample.status === 'passed').length,
    failed: samples.filter((sample) => sample.status === 'failed').length,
    error: samples.filter((sample) => sample.status === 'error').length
  };

  if (counts.error > 0) {
    return 'error';
  }
  if (counts.failed > 0) {
    return 'failed';
  }
  return 'passed';
}

function ratio(count: number, total: number): number {
  return total === 0 ? 0 : Number((count / total).toFixed(4));
}

async function writeAggregateTaskResult(
  taskId: string,
  reference: RefPreparation,
  sampleResults: TaskSampleResult[],
  runDir: string
): Promise<TaskRunResult> {
  const rawPassCount = sampleResults.filter((sample) => sample.status === 'passed').length;
  const commandBudgetPassCount = sampleResults.filter((sample) => sample.status === 'passed' && sample.withinCommandBudget === true).length;
  const tokenBudgetPassCount = sampleResults.filter((sample) => sample.status === 'passed' && sample.withinTokenBudget === true).length;
  const budgetPassCount = sampleResults.filter((sample) => sample.withinAllBudgets).length;

  const aggregateResult: TaskRunResult = {
    taskId,
    refLabel: reference.refLabel,
    ref: reference.resolvedRef,
    status: aggregateTaskStatus(sampleResults),
    sampleCount: sampleResults.length,
    statusCounts: {
      passed: rawPassCount,
      failed: sampleResults.filter((sample) => sample.status === 'failed').length,
      error: sampleResults.filter((sample) => sample.status === 'error').length
    },
    rawPassCount,
    rawPassRate: ratio(rawPassCount, sampleResults.length),
    commandBudgetPassCount,
    commandBudgetPassRate: ratio(commandBudgetPassCount, sampleResults.length),
    tokenBudgetPassCount,
    tokenBudgetPassRate: ratio(tokenBudgetPassCount, sampleResults.length),
    budgetPassCount,
    budgetPassRate: ratio(budgetPassCount, sampleResults.length),
    medianElapsedMs: median(sampleResults.map((sample) => sample.elapsedMs)),
    medianEffectiveTokens: medianNullable(sampleResults.map((sample) => sample.effectiveTokens)),
    medianCommandExecutionCount: medianNullable(sampleResults.map((sample) => sample.commandExecutionCount)),
    medianPhaseTimings: sampleResults.length === 0 ? emptyPhaseTimings() : medianPhaseTimings(sampleResults),
    sampleResultPaths: sampleResults.map((sample) => sample.resultPath)
  };

  await writeJsonFile(path.join(runDir, 'tasks', taskId, reference.refLabel, 'result.json'), aggregateResult);
  return aggregateResult;
}

export async function runTaskSample(
  task: TaskManifest,
  fixturesDir: string,
  executor: Executor,
  reference: RefPreparation,
  runDir: string,
  sampleIndex: number
): Promise<TaskSampleResult> {
  const startedAt = Date.now();
  const languageRootPath = reference.preparationPath
    ? path.join(reference.preparationPath, 'language')
    : path.join(process.cwd(), 'language');
  const workspacePrepStartedAt = Date.now();
  const workspacePath = await prepareTaskWorkspace(task, fixturesDir, languageRootPath);
  const workspacePrepMs = Date.now() - workspacePrepStartedAt;
  const artifactDir = path.join(runDir, 'tasks', task.id, reference.refLabel, 'samples', String(sampleIndex));
  const transcriptPath = path.join(artifactDir, 'transcript.jsonl');
  const diffPath = path.join(artifactDir, 'changes.diff');
  const finalResponsePath = path.join(artifactDir, 'final-response.json');
  const resultPath = path.join(artifactDir, 'result.json');

  const env = {
    SIGIL_BIN: reference.sigilBin,
    BENCH_TASK_ID: task.id
  };

  let setupMs = 0;
  let executorMs = 0;
  let stateCollectionMs = 0;
  let oracleMs = 0;
  let artifactWriteMs = 0;
  let setupResults: ShellCommandResult[] = [];
  let oracleResults: ShellCommandResult[] = [];
  let executorError = '';
  let finalResponse = null;
  let usage = null;
  let toolCounts: Record<string, number> = {};
  let modifiedPaths: string[] = [];
  let patch = {
    diff: '',
    stats: {
      additions: 0,
      deletions: 0,
      filesChanged: 0
    }
  };
  let pathPolicy = {
    allowed: true,
    forbiddenMatches: [] as string[],
    outOfBoundsMatches: [] as string[]
  };
  let executionTempDir: string | null = null;

  try {
    const setupStartedAt = Date.now();
    setupResults = await runCommands(task.setupCommands, workspacePath, env);
    setupMs = Date.now() - setupStartedAt;
    const setupFailed = setupResults.some((result) => result.exitCode !== 0);

    if (!setupFailed) {
      const executorStartedAt = Date.now();
      const execution = await executor.run({
        task,
        workspacePath,
        runLabel: `${reference.refLabel}-sample-${sampleIndex}`,
        prompt: task.initialPrompt,
        env,
        timeoutMs: task.budgets.maxWallClockMs
      });
      executorMs = Date.now() - executorStartedAt;
      executionTempDir = execution.artifact.tempDir;

      executorError = execution.errorMessage ?? '';
      finalResponse = execution.finalResponse;
      usage = execution.usage;
      toolCounts = execution.toolCounts;

      const transcriptWriteStartedAt = Date.now();
      await ensureDir(artifactDir);
      await fs.copyFile(execution.artifact.stdoutPath, transcriptPath);
      await fs.copyFile(execution.artifact.stdoutPath, path.join(artifactDir, 'executor.stdout.log'));
      await fs.copyFile(execution.artifact.stderrPath, path.join(artifactDir, 'executor.stderr.log'));
      await writeJsonFile(finalResponsePath, finalResponse ?? {});
      artifactWriteMs += Date.now() - transcriptWriteStartedAt;

      if (execution.exitCode === 0) {
        const stateCollectionStartedAt = Date.now();
        modifiedPaths = await collectModifiedPaths(workspacePath);
        patch = await collectPatch(workspacePath);
        pathPolicy = evaluatePathPolicy(task, modifiedPaths);
        stateCollectionMs += Date.now() - stateCollectionStartedAt;

        const oracleStartedAt = Date.now();
        oracleResults = await runCommands(task.oracleCommands, workspacePath, env);
        oracleMs += Date.now() - oracleStartedAt;
      }
    }

    if (modifiedPaths.length === 0 && patch.stats.filesChanged === 0) {
      const stateCollectionStartedAt = Date.now();
      modifiedPaths = await collectModifiedPaths(workspacePath);
      patch = await collectPatch(workspacePath);
      pathPolicy = evaluatePathPolicy(task, modifiedPaths);
      stateCollectionMs += Date.now() - stateCollectionStartedAt;
    }

    const artifactWriteStartedAt = Date.now();
    await ensureDir(artifactDir);
    await writeTextFile(diffPath, patch.diff);
    await saveOracleLogs(artifactDir, setupResults, oracleResults);
    artifactWriteMs += Date.now() - artifactWriteStartedAt;

    const oracleFailed = oracleResults.some((result) => result.exitCode !== 0);

    let status: TaskSampleResult['status'] = 'passed';
    let errorMessage: string | undefined;

    if (setupFailed) {
      status = 'error';
      errorMessage = 'setup command failed';
    } else if (executorError) {
      status = 'error';
      errorMessage = executorError;
    } else if (!pathPolicy.allowed) {
      status = 'failed';
      errorMessage = 'path policy violation';
    } else if (oracleFailed) {
      status = 'failed';
      errorMessage = 'oracle command failed';
    }

    const diagnosisTagsMatched = finalResponse
      ? task.rootCauseTags.filter((tag) => finalResponse!.diagnosisTags.includes(tag))
      : [];
    const elapsedMs = Date.now() - startedAt;
    const phaseTimings = finalizePhaseTimings({
      workspacePrepMs,
      setupMs,
      executorMs,
      stateCollectionMs,
      oracleMs,
      artifactWriteMs
    }, elapsedMs);
    const commandExecutionCount = computeCommandExecutionCount(toolCounts);
    const effectiveTokens = computeEffectiveTokens(usage);
    const withinCommandBudget = commandExecutionCount === null
      ? null
      : commandExecutionCount <= task.budgets.maxCommandExecutions;
    const withinTokenBudget = effectiveTokens === null
      ? null
      : effectiveTokens <= task.budgets.maxEffectiveTokens;

    const taskResult: TaskSampleResult = {
      taskId: task.id,
      refLabel: reference.refLabel,
      ref: reference.resolvedRef,
      sampleIndex,
      status,
      elapsedMs,
      phaseTimings,
      oracleResults,
      setupResults,
      modifiedPaths,
      patchStats: patch.stats,
      pathPolicy,
      usage,
      toolCounts,
      commandExecutionCount,
      effectiveTokens,
      withinCommandBudget,
      withinTokenBudget,
      withinAllBudgets: status === 'passed' && withinCommandBudget === true && withinTokenBudget === true,
      finalResponse,
      diagnosisTagsMatched,
      transcriptPath,
      diffPath,
      resultPath,
      workspaceNote: workspacePath,
      errorMessage
    };

    await writeJsonFile(resultPath, taskResult);
    return taskResult;
  } finally {
    if (executionTempDir) {
      await fs.rm(executionTempDir, { recursive: true, force: true });
    }
    await cleanupWorkspace(workspacePath);
  }
}

async function runTaskWithRepeats(
  task: TaskManifest,
  fixturesDir: string,
  executor: Executor,
  reference: RefPreparation,
  runDir: string,
  repeats: number
): Promise<TaskRunResult> {
  const samples: TaskSampleResult[] = [];

  for (let sampleIndex = 1; sampleIndex <= repeats; sampleIndex += 1) {
    samples.push(await runTaskSample(task, fixturesDir, executor, reference, runDir, sampleIndex));
  }

  return writeAggregateTaskResult(task.id, reference, samples, runDir);
}

export async function runTasksForReference(
  repoRoot: string,
  fixturesDir: string,
  executor: Executor,
  tasks: TaskManifest[],
  runDir: string,
  referenceOptions: PrepareRefOptions,
  repeats = 3
): Promise<RefRunSummary> {
  const reference = await prepareReference(referenceOptions);

  try {
    const taskResults: TaskRunResult[] = [];

    for (const task of tasks) {
      taskResults.push(await runTaskWithRepeats(task, fixturesDir, executor, reference, runDir, repeats));
    }

    return summarizeReference(reference, taskResults);
  } finally {
    await cleanupReference(repoRoot, reference);
  }
}

function summarizeReference(reference: RefPreparation, taskResults: TaskRunResult[]): RefRunSummary {
  return {
    refLabel: reference.refLabel,
    sourceKind: reference.sourceKind,
    requestedRef: reference.requestedRef,
    resolvedRef: reference.resolvedRef,
    taskResults,
    passed: taskResults.filter((result) => result.status === 'passed').length,
    failed: taskResults.filter((result) => result.status === 'failed').length,
    errors: taskResults.filter((result) => result.status === 'error').length,
    rawPassTotal: taskResults.reduce((sum, result) => sum + result.rawPassCount, 0),
    budgetPassTotal: taskResults.reduce((sum, result) => sum + result.budgetPassCount, 0),
    medianElapsedMs: median(taskResults.map((result) => result.medianElapsedMs)),
    medianEffectiveTokens: medianNullable(taskResults.map((result) => result.medianEffectiveTokens)),
    medianCommandExecutionCount: medianNullable(taskResults.map((result) => result.medianCommandExecutionCount))
  };
}

export async function compareReferences(
  repoRoot: string,
  fixturesDir: string,
  executor: Executor,
  tasks: TaskManifest[],
  runDir: string,
  baseReferenceOptions: PrepareRefOptions,
  candidateReferenceOptions: PrepareRefOptions,
  repeats = 3
): Promise<CompareSummary> {
  const baseReference = await prepareReference(baseReferenceOptions);
  const candidateReference = await prepareReference(candidateReferenceOptions);

  try {
    const pairedResults: Array<{ base: TaskRunResult; candidate: TaskRunResult } | undefined> = new Array(tasks.length);
    let nextTaskIndex = 0;

    const runTaskComparison = async (task: TaskManifest): Promise<{ base: TaskRunResult; candidate: TaskRunResult }> => {
      const baseSamples: TaskSampleResult[] = [];
      const candidateSamples: TaskSampleResult[] = [];

      for (let sampleIndex = 1; sampleIndex <= repeats; sampleIndex += MAX_CONCURRENT_REPEAT_PAIRS) {
        const batchIndices = Array.from(
          { length: Math.min(MAX_CONCURRENT_REPEAT_PAIRS, repeats - sampleIndex + 1) },
          (_, offset) => sampleIndex + offset
        );

        const batchResults = await Promise.all(batchIndices.map(async (currentSampleIndex) => {
          const [baseSample, candidateSample] = await Promise.all([
            runTaskSample(task, fixturesDir, executor, baseReference, runDir, currentSampleIndex),
            runTaskSample(task, fixturesDir, executor, candidateReference, runDir, currentSampleIndex)
          ]);

          return { baseSample, candidateSample };
        }));

        for (const { baseSample, candidateSample } of batchResults) {
          baseSamples.push(baseSample);
          candidateSamples.push(candidateSample);
        }
      }

      baseSamples.sort((left, right) => left.sampleIndex - right.sampleIndex);
      candidateSamples.sort((left, right) => left.sampleIndex - right.sampleIndex);

      return {
        base: await writeAggregateTaskResult(task.id, baseReference, baseSamples, runDir),
        candidate: await writeAggregateTaskResult(task.id, candidateReference, candidateSamples, runDir)
      };
    };

    const worker = async (): Promise<void> => {
      while (nextTaskIndex < tasks.length) {
        const currentIndex = nextTaskIndex;
        nextTaskIndex += 1;
        pairedResults[currentIndex] = await runTaskComparison(tasks[currentIndex]);
      }
    };

    await Promise.all(Array.from(
      { length: Math.min(MAX_CONCURRENT_TASKS, tasks.length) },
      () => worker()
    ));

    const baseTaskResults = pairedResults.map((pair, index) => {
      if (!pair) {
        throw new Error(`missing base result for task '${tasks[index].id}'`);
      }
      return pair.base;
    });
    const candidateTaskResults = pairedResults.map((pair, index) => {
      if (!pair) {
        throw new Error(`missing candidate result for task '${tasks[index].id}'`);
      }
      return pair.candidate;
    });

    return compareRefRuns(
      summarizeReference(baseReference, baseTaskResults),
      summarizeReference(candidateReference, candidateTaskResults),
      { repeats }
    );
  } finally {
    await Promise.all([
      cleanupReference(repoRoot, baseReference),
      cleanupReference(repoRoot, candidateReference)
    ]);
  }
}

function compareTask(
  baseResult: TaskRunResult,
  candidateResult: TaskRunResult,
  minDecisiveDelta: number
): TaskComparison {
  const budgetPassDelta = candidateResult.budgetPassCount - baseResult.budgetPassCount;
  const direction = budgetPassDelta >= minDecisiveDelta
    ? 'improved'
    : budgetPassDelta <= -minDecisiveDelta
      ? 'regressed'
      : 'neutral';

  return {
    taskId: baseResult.taskId,
    baseStatus: baseResult.status,
    candidateStatus: candidateResult.status,
    direction,
    decisionBasis: direction === 'neutral' ? 'neutral' : 'budget_margin',
    budgetPassDelta,
    minDecisiveBudgetPassDelta: minDecisiveDelta,
    baseRawPassCount: baseResult.rawPassCount,
    candidateRawPassCount: candidateResult.rawPassCount,
    baseRawPassRate: baseResult.rawPassRate,
    candidateRawPassRate: candidateResult.rawPassRate,
    baseCommandBudgetPassCount: baseResult.commandBudgetPassCount,
    candidateCommandBudgetPassCount: candidateResult.commandBudgetPassCount,
    baseCommandBudgetPassRate: baseResult.commandBudgetPassRate,
    candidateCommandBudgetPassRate: candidateResult.commandBudgetPassRate,
    baseTokenBudgetPassCount: baseResult.tokenBudgetPassCount,
    candidateTokenBudgetPassCount: candidateResult.tokenBudgetPassCount,
    baseTokenBudgetPassRate: baseResult.tokenBudgetPassRate,
    candidateTokenBudgetPassRate: candidateResult.tokenBudgetPassRate,
    baseBudgetPassCount: baseResult.budgetPassCount,
    candidateBudgetPassCount: candidateResult.budgetPassCount,
    baseBudgetPassRate: baseResult.budgetPassRate,
    candidateBudgetPassRate: candidateResult.budgetPassRate,
    baseMedianEffectiveTokens: baseResult.medianEffectiveTokens,
    candidateMedianEffectiveTokens: candidateResult.medianEffectiveTokens,
    baseMedianCommandExecutionCount: baseResult.medianCommandExecutionCount,
    candidateMedianCommandExecutionCount: candidateResult.medianCommandExecutionCount
  };
}

export function compareRefRuns(base: RefRunSummary, candidate: RefRunSummary, options: CompareRefRunsOptions = {}): CompareSummary {
  const repeats = options.repeats ?? 3;
  const minDecisiveDelta = minimumDecisiveBudgetPassDelta(repeats);
  const taskComparisons = base.taskResults.map((baseResult) => {
    const candidateResult = candidate.taskResults.find((result) => result.taskId === baseResult.taskId);
    if (!candidateResult) {
      throw new Error(`candidate results are missing task '${baseResult.taskId}'`);
    }
    return compareTask(baseResult, candidateResult, minDecisiveDelta);
  });

  const directions = new Set(taskComparisons.map((comparison) => comparison.direction));
  let status: CompareSummary['status'] = 'neutral';

  if (directions.has('improved') && directions.has('regressed')) {
    status = 'mixed';
  } else if (directions.has('improved')) {
    status = 'improved';
  } else if (directions.has('regressed')) {
    status = 'regressed';
  }

  return {
    status,
    repeats,
    minDecisiveBudgetPassDelta: minDecisiveDelta,
    taskIds: base.taskResults.map((result) => result.taskId),
    base,
    candidate,
    taskComparisons,
    generatedAt: new Date().toISOString()
  };
}
