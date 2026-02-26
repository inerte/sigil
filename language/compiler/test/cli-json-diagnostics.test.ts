import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import Ajv2020 from 'ajv/dist/2020.js';

function repoRoot(): string {
  return resolve(process.cwd(), '..', '..');
}

function runCli(args: string[]) {
  const cli = resolve(process.cwd(), 'dist/cli.js');
  const result = spawnSync('node', [cli, ...args], {
    cwd: repoRoot(),
    encoding: 'utf-8',
  });
  const stdout = result.stdout.trim();
  let payload: any = null;
  if (stdout.length > 0) {
    payload = JSON.parse(stdout);
  }
  return { result, payload };
}

describe('CLI JSON schema and diagnostics', () => {
  const schemaPath = resolve(repoRoot(), 'language/spec/cli-json.schema.json');
  const schema = JSON.parse(readFileSync(schemaPath, 'utf-8'));
  const ajv = new Ajv2020({ strict: false });
  const validate = ajv.compile(schema);

  test('formal CLI JSON schema parses and defines diagnostic suggestions', () => {
    assert.equal(schema.$schema, 'https://json-schema.org/draft/2020-12/schema');
    assert.ok(schema.$defs);
    assert.ok(schema.$defs.diagnostic);
    assert.ok(schema.$defs.suggestion);
  });

  test('schema validates a real compile success envelope and rejects malformed envelope', () => {
    const compileOk = runCli(['compile', 'projects/dungeon-random-rooms/src/main.sigil']);
    assert.equal(compileOk.result.status, 0);
    assert.equal(validate(compileOk.payload), true, JSON.stringify(validate.errors));

    const malformed = { formatVersion: 1, command: 'sigilc compile', ok: false };
    assert.equal(validate(malformed), false);
  });

  test('parser namespace separator error includes fixit and suggestions', () => {
    const dir = mkdtempSync(join(tmpdir(), 'sigil-cli-json-'));
    try {
      const file = join(dir, 'bad-import.sigil');
      writeFileSync(file, 'i stdlib/list\n\nλmain()→𝕌=()\n', 'utf-8');
      const { result, payload } = runCli(['parse', file]);
      assert.equal(result.status, 1);
      assert.equal(payload.ok, false);
      assert.equal(payload.error.code, 'SIGIL-PARSE-NS-SEP');
      assert.equal(validate(payload), true, JSON.stringify(validate.errors));
      assert.equal(payload.error.fixits[0].kind, 'replace');
      assert.equal(payload.error.suggestions[0].kind, 'replace_symbol');
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  test('canonical ordering error includes reorder suggestion', () => {
    const dir = mkdtempSync(join(tmpdir(), 'sigil-cli-json-'));
    try {
      const file = join(dir, 'canon-order.sigil');
      writeFileSync(file, 'c b:ℤ=1\nc a:ℤ=2\n\nλmain()→𝕌=()\n', 'utf-8');
      const { result, payload } = runCli(['compile', file]);
      assert.equal(result.status, 1);
      assert.equal(payload.error.code, 'SIGIL-CANON-DECL-ALPHABETICAL');
      assert.equal(payload.error.location.file, file);
      assert.equal(validate(payload), true, JSON.stringify(validate.errors));
      assert.equal(payload.error.suggestions[0].kind, 'reorder_declaration');
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  test('type missing export error includes suggestions', () => {
    const dir = mkdtempSync(join(tmpdir(), 'sigil-cli-json-'));
    try {
      const file = join(dir, 'type-missing-export.sigil');
      writeFileSync(file, 'i stdlib⋅list\n\nλmain()→ℤ=stdlib⋅list.nope([])\n', 'utf-8');
      const { result, payload } = runCli(['compile', file]);
      assert.equal(result.status, 1);
      assert.equal(payload.error.code, 'SIGIL-TYPE-MODULE-NOT-EXPORTED');
      assert.equal(payload.error.location.file, file);
      assert.equal(validate(payload), true, JSON.stringify(validate.errors));
      assert.ok(Array.isArray(payload.error.suggestions));
      assert.equal(payload.error.suggestions[0].kind, 'export_member');
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
