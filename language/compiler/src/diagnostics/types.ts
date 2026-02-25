export type SigilPhase =
  | 'cli'
  | 'io'
  | 'surface'
  | 'lexer'
  | 'parser'
  | 'canonical'
  | 'typecheck'
  | 'mutability'
  | 'extern'
  | 'codegen'
  | 'mapgen'
  | 'runtime';

export type SourcePoint = {
  line: number;
  column: number;
  offset?: number;
};

export type SourceSpan = {
  file: string;
  start: SourcePoint;
  end?: SourcePoint;
};

export type Fixit = {
  kind: 'replace' | 'insert' | 'delete';
  range: SourceSpan;
  text?: string;
};

export type Diagnostic = {
  code: string;
  phase: SigilPhase;
  message: string;
  location?: SourceSpan;
  found?: unknown;
  expected?: unknown;
  details?: Record<string, unknown>;
  fixits?: Fixit[];
};

export type CommandEnvelope<TData = unknown> = {
  formatVersion: 1;
  command: string;
  ok: boolean;
  phase?: SigilPhase;
  data?: TData;
  error?: Diagnostic;
};
