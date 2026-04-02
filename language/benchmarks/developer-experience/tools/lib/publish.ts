import { promises as fs } from 'node:fs';
import path from 'node:path';

import { ensureDir, readJsonFile, writeJsonFile, writeTextFile } from './util.js';
import type { CompareSummary, PublishedSummary } from './types.js';

export async function publishCompareRun(resultsDir: string, runDir: string, label: string): Promise<PublishedSummary> {
  const comparePath = path.join(runDir, 'compare.json');
  const compare = await readJsonFile<CompareSummary>(comparePath);
  const totalPossible = compare.base.taskResults.reduce((sum, result) => sum + result.sampleCount, 0);

  await ensureDir(resultsDir);

  const summary: PublishedSummary = {
    runId: path.basename(runDir),
    label,
    status: compare.status,
    generatedAt: compare.generatedAt,
    baseRequestedRef: compare.base.requestedRef,
    baseRef: compare.base.resolvedRef,
    candidateRequestedRef: compare.candidate.requestedRef,
    candidateRef: compare.candidate.resolvedRef,
    rawPassTotals: {
      base: compare.base.rawPassTotal,
      candidate: compare.candidate.rawPassTotal,
      totalPossible
    },
    budgetPassTotals: {
      base: compare.base.budgetPassTotal,
      candidate: compare.candidate.budgetPassTotal,
      totalPossible
    }
  };

  await writeJsonFile(path.join(resultsDir, `${label}.json`), {
    summary,
    compare
  });

  const historyPath = path.join(resultsDir, 'history.jsonl');
  const line = JSON.stringify(summary);
  await fs.appendFile(historyPath, `${line}\n`, 'utf8');

  const latestMarkdown = [
    '# Latest Developer-Experience Benchmark',
    '',
    `- Label: \`${label}\``,
    `- Status: \`${compare.status}\``,
    `- Base raw passes: \`${compare.base.rawPassTotal}/${totalPossible}\``,
    `- Candidate raw passes: \`${compare.candidate.rawPassTotal}/${totalPossible}\``,
    `- Base budget passes: \`${compare.base.budgetPassTotal}/${totalPossible}\``,
    `- Candidate budget passes: \`${compare.candidate.budgetPassTotal}/${totalPossible}\``,
    `- Base ref: \`${compare.base.requestedRef}\` -> \`${compare.base.resolvedRef}\``,
    `- Candidate ref: \`${compare.candidate.requestedRef}\` -> \`${compare.candidate.resolvedRef}\``
  ].join('\n');

  await writeTextFile(path.join(resultsDir, 'LATEST.md'), `${latestMarkdown}\n`);
  return summary;
}
