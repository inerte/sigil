import { promises as fs, createWriteStream } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';

import { ensureDir, humanJson } from './util.js';
import type { AgentFinalResponse, Executor, ExecutorResult, ExecutorRunContext, ExecutorUsage } from './types.js';

type CodexExecutorOptions = {
  codexBin?: string;
  model?: string;
  sandbox?: 'read-only' | 'workspace-write' | 'danger-full-access';
  extraArgs?: string[];
};

const LOG_TAIL_LIMIT = 16_384;

function finalResponseSchema(): Record<string, unknown> {
  return {
    type: 'object',
    additionalProperties: false,
    required: ['summary', 'diagnosis', 'diagnosisTags', 'filesChanged'],
    properties: {
      summary: { type: 'string' },
      diagnosis: { type: 'string' },
      diagnosisTags: {
        type: 'array',
        items: { type: 'string' }
      },
      filesChanged: {
        type: 'array',
        items: { type: 'string' }
      }
    }
  };
}

function buildPrompt(context: ExecutorRunContext): string {
  return [
    `Task: ${context.task.title}`,
    '',
    `Goal: ${context.task.goal}`,
    '',
    context.task.initialPrompt,
    '',
    `Success criteria:`,
    ...context.task.successCriteria.map((criterion) => `- ${criterion}`),
    '',
    `Allowed edit paths: ${context.task.allowedEditPaths.join(', ') || '(none)'}`,
    `Forbidden edit paths: ${context.task.forbiddenEditPaths.join(', ') || '(none)'}`,
    `Root-cause tags to consider: ${context.task.rootCauseTags.join(', ')}`,
    '',
    `Workspace notes:`,
    `- You are working inside a benchmark fixture workspace.`,
    `- The Sigil CLI under test is available at SIGIL_BIN.`,
    `- Keep edits focused on the task and do not touch forbidden paths.`,
    `- Return a final JSON response matching the provided schema.`
  ].join('\n');
}

function appendTail(current: string, chunk: string, limit = LOG_TAIL_LIMIT): string {
  const combined = `${current}${chunk}`;
  return combined.length <= limit ? combined : combined.slice(-limit);
}

function countToolEventLine(line: string, counts: Record<string, number>): void {
  try {
    const parsed = JSON.parse(line) as Record<string, unknown>;
    const eventType = typeof parsed.type === 'string' ? parsed.type : 'unknown';
    counts[`event:${eventType}`] = (counts[`event:${eventType}`] ?? 0) + 1;

    const item = parsed.item;
    if (item && typeof item === 'object' && typeof (item as Record<string, unknown>).type === 'string') {
      const itemType = String((item as Record<string, unknown>).type);
      counts[`item:${itemType}`] = (counts[`item:${itemType}`] ?? 0) + 1;
    }
  } catch {
    counts['event:unparsed'] = (counts['event:unparsed'] ?? 0) + 1;
  }
}

function maybeExtractUsage(line: string, current: ExecutorUsage | null): ExecutorUsage | null {
  try {
    const parsed = JSON.parse(line) as Record<string, unknown>;
    if (parsed.type !== 'turn.completed' || !parsed.usage || typeof parsed.usage !== 'object') {
      return current;
    }

    const rawUsage = parsed.usage as Record<string, unknown>;
    return {
      inputTokens: Number(rawUsage.input_tokens) || undefined,
      cachedInputTokens: Number(rawUsage.cached_input_tokens) || undefined,
      outputTokens: Number(rawUsage.output_tokens) || undefined
    };
  } catch {
    return current;
  }
}

function finishStream(stream: ReturnType<typeof createWriteStream>): Promise<void> {
  return new Promise((resolve, reject) => {
    stream.on('finish', resolve);
    stream.on('error', reject);
    stream.end();
  });
}

function buildExitErrorMessage(exitCode: number, stdoutTail: string, stderrTail: string): string {
  const tail = stderrTail.trim() || stdoutTail.trim();
  return tail.length > 0
    ? `Codex exited with code ${exitCode}: ${tail}`
    : `Codex exited with code ${exitCode}`;
}

export class CodexExecutor implements Executor {
  readonly kind = 'codex';
  private readonly options: CodexExecutorOptions;

  constructor(options: CodexExecutorOptions = {}) {
    this.options = options;
  }

  async run(context: ExecutorRunContext): Promise<ExecutorResult> {
    const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'sigil-devex-codex-'));
    const schemaPath = path.join(tempDir, 'final-response.schema.json');
    const outputPath = path.join(tempDir, 'final-response.json');
    const stdoutPath = path.join(tempDir, 'executor.stdout.log');
    const stderrPath = path.join(tempDir, 'executor.stderr.log');
    await ensureDir(tempDir);
    await fs.writeFile(schemaPath, humanJson(finalResponseSchema()), 'utf8');

    const prompt = buildPrompt(context);
    const codexBin = this.options.codexBin ?? 'codex';
    const args = [
      'exec',
      '--json',
      '--color',
      'never',
      '--ephemeral',
      '-C',
      context.workspacePath,
      '-s',
      this.options.sandbox ?? 'workspace-write',
      '-c',
      'approval_policy="never"',
      '--output-schema',
      schemaPath,
      '-o',
      outputPath
    ];

    if (this.options.model) {
      args.push('-m', this.options.model);
    }

    if (this.options.extraArgs) {
      args.push(...this.options.extraArgs);
    }

    args.push('-');

    return new Promise((resolve) => {
      const child = spawn(codexBin, args, {
        cwd: context.workspacePath,
        env: {
          ...process.env,
          ...context.env
        }
      });

      const stdoutStream = createWriteStream(stdoutPath, { encoding: 'utf8' });
      const stderrStream = createWriteStream(stderrPath, { encoding: 'utf8' });
      const toolCounts: Record<string, number> = {};
      let usage: ExecutorUsage | null = null;
      let stdoutTail = '';
      let stderrTail = '';
      let stdoutBuffer = '';
      let settled = false;

      const timer = setTimeout(() => {
        if (!settled) {
          child.kill('SIGKILL');
        }
      }, context.timeoutMs);

      const handleStdoutChunk = (chunk: string): void => {
        stdoutStream.write(chunk);
        stdoutTail = appendTail(stdoutTail, chunk);
        stdoutBuffer += chunk;

        while (true) {
          const newlineIndex = stdoutBuffer.indexOf('\n');
          if (newlineIndex === -1) {
            break;
          }

          const line = stdoutBuffer.slice(0, newlineIndex).trim();
          stdoutBuffer = stdoutBuffer.slice(newlineIndex + 1);

          if (line.length === 0) {
            continue;
          }

          countToolEventLine(line, toolCounts);
          usage = maybeExtractUsage(line, usage);
        }
      };

      child.stdout.on('data', (chunk) => {
        handleStdoutChunk(chunk.toString());
      });

      child.stderr.on('data', (chunk) => {
        const text = chunk.toString();
        stderrStream.write(text);
        stderrTail = appendTail(stderrTail, text);
      });

      child.on('error', (error) => {
        const rendered = `${String(error.message)}\n`;
        stderrStream.write(rendered);
        stderrTail = appendTail(stderrTail, rendered);
      });

      child.stdin.write(prompt);
      child.stdin.end();

      child.on('close', async (code) => {
        settled = true;
        clearTimeout(timer);

        if (stdoutBuffer.trim().length > 0) {
          const trailingLine = stdoutBuffer.trim();
          countToolEventLine(trailingLine, toolCounts);
          usage = maybeExtractUsage(trailingLine, usage);
        }

        await Promise.allSettled([
          finishStream(stdoutStream),
          finishStream(stderrStream)
        ]);

        let finalResponse: AgentFinalResponse | null = null;
        try {
          finalResponse = JSON.parse(await fs.readFile(outputPath, 'utf8')) as AgentFinalResponse;
        } catch {
          finalResponse = null;
        }

        const exitCode = code ?? 1;
        resolve({
          exitCode,
          finalResponse,
          usage,
          toolCounts,
          artifact: {
            tempDir,
            stdoutPath,
            stderrPath,
            stdoutTail,
            stderrTail
          },
          errorMessage: exitCode === 0 ? undefined : buildExitErrorMessage(exitCode, stdoutTail, stderrTail)
        });
      });
    });
  }
}

export class MockExecutor implements Executor {
  readonly kind = 'mock';
  private readonly handler: (context: ExecutorRunContext) => Promise<ExecutorResult>;

  constructor(handler: (context: ExecutorRunContext) => Promise<ExecutorResult>) {
    this.handler = handler;
  }

  async run(context: ExecutorRunContext): Promise<ExecutorResult> {
    return this.handler(context);
  }
}
