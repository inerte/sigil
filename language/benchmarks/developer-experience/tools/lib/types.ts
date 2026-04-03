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

export type ExecutorResult = {
  exitCode: number;
  finalResponse: AgentFinalResponse | null;
  usage: ExecutorUsage | null;
  toolCounts: Record<string, number>;
  artifact: ExecutionArtifact;
  errorMessage?: string;
};

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
  diffPath: string;
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

export type TaskComparison = {
  taskId: string;
  baseStatus: TaskRunResult['status'];
  candidateStatus: TaskRunResult['status'];
  direction: 'improved' | 'regressed' | 'neutral';
  decisionBasis: 'budget_margin' | 'neutral';
  budgetPassDelta: number;
  minDecisiveBudgetPassDelta: number;
  baseRawPassCount: number;
  candidateRawPassCount: number;
  baseRawPassRate: number;
  candidateRawPassRate: number;
  baseCommandBudgetPassCount: number;
  candidateCommandBudgetPassCount: number;
  baseCommandBudgetPassRate: number;
  candidateCommandBudgetPassRate: number;
  baseTokenBudgetPassCount: number;
  candidateTokenBudgetPassCount: number;
  baseTokenBudgetPassRate: number;
  candidateTokenBudgetPassRate: number;
  baseBudgetPassCount: number;
  candidateBudgetPassCount: number;
  baseBudgetPassRate: number;
  candidateBudgetPassRate: number;
  baseMedianEffectiveTokens: number | null;
  candidateMedianEffectiveTokens: number | null;
  baseMedianCommandExecutionCount: number | null;
  candidateMedianCommandExecutionCount: number | null;
};

export type CompareSummary = {
  status: 'improved' | 'neutral' | 'regressed' | 'mixed';
  repeats: number;
  minDecisiveBudgetPassDelta: number;
  taskIds: string[];
  base: RefRunSummary;
  candidate: RefRunSummary;
  taskComparisons: TaskComparison[];
  generatedAt: string;
};

export type PublishedSummary = {
  runId: string;
  label: string;
  status: CompareSummary['status'];
  generatedAt: string;
  baseRequestedRef?: string;
  baseRef?: string;
  candidateRequestedRef?: string;
  candidateRef?: string;
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
