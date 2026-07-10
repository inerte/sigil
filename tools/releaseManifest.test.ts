import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { generateReleaseFiles } from "./releaseManifest.ts";

const VERSION = "2026-07-10T16-00-00Z";
const COMMIT = "a".repeat(40);
const platforms = [
  ["darwin-arm64", "darwin", "arm64", "aarch64-apple-darwin", "tar.gz"],
  ["darwin-x64", "darwin", "x64", "x86_64-apple-darwin", "tar.gz"],
  ["linux-arm64", "linux", "arm64", "aarch64-unknown-linux-gnu", "tar.gz"],
  ["linux-x64", "linux", "x64", "x86_64-unknown-linux-gnu", "tar.gz"],
  ["windows-x64", "windows", "x64", "x86_64-pc-windows-msvc", "zip"],
] as const;

function capabilities() {
  return {
    analysis: { level: "none", status: "notApplicable" },
    command: "sigil capabilities",
    compilerVersion: VERSION,
    data: { commands: [], compiler: { version: VERSION }, features: {}, output: { formatVersion: 1 }, phases: [] },
    diagnostics: [],
    formatVersion: 1,
    ok: true,
    phase: "cli",
  };
}

async function fixture() {
  const directory = await mkdtemp(join(tmpdir(), "sigil-release-manifest-"));
  for (const [id, os, architecture, target, archiveFormat] of platforms) {
    const file = `sigil-${VERSION}-${id}.${archiveFormat}`;
    await writeFile(join(directory, file), `archive:${id}`);
    await writeFile(
      join(directory, `${id}.artifact.json`),
      JSON.stringify({
        archiveFormat,
        capabilities: capabilities(),
        file,
        formatVersion: 1,
        platform: { architecture, id, os, target },
      }),
    );
  }
  return directory;
}

async function generate(directory: string, suffix: string) {
  return generateReleaseFiles({
    artifactsDir: directory,
    commit: COMMIT,
    manifestPath: join(directory, `release-manifest${suffix}.json`),
    repository: "inerte/sigil",
    sumsPath: join(directory, `SHA256SUMS${suffix}`),
    version: VERSION,
  });
}

test("manifest generation is canonical and complete", async () => {
  const directory = await fixture();
  try {
    const manifest = await generate(directory, "");
    assert.deepEqual(manifest.artifacts.map((artifact) => artifact.platform.id), platforms.map(([id]) => id));
    assert.equal(manifest.compiler.version, VERSION);
    assert.equal(manifest.source.commit, COMMIT);
    assert.match(manifest.artifacts[0].sha256, /^[0-9a-f]{64}$/);
  } finally {
    await rm(directory, { force: true, recursive: true });
  }
});

test("identical inputs produce identical manifest bytes", async () => {
  const directory = await fixture();
  try {
    await generate(directory, "-one");
    await generate(directory, "-two");
    assert.equal(
      await readFile(join(directory, "release-manifest-one.json"), "utf8"),
      await readFile(join(directory, "release-manifest-two.json"), "utf8"),
    );
  } finally {
    await rm(directory, { force: true, recursive: true });
  }
});

test("missing platform metadata is rejected", async () => {
  const directory = await fixture();
  try {
    await rm(join(directory, "windows-x64.artifact.json"));
    await assert.rejects(() => generate(directory, ""), /must contain exactly/);
  } finally {
    await rm(directory, { force: true, recursive: true });
  }
});

test("cross-platform capability drift is rejected", async () => {
  const directory = await fixture();
  try {
    const path = join(directory, "windows-x64.artifact.json");
    const metadata = JSON.parse(await readFile(path, "utf8"));
    metadata.capabilities.data.features = { windowsOnly: true };
    await writeFile(path, JSON.stringify(metadata));
    await assert.rejects(() => generate(directory, ""), /identical capabilities/);
  } finally {
    await rm(directory, { force: true, recursive: true });
  }
});
