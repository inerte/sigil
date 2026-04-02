import { promises as fs } from 'node:fs';
import path from 'node:path';

import { readJsonFile } from './util.js';
import type { TaskManifest } from './types.js';

type ValidationIssue = {
  message: string;
  path: string;
};

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((entry) => typeof entry === 'string' && entry.length > 0);
}

function validateCommandArray(value: unknown, key: string, issues: ValidationIssue[]): void {
  if (!Array.isArray(value)) {
    issues.push({ path: key, message: 'must be an array of command objects' });
    return;
  }

  for (const [index, entry] of value.entries()) {
    if (typeof entry !== 'object' || entry === null) {
      issues.push({ path: `${key}[${index}]`, message: 'must be an object' });
      continue;
    }

    const record = entry as Record<string, unknown>;
    if (typeof record.command !== 'string' || record.command.trim().length === 0) {
      issues.push({ path: `${key}[${index}].command`, message: 'must be a non-empty string' });
    }

    if (record.timeoutMs !== undefined && (!Number.isInteger(record.timeoutMs) || Number(record.timeoutMs) <= 0)) {
      issues.push({ path: `${key}[${index}].timeoutMs`, message: 'must be a positive integer when provided' });
    }
  }
}

function validateTaskManifestShape(value: unknown): ValidationIssue[] {
  const issues: ValidationIssue[] = [];

  if (typeof value !== 'object' || value === null) {
    return [{ path: '', message: 'task manifest must be an object' }];
  }

  const record = value as Record<string, unknown>;
  const stringKeys = ['id', 'title', 'goal', 'initialPrompt', 'fixture'];

  for (const key of stringKeys) {
    if (typeof record[key] !== 'string' || String(record[key]).trim().length === 0) {
      issues.push({ path: key, message: 'must be a non-empty string' });
    }
  }

  for (const key of ['successCriteria', 'allowedEditPaths', 'forbiddenEditPaths', 'rootCauseTags']) {
    if (!isStringArray(record[key])) {
      issues.push({ path: key, message: 'must be a non-empty string array' });
    }
  }

  validateCommandArray(record.setupCommands, 'setupCommands', issues);
  validateCommandArray(record.oracleCommands, 'oracleCommands', issues);

  if (typeof record.budgets !== 'object' || record.budgets === null) {
    issues.push({ path: 'budgets', message: 'must be an object' });
  } else {
    const budgets = record.budgets as Record<string, unknown>;
    if (budgets.maxTurns !== undefined) {
      issues.push({ path: 'budgets.maxTurns', message: 'is no longer supported; use maxCommandExecutions and maxEffectiveTokens' });
    }
    if (!Number.isInteger(budgets.maxCommandExecutions) || Number(budgets.maxCommandExecutions) <= 0) {
      issues.push({ path: 'budgets.maxCommandExecutions', message: 'must be a positive integer' });
    }
    if (!Number.isInteger(budgets.maxEffectiveTokens) || Number(budgets.maxEffectiveTokens) <= 0) {
      issues.push({ path: 'budgets.maxEffectiveTokens', message: 'must be a positive integer' });
    }
    if (!Number.isInteger(budgets.maxWallClockMs) || Number(budgets.maxWallClockMs) <= 0) {
      issues.push({ path: 'budgets.maxWallClockMs', message: 'must be a positive integer' });
    }
  }

  return issues;
}

function issuesToError(kind: string, filePath: string, issues: ValidationIssue[]): Error {
  const rendered = issues
    .map((issue) => `${issue.path || '<root>'}: ${issue.message}`)
    .join('; ');

  return new Error(`${kind} manifest ${filePath} is invalid: ${rendered}`);
}

export async function loadTaskManifest(filePath: string): Promise<TaskManifest> {
  const value = await readJsonFile<unknown>(filePath);
  const issues = validateTaskManifestShape(value);

  if (issues.length > 0) {
    throw issuesToError('task', filePath, issues);
  }

  return value as TaskManifest;
}

export async function loadTaskManifests(tasksDir: string): Promise<TaskManifest[]> {
  const entries = (await fs.readdir(tasksDir))
    .filter((entry) => entry.endsWith('.json'))
    .sort();

  const manifests = await Promise.all(entries.map((entry) => loadTaskManifest(path.join(tasksDir, entry))));
  const ids = new Set<string>();

  for (const manifest of manifests) {
    if (ids.has(manifest.id)) {
      throw new Error(`duplicate task id '${manifest.id}' in ${tasksDir}`);
    }
    ids.add(manifest.id);
  }

  return manifests;
}
