import { randomInt } from 'node:crypto';

import type {
  CompareSide,
  JudgeArtifactPaths,
  JudgeSide,
  JudgeWinner,
  TaskJudgmentSummary,
  TaskManifest,
  TaskRepeatJudgeInput,
  TaskRepeatJudgeResult,
  TaskRepeatJudgmentSummary,
  TaskRunResult,
  TaskSampleResult
} from './types.js';

function artifactPathsFromSample(sample: TaskSampleResult): JudgeArtifactPaths {
  return {
    resultPath: sample.resultPath,
    transcriptPath: sample.transcriptPath,
    stdoutPath: sample.executorStdoutPath,
    stderrPath: sample.executorStderrPath,
    finalResponsePath: sample.finalResponsePath,
    diffPath: sample.diffPath,
    oracleResultsPath: sample.oracleResultsPath,
    setupResultsPath: sample.setupResultsPath
  };
}

export function buildJudgePrompt(judgeInputPath: string): string {
  return [
    'You are an impartial judge comparing two Codex runs on the same software engineering task repeat.',
    '',
    'First read the JSON file at this path:',
    judgeInputPath,
    '',
    'That file lists the full artifact bundle for blinded Run A and blinded Run B.',
    'You must read every file referenced under runs.A and runs.B before deciding.',
    'Only inspect the files explicitly listed in judge-input.json.',
    'Do not search the repository, temp directories, or the broader filesystem for extra evidence.',
    'If a listed file is missing or unreadable, note that and continue with the remaining listed artifacts.',
    '',
    'Judging rules:',
    '- The runs are blinded as Run A and Run B. Do not assume either is baseline or candidate.',
    '- Use the oracle and final observed outcome as the strongest evidence.',
    '- Prefer concrete evidence from artifacts: command outputs, test results, compile results, tool calls, diffs, code edits, final responses, and final state.',
    '- Ignore elapsed time, token counts, command counts, budget flags, and benchmark score fields, even if they appear in result.json.',
    '- If one run clearly completed the task correctly and the other did not, that run should win.',
    '- If both completed the task, prefer the run with better diagnosis, more focused edits, less collateral damage, better evidence use, and more robust handling of ambiguity.',
    '- If neither completed the task, prefer the run that made more meaningful progress toward the correct solution.',
    '- If the evidence is too close or ambiguous, return TIE.',
    '- Do not reward verbosity or more activity by itself.',
    '- Do not invent evidence that is not present in the artifacts.',
    '',
    'Return only valid JSON matching the provided schema.'
  ].join('\n');
}

export function buildJudgeInput(
  task: TaskManifest,
  baseSample: TaskSampleResult,
  candidateSample: TaskSampleResult
): { input: TaskRepeatJudgeInput; aRealSide: CompareSide; bRealSide: CompareSide } {
  const flip = randomInt(2) === 1;
  const aRealSide: CompareSide = flip ? 'candidate' : 'base';
  const bRealSide: CompareSide = aRealSide === 'base' ? 'candidate' : 'base';

  const sideMap: Record<CompareSide, TaskSampleResult> = {
    base: baseSample,
    candidate: candidateSample
  };

  const input: TaskRepeatJudgeInput = {
    taskId: task.id,
    title: task.title,
    goal: task.goal,
    successCriteria: task.successCriteria,
    allowedEditPaths: task.allowedEditPaths,
    forbiddenEditPaths: task.forbiddenEditPaths,
    repeatIndex: baseSample.sampleIndex,
    runs: {
      A: artifactPathsFromSample(sideMap[aRealSide]),
      B: artifactPathsFromSample(sideMap[bRealSide])
    }
  };

  return {
    input,
    aRealSide,
    bRealSide
  };
}

function resolveWinner(
  winner: JudgeWinner,
  aRealSide: CompareSide,
  bRealSide: CompareSide
): CompareSide | 'TIE' {
  if (winner === 'A') {
    return aRealSide;
  }
  if (winner === 'B') {
    return bRealSide;
  }
  return 'TIE';
}

export function summarizeRepeatJudgment(
  judgment: TaskRepeatJudgeResult
): TaskRepeatJudgmentSummary {
  return {
    repeatIndex: judgment.repeatIndex,
    resolvedWinner: judgment.resolvedWinner,
    judgeStatus: judgment.judgeStatus,
    confidence: judgment.judgeResponse.confidence,
    summary: judgment.judgeResponse.summary,
    resultPath: judgment.resultPath,
    errorMessage: judgment.errorMessage
  };
}

export function summarizeTaskJudgment(
  baseResult: TaskRunResult,
  candidateResult: TaskRunResult,
  repeatJudgments: TaskRepeatJudgeResult[]
): TaskJudgmentSummary {
  const baselineRepeatWins = repeatJudgments.filter((judgment) => judgment.resolvedWinner === 'base').length;
  const compareRepeatWins = repeatJudgments.filter((judgment) => judgment.resolvedWinner === 'candidate').length;
  const repeatTies = repeatJudgments.filter((judgment) => judgment.resolvedWinner === 'TIE').length;

  let taskLean: CompareSide | 'tie' = 'tie';
  if (baselineRepeatWins > compareRepeatWins) {
    taskLean = 'base';
  } else if (compareRepeatWins > baselineRepeatWins) {
    taskLean = 'candidate';
  }

  return {
    taskId: baseResult.taskId,
    baseStatus: baseResult.status,
    candidateStatus: candidateResult.status,
    baseRawPassCount: baseResult.rawPassCount,
    candidateRawPassCount: candidateResult.rawPassCount,
    baseBudgetPassCount: baseResult.budgetPassCount,
    candidateBudgetPassCount: candidateResult.budgetPassCount,
    baselineRepeatWins,
    compareRepeatWins,
    repeatTies,
    taskLean,
    repeatJudgments: repeatJudgments
      .slice()
      .sort((left, right) => left.repeatIndex - right.repeatIndex)
      .map(summarizeRepeatJudgment)
  };
}

export function summarizeSuiteJudgment(taskJudgments: TaskJudgmentSummary[]) {
  return {
    baselineTaskLeans: taskJudgments.filter((judgment) => judgment.taskLean === 'base').length,
    compareTaskLeans: taskJudgments.filter((judgment) => judgment.taskLean === 'candidate').length,
    taskTies: taskJudgments.filter((judgment) => judgment.taskLean === 'tie').length,
    totalTasks: taskJudgments.length
  };
}
