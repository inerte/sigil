#!/usr/bin/env node

import { copyFile, mkdtemp, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

function fail(message) {
  throw new Error(message);
}

function run(binary, args, cwd) {
  const result = spawnSync(binary, args, {
    cwd,
    encoding: "utf8",
    env: { ...process.env },
  });
  if (result.status !== 0) {
    fail(`${basename(binary)} ${args.join(" ")} failed (${result.status})\n${result.stdout}\n${result.stderr}`);
  }
  return result.stdout.trim();
}

async function requireFile(path) {
  try {
    if (!(await stat(path)).isFile()) fail(`required bundle path is not a file: ${path}`);
  } catch {
    fail(`missing required bundle file: ${path}`);
  }
}

async function requireDirectory(path) {
  try {
    if (!(await stat(path)).isDirectory()) fail(`required bundle path is not a directory: ${path}`);
  } catch {
    fail(`missing required bundle directory: ${path}`);
  }
}

function option(name) {
  const index = process.argv.indexOf(name);
  if (index < 0 || index + 1 >= process.argv.length) fail(`missing ${name}`);
  return process.argv[index + 1];
}

const root = resolve(option("--root"));
const version = option("--version");
const fixture = resolve(option("--fixture"));
const metadataPath = resolve(option("--metadata"));
const archive = option("--archive");
const archiveFormat = option("--archive-format");
const platform = {
  architecture: option("--architecture"),
  id: option("--platform-id"),
  os: option("--os"),
  target: option("--target"),
};
const binary = join(root, process.platform === "win32" ? "sigil.exe" : "sigil");

for (const path of [
  "language/core/prelude.lib.sigil",
  "language/stdlib/path.lib.sigil",
  "language/world/runtime.lib.sigil",
  "language/test/check/file.lib.sigil",
  "language/test/observe/file.lib.sigil",
  "runtime/node/package.json",
  "runtime/node/pty-runtime.mjs",
  "runtime/node/websocket-runtime.mjs",
  "runtime/node/fswatch-runtime.mjs",
  "runtime/node/sql-runtime.mjs",
]) {
  await requireFile(join(root, path));
}
await requireDirectory(join(root, "runtime/node/node_modules"));
await requireFile(binary);

const reportedVersion = run(binary, ["--version"], root);
if (reportedVersion !== `sigil ${version}`) {
  fail(`unexpected version output: ${reportedVersion}`);
}
const capabilities = JSON.parse(run(binary, ["capabilities"], root));
if (
  capabilities.formatVersion !== 1 ||
  capabilities.compilerVersion !== version ||
  capabilities.command !== "sigil capabilities" ||
  capabilities.ok !== true
) {
  fail("packaged binary returned an invalid capabilities response");
}
await writeFile(
  metadataPath,
  `${JSON.stringify(
    {
      archiveFormat,
      capabilities,
      file: archive,
      formatVersion: 1,
      platform,
    },
    null,
    2,
  )}\n`,
);

const smoke = await mkdtemp(join(tmpdir(), "sigil-release-smoke-"));
try {
  run(binary, ["init"], smoke);
  await writeFile(join(smoke, "src/main.sigil"), "λmain()=>Unit=()\n");
  await writeFile(
    join(smoke, "tests/basic.sigil"),
    'λmain()=>Unit=()\n\ntest "adds" {\n  1+1=2\n}\n',
  );
  run(binary, ["inspect", "codegen", "src/main.sigil"], smoke);
  run(binary, ["compile", "."], smoke);
  run(binary, ["test"], smoke);
  run(binary, ["run", "src/main.sigil"], smoke);
} finally {
  await rm(smoke, { force: true, recursive: true });
}

const runtimeSmoke = await mkdtemp(join(tmpdir(), "sigil-release-runtime-smoke-"));
try {
  const runtimeFixture = join(runtimeSmoke, "ptyBasics.sigil");
  await copyFile(fixture, runtimeFixture);
  run(binary, ["test", runtimeFixture], runtimeSmoke);
} finally {
  await rm(runtimeSmoke, { force: true, recursive: true });
}

process.stdout.write(`validated installed Sigil ${version} under ${root}\n`);
