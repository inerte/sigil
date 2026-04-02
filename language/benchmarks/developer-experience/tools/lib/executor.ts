import { promises as fs } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';

import { ensureDir, humanJson } from './util.js';
import type { AgentFinalResponse, Executor, ExecutorResult, ExecutorRunContext } from './types.js';

type CodexExecutorOptions = {
  codexBin?: string;
  model?: string;
  sandbox?: 'read-only' | 'workspace-write' | 'danger-full-access';
  extraArgs?: string[];
};

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
        items: { type: 'string' },
        uniqueItems: true
      },
      filesChanged: {
        type: 'array',
        items: { type: 'string' },
        uniqueItems: true
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
    `Capability tags: ${context.task.capabilityTags.join(', ')}`,
    `Surface tags: ${context.task.surfaceTags.join(', ')}`,
    `Root-cause tags to consider: ${context.task.rootCauseTags.join(', ')}`,
    '',
    `Workspace notes:`,
    `- You are working inside a benchmark fixture workspace.`,
    `- The Sigil CLI under test is available at SIGIL_BIN.`,
    `- Keep edits focused on the task and do not touch forbidden paths.`,
    `- Return a final JSON response matching the provided schema.`
  ].join('\n');
}

function countToolEvents(lines: string[]): Record<string, number> {
  const counts: Record<string, number> = {};

  for (const line of lines) {
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

  return counts;
}

export class CodexExecutor implements Executor {
  readonly kind = 'codex';
  private readonly options: CodexExecutorOptions;

  constructor(options: CodexExecutorOptions = {}) {
    this.options = options;
  }

  async run(context: ExecutorRunContext): Promise<ExecutorResult> {
    const schemaDir = await fs.mkdtemp(path.join(os.tmpdir(), 'sigil-devex-codex-'));
    const schemaPath = path.join(schemaDir, 'final-response.schema.json');
    const outputPath = path.join(schemaDir, 'final-response.json');
    await ensureDir(schemaDir);
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

      let stdout = '';
      let stderr = '';
      let settled = false;

      const timer = setTimeout(() => {
        if (!settled) {
          child.kill('SIGKILL');
        }
      }, context.timeoutMs);

      child.stdout.on('data', (chunk) => {
        stdout += chunk.toString();
      });

      child.stderr.on('data', (chunk) => {
        stderr += chunk.toString();
      });

      child.stdin.write(prompt);
      child.stdin.end();

      child.on('close', async (code) => {
        settled = true;
        clearTimeout(timer);

        let finalResponse: AgentFinalResponse | null = null;
        try {
          finalResponse = JSON.parse(await fs.readFile(outputPath, 'utf8')) as AgentFinalResponse;
        } catch {
          finalResponse = null;
        }

        const events = stdout
          .split('\n')
          .map((line) => line.trim())
          .filter(Boolean);

        let usage = null;
        for (const line of events) {
          try {
            const parsed = JSON.parse(line) as Record<string, unknown>;
            if (parsed.type === 'turn.completed' && parsed.usage && typeof parsed.usage === 'object') {
              const rawUsage = parsed.usage as Record<string, unknown>;
              usage = {
                inputTokens: Number(rawUsage.input_tokens) || undefined,
                cachedInputTokens: Number(rawUsage.cached_input_tokens) || undefined,
                outputTokens: Number(rawUsage.output_tokens) || undefined
              };
            }
          } catch {
            // Ignore malformed event lines and leave them in the transcript.
          }
        }

        resolve({
          exitCode: code ?? 1,
          finalResponse,
          usage,
          toolCounts: countToolEvents(events),
          artifact: {
            events,
            rawStdout: stdout,
            rawStderr: stderr
          },
          errorMessage: code === 0 ? undefined : `Codex exited with code ${code ?? 1}`
        });

        await fs.rm(schemaDir, { recursive: true, force: true });
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
