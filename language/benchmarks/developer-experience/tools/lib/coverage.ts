import type { CoverageReport, CoverageTagReport, FeatureManifest, TaskManifest } from './types.js';

function coverageRows(items: TaskManifest[], tagSelector: (task: TaskManifest) => string[], targets: string[], requiredCount: number): CoverageTagReport[] {
  return targets.map((target) => {
    const matchedTaskIds = items
      .filter((task) => tagSelector(task).includes(target))
      .map((task) => task.id);

    return {
      tag: target,
      matchedTaskIds,
      requiredCount,
      covered: matchedTaskIds.length >= requiredCount
    };
  });
}

export function buildCoverageReport(feature: FeatureManifest, tasks: TaskManifest[]): CoverageReport {
  const primaryCapabilities = coverageRows(tasks, (task) => task.capabilityTags, feature.primaryCapabilityTags, 2);
  const expectedSurfaces = coverageRows(tasks, (task) => task.surfaceTags, feature.expectedSurfaceTags, 1);

  const missingPrimaryCapabilities = primaryCapabilities.filter((row) => !row.covered).map((row) => row.tag);
  const missingExpectedSurfaces = expectedSurfaces.filter((row) => !row.covered).map((row) => row.tag);
  const sufficient = missingPrimaryCapabilities.length === 0 && missingExpectedSurfaces.length === 0;

  const summary = sufficient
    ? `Coverage is sufficient for feature '${feature.featureId}'.`
    : `Coverage is insufficient for feature '${feature.featureId}': missing primary capability coverage for [${missingPrimaryCapabilities.join(', ')}] and missing expected surface coverage for [${missingExpectedSurfaces.join(', ')}].`;

  return {
    featureId: feature.featureId,
    taskIds: tasks.map((task) => task.id),
    sufficient,
    primaryCapabilities,
    expectedSurfaces,
    missingPrimaryCapabilities,
    missingExpectedSurfaces,
    summary
  };
}

export function proposeTasks(feature: FeatureManifest, coverage: CoverageReport): Array<Record<string, unknown>> {
  const proposals: Array<Record<string, unknown>> = [];

  for (const row of coverage.primaryCapabilities) {
    const missing = Math.max(0, row.requiredCount - row.matchedTaskIds.length);
    for (let index = 0; index < missing; index += 1) {
      proposals.push({
        id: `${feature.featureId}-${row.tag}-proposal-${index + 1}`,
        title: `Add ${row.tag} coverage for ${feature.title}`,
        goal: `Create a deterministic Sigil maintenance task that exercises capability tag '${row.tag}'.`,
        initialPrompt: `Design a task that forces the agent to rely on '${row.tag}' while keeping the oracle deterministic.`,
        fixture: 'TODO',
        capabilityTags: [row.tag],
        surfaceTags: feature.expectedSurfaceTags.slice(0, 1),
        setupCommands: [],
        oracleCommands: [],
        successCriteria: ['TODO'],
        allowedEditPaths: ['TODO'],
        forbiddenEditPaths: [],
        budgets: {
          maxTurns: 30,
          maxWallClockMs: 600000
        },
        rootCauseTags: [row.tag]
      });
    }
  }

  for (const row of coverage.expectedSurfaces) {
    if (row.covered) {
      continue;
    }

    proposals.push({
      id: `${feature.featureId}-${row.tag}-surface-proposal`,
      title: `Add ${row.tag} surface coverage for ${feature.title}`,
      goal: `Create a deterministic task that directly exercises the '${row.tag}' surface.`,
      initialPrompt: `Design a task whose fastest reliable solution uses the '${row.tag}' surface.`,
      fixture: 'TODO',
      capabilityTags: feature.primaryCapabilityTags.slice(0, 1),
      surfaceTags: [row.tag],
      setupCommands: [],
      oracleCommands: [],
      successCriteria: ['TODO'],
      allowedEditPaths: ['TODO'],
      forbiddenEditPaths: [],
      budgets: {
        maxTurns: 30,
        maxWallClockMs: 600000
      },
      rootCauseTags: [row.tag]
    });
  }

  return proposals;
}

