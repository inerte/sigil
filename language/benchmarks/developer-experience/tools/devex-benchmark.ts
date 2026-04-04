#!/usr/bin/env node

import path from 'node:path';

import { CodexExecutor, CodexJudgeExecutor } from './lib/executor.js';
import { loadTaskManifests } from './lib/manifests.js';
import { publishCompareRun } from './lib/publish.js';
import { compareReferences, runTasksForReference } from './lib/runner.js';
import { ensureDir, humanJson, runId, writeJsonFile } from './lib/util.js';
import type { TaskManifest } from './lib/types.js';

type ParsedOptions = {
  baseRef?: string;
  candidateRef?: string;
  executor?: string;
  label?: string;
  model?: string;
  ref?: string;
  repeats?: number;
  runPath?: string;
  sigilBin?: string;
  tasks?: string[];
};

const repoRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), '..', '..', '..', '..');
const benchmarkRoot = path.join(repoRoot, 'language/benchmarks/developer-experience');
const tasksDir = path.join(benchmarkRoot, 'tasks');
const fixturesDir = path.join(benchmarkRoot, 'fixtures');
const localRunsDir = path.join(benchmarkRoot, '.local', 'runs');
const resultsDir = path.join(benchmarkRoot, 'results');

function parseArgs(args: string[]): { command: string; options: ParsedOptions } {
  const [command, ...rest] = args;
  const options: ParsedOptions = {};

  for (let index = 0; index < rest.length; index += 1) {
    const current = rest[index];
    if (!current.startsWith('--')) {
      continue;
    }

    const key = current.slice(2);
    const next = rest[index + 1];
    index += 1;

    switch (key) {
      case 'base':
        options.baseRef = next;
        break;
      case 'candidate':
        options.candidateRef = next;
        break;
      case 'executor':
        options.executor = next;
        break;
      case 'label':
        options.label = next;
        break;
      case 'model':
        options.model = next;
        break;
      case 'ref':
        options.ref = next;
        break;
      case 'repeats': {
        const parsed = Number.parseInt(next, 10);
        if (!Number.isInteger(parsed) || parsed <= 0) {
          throw new Error(`--repeats must be a positive integer, got '${next}'`);
        }
        options.repeats = parsed;
        break;
      }
      case 'run':
        options.runPath = next;
        break;
      case 'sigil-bin':
        options.sigilBin = next;
        break;
      case 'tasks':
        options.tasks = next.split(',').map((entry) => entry.trim()).filter(Boolean);
        break;
      default:
        throw new Error(`unknown option '${current}'`);
    }
  }

  return {
    command: command ?? 'help',
    options
  };
}

async function selectTasks(selectedIds?: string[]): Promise<TaskManifest[]> {
  const tasks = await loadTaskManifests(tasksDir);
  if (!selectedIds || selectedIds.length === 0) {
    return tasks;
  }

  const byId = new Map(tasks.map((task) => [task.id, task]));
  return selectedIds.map((id) => {
    const manifest = byId.get(id);
    if (!manifest) {
      throw new Error(`unknown task id '${id}'`);
    }
    return manifest;
  });
}

async function cmdValidate(): Promise<void> {
  const tasks = await loadTaskManifests(tasksDir);

  console.log(humanJson({
    ok: true,
    taskCount: tasks.length
  }));
}

async function cmdRun(options: ParsedOptions): Promise<void> {
  const ref = options.ref;
  const sourceKind = ref ? 'ref' : 'worktree';
  const tasks = await selectTasks(options.tasks);
  const repeats = options.repeats ?? 3;
  const executor = new CodexExecutor({
    model: options.model
  });
  const runDirectory = path.join(localRunsDir, runId());
  await ensureDir(runDirectory);
  await writeJsonFile(path.join(runDirectory, 'meta.json'), {
    mode: 'run',
    createdAt: new Date().toISOString(),
    ref: ref ?? 'WORKTREE',
    sourceKind,
    taskIds: tasks.map((task) => task.id),
    repeats,
    executor: executor.kind
  });

  const summary = await runTasksForReference(repoRoot, fixturesDir, executor, tasks, runDirectory, {
    repoRoot,
    runsLocalDir: localRunsDir,
    refLabel: 'subject',
    ref,
    sourceKind,
    sigilBinOverride: options.sigilBin
  }, repeats);

  await writeJsonFile(path.join(runDirectory, 'run.json'), summary);
  console.log(humanJson({
    runDir: runDirectory,
    summary
  }));
}

async function cmdCompare(options: ParsedOptions): Promise<void> {
  const tasks = await selectTasks(options.tasks);
  const baseRef = options.baseRef ?? 'HEAD';
  const candidateRef = options.candidateRef;
  const candidateSourceKind = candidateRef ? 'ref' : 'worktree';
  const repeats = options.repeats ?? 3;
  const runDirectory = path.join(localRunsDir, runId());
  await ensureDir(runDirectory);
  await writeJsonFile(path.join(runDirectory, 'meta.json'), {
    mode: 'compare',
    createdAt: new Date().toISOString(),
    taskIds: tasks.map((task) => task.id),
    baseRef,
    candidateRef: candidateRef ?? 'WORKTREE',
    baseSourceKind: 'ref',
    candidateSourceKind,
    repeats,
    executor: options.executor ?? 'codex',
    judgeExecutor: 'codex-judge'
  });

  const executor = new CodexExecutor({
    model: options.model
  });
  const judgeExecutor = new CodexJudgeExecutor({
    model: options.model
  });

  const compare = await compareReferences(repoRoot, fixturesDir, executor, judgeExecutor, tasks, runDirectory, {
    repoRoot,
    runsLocalDir: localRunsDir,
    refLabel: 'base',
    ref: baseRef,
    sourceKind: 'ref'
  }, {
    repoRoot,
    runsLocalDir: localRunsDir,
    refLabel: 'candidate',
    ref: candidateRef,
    sourceKind: candidateSourceKind
  }, repeats);
  await writeJsonFile(path.join(runDirectory, 'compare.json'), compare);
  console.log(humanJson({
    runDir: runDirectory,
    compare
  }));
}

async function cmdPublish(options: ParsedOptions): Promise<void> {
  if (!options.runPath) {
    throw new Error('--run is required');
  }

  const runPath = path.isAbsolute(options.runPath)
    ? options.runPath
    : path.join(localRunsDir, options.runPath);

  const label = options.label ?? path.basename(runPath);
  const summary = await publishCompareRun(resultsDir, runPath, label);
  console.log(humanJson(summary));
}

function printHelp(): void {
  console.log(`Usage:
  pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts validate
  pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts compare [--base <ref>] [--candidate <ref>] [--tasks <id,id>] [--repeats <n>]
  pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts run [--ref <ref>] [--tasks <id,id>] [--repeats <n>]
  pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts publish --run <run-dir> [--label name]
  `);
}

async function main(): Promise<void> {
  const { command, options } = parseArgs(process.argv.slice(2));

  switch (command) {
    case 'validate':
      await cmdValidate();
      break;
    case 'compare':
      await cmdCompare(options);
      break;
    case 'publish':
      await cmdPublish(options);
      break;
    case 'run':
      await cmdRun(options);
      break;
    case 'help':
    default:
      printHelp();
      break;
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
