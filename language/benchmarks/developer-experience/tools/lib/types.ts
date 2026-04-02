export type BenchmarkCommand = {
  command: string;
  timeoutMs?: number;
};

export type TaskBudgets = {
  maxTurns: number;
  maxWallClockMs: number;
};

export type TaskManifest = {
  id: string;
  title: string;
  goal: string;
  initialPrompt: string;
  fixture: string;
  capabilityTags: string[];
  surfaceTags: string[];
  setupCommands: BenchmarkCommand[];
  oracleCommands: BenchmarkCommand[];
  successCriteria: string[];
  allowedEditPaths: string[];
  forbiddenEditPaths: string[];
  budgets: TaskBudgets;
  rootCauseTags: string[];
};

export type FeatureManifest = {
  featureId: string;
  title: string;
  summary: string;
  primaryCapabilityTags: string[];
  secondaryCapabilityTags: string[];
  expectedSurfaceTags: string[];
  claims: string[];
};

export type CoverageTagReport = {
  tag: string;
  matchedTaskIds: string[];
  requiredCount: number;
  covered: boolean;
};

export type CoverageReport = {
  featureId: string;
  taskIds: string[];
  sufficient: boolean;
  primaryCapabilities: CoverageTagReport[];
  expectedSurfaces: CoverageTagReport[];
  missingPrimaryCapabilities: string[];
  missingExpectedSurfaces: string[];
  summary: string;
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
  events: string[];
  rawStdout: string;
  rawStderr: string;
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

export type TaskRunResult = {
  taskId: string;
  refLabel: string;
  ref: string;
  status: 'passed' | 'failed' | 'error' | 'insufficient_coverage';
  elapsedMs: number;
  oracleResults: ShellCommandResult[];
  setupResults: ShellCommandResult[];
  modifiedPaths: string[];
  patchStats: PatchStats;
  pathPolicy: PathPolicyResult;
  usage: ExecutorUsage | null;
  toolCounts: Record<string, number>;
  finalResponse: AgentFinalResponse | null;
  diagnosisTagsMatched: string[];
  transcriptPath: string;
  diffPath: string;
  workspaceNote?: string;
  errorMessage?: string;
};

export type RefPreparation = {
  refLabel: string;
  requestedRef: string;
  resolvedRef: string;
  worktreePath: string | null;
  sigilBin: string;
};

export type RefRunSummary = {
  refLabel: string;
  requestedRef: string;
  resolvedRef: string;
  taskResults: TaskRunResult[];
  passed: number;
  failed: number;
  errors: number;
  medianElapsedMs: number;
};

export type TaskComparison = {
  taskId: string;
  baseStatus: TaskRunResult['status'];
  candidateStatus: TaskRunResult['status'];
  direction: 'improved' | 'regressed' | 'neutral' | 'mixed';
};

export type CompareSummary = {
  status: 'improved' | 'neutral' | 'regressed' | 'mixed' | 'insufficient_coverage';
  featureId: string;
  taskIds: string[];
  base: RefRunSummary;
  candidate: RefRunSummary;
  taskComparisons: TaskComparison[];
  coverage: CoverageReport;
  generatedAt: string;
};

export type PublishedSummary = {
  runId: string;
  label: string;
  featureId?: string;
  status: CompareSummary['status'] | RefRunSummary['refLabel'];
  generatedAt: string;
  baseRef?: string;
  candidateRef?: string;
  passed?: {
    base: number;
    candidate: number;
  };
};

