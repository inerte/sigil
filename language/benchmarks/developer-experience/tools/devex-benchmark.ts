#!/usr/bin/env node

import { promises as fs } from 'node:fs';
import path from 'node:path';

import { buildCoverageReport, proposeTasks } from './lib/coverage.js';
import { CodexExecutor } from './lib/executor.js';
import { loadFeatureManifest, loadTaskManifest, loadTaskManifests } from './lib/manifests.js';
import { publishCompareRun } from './lib/publish.js';
import { compareRefRuns, runTasksForReference } from './lib/runner.js';
import { ensureDir, humanJson, runId, writeJsonFile } from './lib/util.js';
import type { FeatureManifest, TaskManifest } from './lib/types.js';

type ParsedOptions = {
  baseRef?: string;
  candidateRef?: string;
  executor?: string;
  featurePath?: string;
  label?: string;
  model?: string;
  ref?: string;
  runPath?: string;
  sigilBin?: string;
  tasks?: string[];
  write?: boolean;
};

const repoRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), '..', '..', '..', '..');
const benchmarkRoot = path.join(repoRoot, 'language/benchmarks/developer-experience');
const tasksDir = path.join(benchmarkRoot, 'tasks');
const fixturesDir = path.join(benchmarkRoot, 'fixtures');
const featuresDir = path.join(benchmarkRoot, 'features');
const localRunsDir = path.join(benchmarkRoot, '.local', 'runs');
const proposalsDir = path.join(benchmarkRoot, '.local', 'proposals');
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
    if (key === 'write') {
      options.write = true;
      continue;
    }

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
      case 'feature':
        options.featurePath = next;
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
  const selected = selectedIds.map((id) => {
    const manifest = byId.get(id);
    if (!manifest) {
      throw new Error(`unknown task id '${id}'`);
    }
    return manifest;
  });

  return selected;
}

async function resolveFeature(featurePath?: string): Promise<FeatureManifest> {
  if (!featurePath) {
    throw new Error('--feature is required');
  }

  const fullPath = path.isAbsolute(featurePath)
    ? featurePath
    : path.join(featuresDir, featurePath);

  return loadFeatureManifest(fullPath);
}

async function cmdValidate(): Promise<void> {
  const tasks = await loadTaskManifests(tasksDir);
  const features = (await fs.readdir(featuresDir))
    .filter((entry) => entry.endsWith('.json'))
    .sort();
  for (const entry of features) {
    await loadFeatureManifest(path.join(featuresDir, entry));
  }

  console.log(humanJson({
    ok: true,
    taskCount: tasks.length,
    featureCount: features.length
  }));
}

async function cmdCoverage(options: ParsedOptions): Promise<void> {
  const feature = await resolveFeature(options.featurePath);
  const tasks = await selectTasks(options.tasks);
  const coverage = buildCoverageReport(feature, tasks);
  console.log(humanJson(coverage));
}

async function cmdProposeTasks(options: ParsedOptions): Promise<void> {
  const feature = await resolveFeature(options.featurePath);
  const tasks = await selectTasks(options.tasks);
  const coverage = buildCoverageReport(feature, tasks);
  const proposals = proposeTasks(feature, coverage);

  if (options.write) {
    const targetDir = path.join(proposalsDir, feature.featureId);
    await ensureDir(targetDir);
    await writeJsonFile(path.join(targetDir, 'coverage.json'), coverage);
    for (const proposal of proposals) {
      await writeJsonFile(path.join(targetDir, `${String(proposal.id)}.json`), proposal);
    }
  }

  console.log(humanJson({
    coverage,
    proposals
  }));
}

async function cmdRun(options: ParsedOptions): Promise<void> {
  const ref = options.ref ?? 'HEAD';
  const tasks = await selectTasks(options.tasks);
  const executor = new CodexExecutor({
    model: options.model
  });
  const runDirectory = path.join(localRunsDir, runId());
  await ensureDir(runDirectory);
  await writeJsonFile(path.join(runDirectory, 'meta.json'), {
    mode: 'run',
    createdAt: new Date().toISOString(),
    ref,
    taskIds: tasks.map((task) => task.id),
    executor: executor.kind
  });

  const summary = await runTasksForReference(repoRoot, fixturesDir, executor, tasks, runDirectory, {
    repoRoot,
    runsLocalDir: localRunsDir,
    refLabel: 'subject',
    ref,
    sigilBinOverride: options.sigilBin
  });

  await writeJsonFile(path.join(runDirectory, 'run.json'), summary);
  console.log(humanJson({
    runDir: runDirectory,
    summary
  }));
}

async function cmdCompare(options: ParsedOptions): Promise<void> {
  const feature = await resolveFeature(options.featurePath);
  const tasks = await selectTasks(options.tasks);
  const coverage = buildCoverageReport(feature, tasks);
  const runDirectory = path.join(localRunsDir, runId());
  await ensureDir(runDirectory);
  await writeJsonFile(path.join(runDirectory, 'meta.json'), {
    mode: 'compare',
    createdAt: new Date().toISOString(),
    featureId: feature.featureId,
    taskIds: tasks.map((task) => task.id),
    baseRef: options.baseRef ?? 'main',
    candidateRef: options.candidateRef ?? 'HEAD',
    executor: options.executor ?? 'codex'
  });
  await writeJsonFile(path.join(runDirectory, 'coverage.json'), coverage);

  if (!coverage.sufficient) {
    console.log(humanJson({
      runDir: runDirectory,
      coverage,
      status: 'insufficient_coverage'
    }));
    return;
  }

  const executor = new CodexExecutor({
    model: options.model
  });

  const base = await runTasksForReference(repoRoot, fixturesDir, executor, tasks, runDirectory, {
    repoRoot,
    runsLocalDir: localRunsDir,
    refLabel: 'base',
    ref: options.baseRef ?? 'main'
  });

  const candidate = await runTasksForReference(repoRoot, fixturesDir, executor, tasks, runDirectory, {
    repoRoot,
    runsLocalDir: localRunsDir,
    refLabel: 'candidate',
    ref: options.candidateRef ?? 'HEAD'
  });

  const compare = compareRefRuns(feature, coverage, base, candidate);
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
  pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts coverage --feature <file>
  pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts compare --feature <file> [--base main] [--candidate HEAD]
  pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts run --ref <ref>
  pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts publish --run <run-dir> [--label name]
  pnpm exec tsx language/benchmarks/developer-experience/tools/devex-benchmark.ts propose-tasks --feature <file> [--write]`);
}

async function main(): Promise<void> {
  const { command, options } = parseArgs(process.argv.slice(2));

  switch (command) {
    case 'validate':
      await cmdValidate();
      break;
    case 'coverage':
      await cmdCoverage(options);
      break;
    case 'compare':
      await cmdCompare(options);
      break;
    case 'publish':
      await cmdPublish(options);
      break;
    case 'propose-tasks':
      await cmdProposeTasks(options);
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
