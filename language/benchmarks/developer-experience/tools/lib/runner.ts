import { promises as fs } from 'node:fs';
import path from 'node:path';

import { buildCoverageReport } from './coverage.js';
import { createWorktree, removeWorktree, collectModifiedPaths, collectPatch, evaluatePathPolicy, prepareTaskWorkspace, cleanupWorkspace } from './workspace.js';
import { ensureDir, execShellCommand, median, writeJsonFile, writeTextFile } from './util.js';
import type {
  CompareSummary,
  Executor,
  FeatureManifest,
  RefPreparation,
  RefRunSummary,
  ShellCommandResult,
  TaskComparison,
  TaskManifest,
  TaskRunResult
} from './types.js';

type PrepareRefOptions = {
  repoRoot: string;
  runsLocalDir: string;
  refLabel: string;
  ref: string;
  sigilBinOverride?: string;
};

async function prepareReference(options: PrepareRefOptions): Promise<RefPreparation> {
  if (options.sigilBinOverride) {
    return {
      refLabel: options.refLabel,
      requestedRef: options.ref,
      resolvedRef: options.ref,
      worktreePath: null,
      sigilBin: options.sigilBinOverride
    };
  }

  const worktreeRoot = path.join(options.runsLocalDir, 'worktrees');
  const { worktreePath, resolvedRef } = await createWorktree(options.repoRoot, worktreeRoot, options.refLabel, options.ref);
  const build = await execShellCommand('cargo build --quiet --manifest-path language/compiler/Cargo.toml -p sigil-cli', worktreePath, {}, 1_800_000);

  if (build.exitCode !== 0) {
    await removeWorktree(options.repoRoot, worktreePath);
    throw new Error(`failed to build ref '${options.ref}': ${build.stderr || build.stdout}`);
  }

  return {
    refLabel: options.refLabel,
    requestedRef: options.ref,
    resolvedRef,
    worktreePath,
    sigilBin: path.join(worktreePath, 'language/compiler/target/debug/sigil')
  };
}

async function cleanupReference(repoRoot: string, preparation: RefPreparation): Promise<void> {
  if (preparation.worktreePath) {
    await removeWorktree(repoRoot, preparation.worktreePath);
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

export async function runTask(
  task: TaskManifest,
  fixturesDir: string,
  executor: Executor,
  reference: RefPreparation,
  runDir: string
): Promise<TaskRunResult> {
  const startedAt = Date.now();
  const workspacePath = await prepareTaskWorkspace(task, fixturesDir);
  const taskDir = path.join(runDir, 'tasks', task.id);
  const artifactDir = path.join(taskDir, reference.refLabel);
  const transcriptPath = path.join(artifactDir, 'transcript.jsonl');
  const diffPath = path.join(artifactDir, 'changes.diff');
  const finalResponsePath = path.join(artifactDir, 'final-response.json');

  const env = {
    SIGIL_BIN: reference.sigilBin,
    BENCH_TASK_ID: task.id
  };

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
  try {
    setupResults = await runCommands(task.setupCommands, workspacePath, env);
    const setupFailed = setupResults.some((result) => result.exitCode !== 0);

    if (!setupFailed) {
      const execution = await executor.run({
        task,
        workspacePath,
        runLabel: reference.refLabel,
        prompt: task.initialPrompt,
        env,
        timeoutMs: task.budgets.maxWallClockMs
      });

      executorError = execution.errorMessage ?? '';
      finalResponse = execution.finalResponse;
      usage = execution.usage;
      toolCounts = execution.toolCounts;

      await ensureDir(artifactDir);
      await writeTextFile(transcriptPath, `${execution.artifact.events.join('\n')}${execution.artifact.events.length > 0 ? '\n' : ''}`);
      await writeTextFile(path.join(artifactDir, 'executor.stderr.log'), execution.artifact.rawStderr);
      await writeTextFile(path.join(artifactDir, 'executor.stdout.log'), execution.artifact.rawStdout);
      await writeJsonFile(finalResponsePath, finalResponse ?? {});

      if (execution.exitCode === 0) {
        modifiedPaths = await collectModifiedPaths(workspacePath);
        patch = await collectPatch(workspacePath);
        pathPolicy = evaluatePathPolicy(task, modifiedPaths);
        oracleResults = await runCommands(task.oracleCommands, workspacePath, env);
      }
    }

    if (modifiedPaths.length === 0 && patch.stats.filesChanged === 0) {
      modifiedPaths = await collectModifiedPaths(workspacePath);
      patch = await collectPatch(workspacePath);
      pathPolicy = evaluatePathPolicy(task, modifiedPaths);
    }

    await ensureDir(artifactDir);
    await writeTextFile(diffPath, patch.diff);
    await saveOracleLogs(artifactDir, setupResults, oracleResults);

    const oracleFailed = oracleResults.some((result) => result.exitCode !== 0);

    let status: TaskRunResult['status'] = 'passed';
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

    const taskResult: TaskRunResult = {
      taskId: task.id,
      refLabel: reference.refLabel,
      ref: reference.resolvedRef,
      status,
      elapsedMs: Date.now() - startedAt,
      oracleResults,
      setupResults,
      modifiedPaths,
      patchStats: patch.stats,
      pathPolicy,
      usage,
      toolCounts,
      finalResponse,
      diagnosisTagsMatched,
      transcriptPath,
      diffPath,
      workspaceNote: workspacePath,
      errorMessage
    };

    await writeJsonFile(path.join(artifactDir, 'result.json'), taskResult);
    return taskResult;
  } finally {
    await cleanupWorkspace(workspacePath);
  }
}

export async function runTasksForReference(
  repoRoot: string,
  fixturesDir: string,
  executor: Executor,
  tasks: TaskManifest[],
  runDir: string,
  referenceOptions: PrepareRefOptions
): Promise<RefRunSummary> {
  const reference = await prepareReference(referenceOptions);

  try {
    const taskResults: TaskRunResult[] = [];

    for (const task of tasks) {
      taskResults.push(await runTask(task, fixturesDir, executor, reference, runDir));
    }

    return {
      refLabel: reference.refLabel,
      requestedRef: reference.requestedRef,
      resolvedRef: reference.resolvedRef,
      taskResults,
      passed: taskResults.filter((result) => result.status === 'passed').length,
      failed: taskResults.filter((result) => result.status === 'failed').length,
      errors: taskResults.filter((result) => result.status === 'error').length,
      medianElapsedMs: median(taskResults.map((result) => result.elapsedMs))
    };
  } finally {
    await cleanupReference(repoRoot, reference);
  }
}

function compareTask(baseResult: TaskRunResult, candidateResult: TaskRunResult): TaskComparison {
  if (baseResult.status !== candidateResult.status) {
    if (baseResult.status !== 'passed' && candidateResult.status === 'passed') {
      return { taskId: baseResult.taskId, baseStatus: baseResult.status, candidateStatus: candidateResult.status, direction: 'improved' };
    }
    if (baseResult.status === 'passed' && candidateResult.status !== 'passed') {
      return { taskId: baseResult.taskId, baseStatus: baseResult.status, candidateStatus: candidateResult.status, direction: 'regressed' };
    }
    return { taskId: baseResult.taskId, baseStatus: baseResult.status, candidateStatus: candidateResult.status, direction: 'mixed' };
  }

  if (baseResult.status === 'passed') {
    const candidateFaster = candidateResult.elapsedMs < baseResult.elapsedMs * 0.95;
    const candidateSlower = candidateResult.elapsedMs > baseResult.elapsedMs * 1.05;
    if (candidateFaster) {
      return { taskId: baseResult.taskId, baseStatus: baseResult.status, candidateStatus: candidateResult.status, direction: 'improved' };
    }
    if (candidateSlower) {
      return { taskId: baseResult.taskId, baseStatus: baseResult.status, candidateStatus: candidateResult.status, direction: 'regressed' };
    }
  }

  return { taskId: baseResult.taskId, baseStatus: baseResult.status, candidateStatus: candidateResult.status, direction: 'neutral' };
}

export function compareRefRuns(feature: FeatureManifest, coverage = buildCoverageReport(feature, []), base: RefRunSummary, candidate: RefRunSummary): CompareSummary {
  const taskComparisons = base.taskResults.map((baseResult) => {
    const candidateResult = candidate.taskResults.find((result) => result.taskId === baseResult.taskId);
    if (!candidateResult) {
      throw new Error(`candidate results are missing task '${baseResult.taskId}'`);
    }
    return compareTask(baseResult, candidateResult);
  });

  const directions = new Set(taskComparisons.map((comparison) => comparison.direction));
  let status: CompareSummary['status'] = 'neutral';

  if (!coverage.sufficient) {
    status = 'insufficient_coverage';
  } else if (directions.has('mixed') || (directions.has('improved') && directions.has('regressed'))) {
    status = 'mixed';
  } else if (candidate.passed > base.passed || directions.has('improved')) {
    status = 'improved';
  } else if (candidate.passed < base.passed || directions.has('regressed')) {
    status = 'regressed';
  }

  return {
    status,
    featureId: feature.featureId,
    taskIds: base.taskResults.map((result) => result.taskId),
    base,
    candidate,
    taskComparisons,
    coverage,
    generatedAt: new Date().toISOString()
  };
}
