export type BenchmarkCommand = {
  command: string;
  timeoutMs?: number;
};

export type TaskBudgets = {
  maxCommandExecutions: number;
  maxEffectiveTokens: number;
  maxWallClockMs: number;
};

export type TaskManifest = {
  id: string;
  title: string;
  goal: string;
  initialPrompt: string;
  fixture: string;
  setupCommands: BenchmarkCommand[];
  oracleCommands: BenchmarkCommand[];
  successCriteria: string[];
  allowedEditPaths: string[];
  forbiddenEditPaths: string[];
  budgets: TaskBudgets;
  rootCauseTags: string[];
};

export type AgentFinalResponse = {
  summary: string;
  diagnosis: string;
  diagnosisTags: string[];
  filesChanged: string[];
};

export type JudgeSide = 'A' | 'B';
export type JudgeWinner = JudgeSide | 'TIE';
export type JudgeConfidence = 'low' | 'medium' | 'high';
export type JudgeTaskCompletion = 'completed' | 'partial' | 'failed';
export type CompareSide = 'base' | 'candidate';

export type JudgeScoreCard = {
  A: number;
  B: number;
};

export type JudgeTaskCompletionCard = {
  A: JudgeTaskCompletion;
  B: JudgeTaskCompletion;
};

export type JudgeEvidenceCitation = {
  run: JudgeSide;
  artifact: string;
  fact: string;
};

export type JudgeFinalResponse = {
  winner: JudgeWinner;
  confidence: JudgeConfidence;
  summary: string;
  task_completion: JudgeTaskCompletionCard;
  diagnosis_quality: JudgeScoreCard;
  edit_quality: JudgeScoreCard;
  evidence_use: JudgeScoreCard;
  key_reasons: string[];
  evidence_citations: JudgeEvidenceCitation[];
};

export type ExecutorUsage = {
  inputTokens?: number;
  cachedInputTokens?: number;
  outputTokens?: number;
};

export type ExecutionArtifact = {
  tempDir: string;
  stdoutPath: string;
  stderrPath: string;
  stdoutTail: string;
  stderrTail: string;
};

export type JsonExecutorResult<TFinalResponse> = {
  exitCode: number;
  finalResponse: TFinalResponse | null;
  usage: ExecutorUsage | null;
  toolCounts: Record<string, number>;
  artifact: ExecutionArtifact;
  errorMessage?: string;
};

export type ExecutorResult = JsonExecutorResult<AgentFinalResponse>;
export type JudgeExecutorResult = JsonExecutorResult<JudgeFinalResponse>;

export type ExecutorRunContext = {
  task: TaskManifest;
  workspacePath: string;
  runLabel: string;
  prompt: string;
  env: Record<string, string>;
  timeoutMs: number;
};

export interface Executor {
  readonly kind: string;
  run(context: ExecutorRunContext): Promise<ExecutorResult>;
}

export type JudgeRunContext = {
  cwd: string;
  runLabel: string;
  prompt: string;
  env: Record<string, string>;
  timeoutMs: number;
};

export interface JudgeExecutor {
  readonly kind: string;
  run(context: JudgeRunContext): Promise<JudgeExecutorResult>;
}

export type ShellCommandResult = {
  command: string;
  cwd: string;
  stdout: string;
  stderr: string;
  exitCode: number;
  durationMs: number;
};

export type PathPolicyResult = {
  allowed: boolean;
  forbiddenMatches: string[];
  outOfBoundsMatches: string[];
};

export type PatchStats = {
  additions: number;
  deletions: number;
  filesChanged: number;
};

export type PhaseTimings = {
  workspacePrepMs: number;
  setupMs: number;
  executorMs: number;
  stateCollectionMs: number;
  oracleMs: number;
  artifactWriteMs: number;
  overheadMs: number;
};

export type TaskSampleResult = {
  taskId: string;
  refLabel: string;
  ref: string;
  sampleIndex: number;
  status: 'passed' | 'failed' | 'error';
  elapsedMs: number;
  phaseTimings: PhaseTimings;
  oracleResults: ShellCommandResult[];
  setupResults: ShellCommandResult[];
  modifiedPaths: string[];
  patchStats: PatchStats;
  pathPolicy: PathPolicyResult;
  usage: ExecutorUsage | null;
  toolCounts: Record<string, number>;
  commandExecutionCount: number | null;
  effectiveTokens: number | null;
  withinCommandBudget: boolean | null;
  withinTokenBudget: boolean | null;
  withinAllBudgets: boolean;
  finalResponse: AgentFinalResponse | null;
  diagnosisTagsMatched: string[];
  transcriptPath: string;
  executorStdoutPath: string;
  executorStderrPath: string;
  diffPath: string;
  finalResponsePath: string;
  setupResultsPath: string;
  oracleResultsPath: string;
  resultPath: string;
  workspaceNote?: string;
  errorMessage?: string;
};

export type TaskRunResult = {
  taskId: string;
  refLabel: string;
  ref: string;
  status: 'passed' | 'failed' | 'error';
  sampleCount: number;
  statusCounts: Record<TaskSampleResult['status'], number>;
  rawPassCount: number;
  rawPassRate: number;
  commandBudgetPassCount: number;
  commandBudgetPassRate: number;
  tokenBudgetPassCount: number;
  tokenBudgetPassRate: number;
  budgetPassCount: number;
  budgetPassRate: number;
  medianElapsedMs: number;
  medianEffectiveTokens: number | null;
  medianCommandExecutionCount: number | null;
  medianPhaseTimings: PhaseTimings;
  sampleResultPaths: string[];
};

export type ReferenceSourceKind = 'ref' | 'worktree' | 'binary';

export type RefPreparation = {
  refLabel: string;
  sourceKind: ReferenceSourceKind;
  requestedRef: string;
  resolvedRef: string;
  preparationPath: string | null;
  sigilBin: string;
};

export type RefRunSummary = {
  refLabel: string;
  sourceKind: ReferenceSourceKind;
  requestedRef: string;
  resolvedRef: string;
  taskResults: TaskRunResult[];
  passed: number;
  failed: number;
  errors: number;
  rawPassTotal: number;
  budgetPassTotal: number;
  medianElapsedMs: number;
  medianEffectiveTokens: number | null;
  medianCommandExecutionCount: number | null;
};

export type JudgeArtifactPaths = {
  resultPath: string;
  transcriptPath: string;
  stdoutPath: string;
  stderrPath: string;
  finalResponsePath: string;
  diffPath: string;
  oracleResultsPath: string;
  setupResultsPath: string;
};

export type TaskRepeatJudgeInput = {
  taskId: string;
  title: string;
  goal: string;
  successCriteria: string[];
  allowedEditPaths: string[];
  forbiddenEditPaths: string[];
  repeatIndex: number;
  runs: Record<JudgeSide, JudgeArtifactPaths>;
};

export type TaskRepeatJudgeResult = {
  taskId: string;
  repeatIndex: number;
  aRealSide: CompareSide;
  bRealSide: CompareSide;
  resolvedWinner: CompareSide | 'TIE';
  judgeStatus: 'completed' | 'error';
  judgeResponse: JudgeFinalResponse;
  judgeInputPath: string;
  judgePromptPath: string;
  judgeStdoutPath: string;
  judgeStderrPath: string;
  resultPath: string;
  errorMessage?: string;
};

export type TaskRepeatJudgmentSummary = {
  repeatIndex: number;
  resolvedWinner: CompareSide | 'TIE';
  judgeStatus: 'completed' | 'error';
  confidence: JudgeConfidence;
  summary: string;
  resultPath: string;
  errorMessage?: string;
};

export type TaskJudgmentSummary = {
  taskId: string;
  baseStatus: TaskRunResult['status'];
  candidateStatus: TaskRunResult['status'];
  baseRawPassCount: number;
  candidateRawPassCount: number;
  baseBudgetPassCount: number;
  candidateBudgetPassCount: number;
  baselineRepeatWins: number;
  compareRepeatWins: number;
  repeatTies: number;
  taskLean: CompareSide | 'tie';
  repeatJudgments: TaskRepeatJudgmentSummary[];
};

export type SuiteJudgmentSummary = {
  baselineTaskLeans: number;
  compareTaskLeans: number;
  taskTies: number;
  totalTasks: number;
};

export type CompareSummary = {
  repeats: number;
  taskIds: string[];
  base: RefRunSummary;
  candidate: RefRunSummary;
  taskJudgments: TaskJudgmentSummary[];
  suiteJudgment: SuiteJudgmentSummary;
  generatedAt: string;
};

export type PublishedSummary = {
  runId: string;
  label: string;
  generatedAt: string;
  baseRequestedRef?: string;
  baseRef?: string;
  candidateRequestedRef?: string;
  candidateRef?: string;
  taskLeanTotals?: {
    baseline: number;
    compare: number;
    ties: number;
    totalTasks: number;
  };
  rawPassTotals?: {
    base: number;
    candidate: number;
    totalPossible: number;
  };
  budgetPassTotals?: {
    base: number;
    candidate: number;
    totalPossible: number;
  };
};
