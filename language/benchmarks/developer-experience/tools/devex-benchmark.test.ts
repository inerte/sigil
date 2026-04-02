import test from 'node:test';
import assert from 'node:assert/strict';
import os from 'node:os';
import path from 'node:path';
import { promises as fs } from 'node:fs';

import { buildCoverageReport, proposeTasks } from './lib/coverage.js';
import { MockExecutor } from './lib/executor.js';
import { loadFeatureManifest, loadTaskManifests } from './lib/manifests.js';
import { publishCompareRun } from './lib/publish.js';
import { compareRefRuns, runTasksForReference } from './lib/runner.js';
import { ensureDir, writeJsonFile } from './lib/util.js';
import type { ExecutorResult, FeatureManifest, TaskManifest } from './lib/types.js';

const benchmarkRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), '..');
const tasksDir = path.join(benchmarkRoot, 'tasks');
const featuresDir = path.join(benchmarkRoot, 'features');

test('current task and feature manifests validate', async () => {
  const tasks = await loadTaskManifests(tasksDir);
  const feature = await loadFeatureManifest(path.join(featuresDir, 'agent-edit-loop-smoke.json'));

  assert.equal(tasks.length, 4);
  assert.equal(feature.featureId, 'agent-edit-loop-smoke');
});

test('inspect-types feature reports insufficient coverage and proposes tasks', async () => {
  const tasks = await loadTaskManifests(tasksDir);
  const feature = await loadFeatureManifest(path.join(featuresDir, 'inspect-types-typeids.json'));
  const coverage = buildCoverageReport(feature, tasks);
  const proposals = proposeTasks(feature, coverage);

  assert.equal(coverage.sufficient, false);
  assert.ok(coverage.missingPrimaryCapabilities.includes('type_semantics'));
  assert.ok(proposals.length >= 2);
});

test('runner records a passing task result with a mock executor', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'sigil-devex-runner-'));
  const fixturesDir = path.join(root, 'fixtures');
  const fixtureDir = path.join(fixturesDir, 'simple-pass');
  const runDir = path.join(root, '.local', 'runs', 'sample-run');
  await ensureDir(path.join(fixtureDir));
  await fs.writeFile(path.join(fixtureDir, 'note.txt'), 'broken\n', 'utf8');

  const task: TaskManifest = {
    id: 'simple-pass',
    title: 'Simple pass task',
    goal: 'Write a file that satisfies the oracle.',
    initialPrompt: 'Create fixed.txt in the workspace.',
    fixture: 'simple-pass',
    capabilityTags: ['basic_repair'],
    surfaceTags: ['compile'],
    setupCommands: [],
    oracleCommands: [
      {
        command: 'test -f fixed.txt'
      }
    ],
    successCriteria: ['fixed.txt exists'],
    allowedEditPaths: ['fixed.txt'],
    forbiddenEditPaths: ['.local'],
    budgets: {
      maxTurns: 5,
      maxWallClockMs: 60_000
    },
    rootCauseTags: ['missing_output']
  };

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
        'event:item.completed': 1
      },
      artifact: {
        events: ['{"type":"item.completed","item":{"type":"agent_message"}}'],
        rawStdout: '',
        rawStderr: ''
      }
    };
  });

  const summary = await runTasksForReference(root, fixturesDir, executor, [task], runDir, {
    repoRoot: root,
    runsLocalDir: path.join(root, '.local', 'runs'),
    refLabel: 'candidate',
    ref: 'HEAD',
    sigilBinOverride: '/usr/bin/true'
  });

  assert.equal(summary.passed, 1);
  assert.equal(summary.failed, 0);
  assert.equal(summary.taskResults[0].status, 'passed');
  assert.deepEqual(summary.taskResults[0].diagnosisTagsMatched, ['missing_output']);
});

test('publish writes history and latest summary files', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'sigil-devex-publish-'));
  const resultsDir = path.join(root, 'results');
  const runDir = path.join(root, '.local', 'runs', 'publish-sample');
  await ensureDir(runDir);

  const feature = await loadFeatureManifest(path.join(featuresDir, 'agent-edit-loop-smoke.json'));
  const base = {
    refLabel: 'base',
    requestedRef: 'main',
    resolvedRef: 'aaa111',
    taskResults: [],
    passed: 1,
    failed: 0,
    errors: 0,
    medianElapsedMs: 100
  };
  const candidate = {
    refLabel: 'candidate',
    requestedRef: 'HEAD',
    resolvedRef: 'bbb222',
    taskResults: [],
    passed: 2,
    failed: 0,
    errors: 0,
    medianElapsedMs: 80
  };
  const compare = compareRefRuns(feature, buildCoverageReport(feature, await loadTaskManifests(tasksDir)), base, candidate);
  await writeJsonFile(path.join(runDir, 'compare.json'), compare);

  const published = await publishCompareRun(resultsDir, runDir, 'smoke-sample');

  assert.equal(published.label, 'smoke-sample');
  assert.match(await fs.readFile(path.join(resultsDir, 'history.jsonl'), 'utf8'), /smoke-sample/);
  assert.match(await fs.readFile(path.join(resultsDir, 'LATEST.md'), 'utf8'), /Latest Developer-Experience Benchmark/);
});
