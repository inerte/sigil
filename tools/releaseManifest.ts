import { createHash } from "node:crypto";
import { readFile, readdir, stat, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";
import { fileURLToPath } from "node:url";

const PLATFORM_IDS = [
  "darwin-arm64",
  "darwin-x64",
  "linux-arm64",
  "linux-x64",
  "windows-x64",
] as const;

type JsonObject = Record<string, unknown>;

type ArtifactMetadata = {
  archiveFormat: "tar.gz" | "zip";
  capabilities: JsonObject;
  file: string;
  formatVersion: 1;
  platform: {
    architecture: "arm64" | "x64";
    id: (typeof PLATFORM_IDS)[number];
    os: "darwin" | "linux" | "windows";
    target: string;
  };
};

export type ReleaseManifest = {
  artifacts: Array<{
    archiveFormat: "tar.gz" | "zip";
    file: string;
    platform: ArtifactMetadata["platform"];
    sha256: string;
    sizeBytes: number;
  }>;
  compiler: {
    capabilities: JsonObject;
    version: string;
  };
  formatVersion: 1;
  source: {
    commit: string;
    repository: string;
  };
  version: string;
};

function assertObject(value: unknown, label: string): asserts value is JsonObject {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
}

function assertKeys(value: JsonObject, expected: string[], label: string): void {
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    throw new Error(`${label} fields must be exactly: ${wanted.join(", ")}`);
  }
}

function assertString(value: unknown, label: string): asserts value is string {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`${label} must be a non-empty string`);
  }
}

function canonicalJson(value: unknown): string {
  if (Array.isArray(value)) {
    return `[${value.map(canonicalJson).join(",")}]`;
  }
  if (value !== null && typeof value === "object") {
    return `{${Object.entries(value as JsonObject)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, item]) => `${JSON.stringify(key)}:${canonicalJson(item)}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
}

function parseMetadata(value: unknown, source: string): ArtifactMetadata {
  assertObject(value, source);
  assertKeys(
    value,
    ["archiveFormat", "capabilities", "file", "formatVersion", "platform"],
    source,
  );
  if (value.formatVersion !== 1) {
    throw new Error(`${source} formatVersion must be 1`);
  }
  if (value.archiveFormat !== "tar.gz" && value.archiveFormat !== "zip") {
    throw new Error(`${source} archiveFormat must be tar.gz or zip`);
  }
  assertString(value.file, `${source}.file`);
  assertObject(value.capabilities, `${source}.capabilities`);
  assertObject(value.platform, `${source}.platform`);
  assertKeys(
    value.platform,
    ["architecture", "id", "os", "target"],
    `${source}.platform`,
  );
  if (!PLATFORM_IDS.includes(value.platform.id as (typeof PLATFORM_IDS)[number])) {
    throw new Error(`${source}.platform.id is unsupported`);
  }
  if (value.platform.os !== "darwin" && value.platform.os !== "linux" && value.platform.os !== "windows") {
    throw new Error(`${source}.platform.os is unsupported`);
  }
  if (value.platform.architecture !== "arm64" && value.platform.architecture !== "x64") {
    throw new Error(`${source}.platform.architecture is unsupported`);
  }
  assertString(value.platform.target, `${source}.platform.target`);
  return value as ArtifactMetadata;
}

async function sha256(path: string): Promise<string> {
  return createHash("sha256").update(await readFile(path)).digest("hex");
}

function validateCapabilities(capabilities: JsonObject, version: string, source: string): JsonObject {
  if (
    capabilities.formatVersion !== 1 ||
    capabilities.command !== "sigil capabilities" ||
    capabilities.ok !== true ||
    capabilities.compilerVersion !== version
  ) {
    throw new Error(`${source} is not a successful canonical capabilities response for ${version}`);
  }
  assertObject(capabilities.data, `${source}.data`);
  return capabilities.data;
}

function expectedArchiveName(version: string, metadata: ArtifactMetadata): string {
  return `sigil-${version}-${metadata.platform.id}.${metadata.archiveFormat}`;
}

export async function generateReleaseFiles(options: {
  artifactsDir: string;
  commit: string;
  manifestPath: string;
  repository: string;
  sumsPath: string;
  version: string;
}): Promise<ReleaseManifest> {
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}-\d{2}-\d{2}Z$/.test(options.version)) {
    throw new Error("version must use Sigil's canonical UTC timestamp format");
  }
  if (!/^[0-9a-f]{40}$/.test(options.commit)) {
    throw new Error("commit must be a full lowercase Git SHA");
  }
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(options.repository)) {
    throw new Error("repository must use owner/name form");
  }

  const metadataFiles = (await readdir(options.artifactsDir))
    .filter((name) => name.endsWith(".artifact.json"))
    .sort();
  const metadata = await Promise.all(
    metadataFiles.map(async (name) =>
      parseMetadata(
        JSON.parse(await readFile(join(options.artifactsDir, name), "utf8")),
        name,
      ),
    ),
  );
  const ids = metadata.map((item) => item.platform.id).sort();
  if (JSON.stringify(ids) !== JSON.stringify([...PLATFORM_IDS].sort())) {
    throw new Error(`artifact metadata must contain exactly: ${PLATFORM_IDS.join(", ")}`);
  }

  let canonicalCapabilities = "";
  let capabilityData: JsonObject | undefined;
  const artifacts = [] as ReleaseManifest["artifacts"];
  for (const platformId of PLATFORM_IDS) {
    const item = metadata.find((candidate) => candidate.platform.id === platformId)!;
    const expectedName = expectedArchiveName(options.version, item);
    if (item.file !== expectedName || basename(item.file) !== item.file) {
      throw new Error(`${platformId} archive must be named ${expectedName}`);
    }
    const currentCapabilities = canonicalJson(item.capabilities);
    const currentData = validateCapabilities(item.capabilities, options.version, `${platformId}.capabilities`);
    if (canonicalCapabilities.length === 0) {
      canonicalCapabilities = currentCapabilities;
      capabilityData = currentData;
    } else if (currentCapabilities !== canonicalCapabilities) {
      throw new Error("all release binaries must report identical capabilities");
    }
    const archivePath = join(options.artifactsDir, item.file);
    const archiveStat = await stat(archivePath);
    if (!archiveStat.isFile()) {
      throw new Error(`${item.file} must be a file`);
    }
    artifacts.push({
      archiveFormat: item.archiveFormat,
      file: item.file,
      platform: item.platform,
      sha256: await sha256(archivePath),
      sizeBytes: archiveStat.size,
    });
  }

  const manifest: ReleaseManifest = {
    artifacts,
    compiler: {
      capabilities: capabilityData!,
      version: options.version,
    },
    formatVersion: 1,
    source: {
      commit: options.commit,
      repository: options.repository,
    },
    version: options.version,
  };
  await writeFile(options.manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  const checksumFiles = [...artifacts.map((artifact) => artifact.file), basename(options.manifestPath)].sort();
  const sums = await Promise.all(
    checksumFiles.map(async (name) => `${await sha256(join(options.artifactsDir, name))}  ${name}`),
  );
  await writeFile(options.sumsPath, `${sums.join("\n")}\n`);
  return manifest;
}

function option(args: string[], name: string): string {
  const index = args.indexOf(name);
  if (index < 0 || index + 1 >= args.length) {
    throw new Error(`missing ${name}`);
  }
  return args[index + 1];
}

async function main(args: string[]): Promise<void> {
  if (args[0] !== "generate") {
    throw new Error("usage: releaseManifest.ts generate --artifacts-dir DIR --commit SHA --manifest PATH --repository OWNER/REPO --sums PATH --version VERSION");
  }
  const artifactsDir = option(args, "--artifacts-dir");
  await generateReleaseFiles({
    artifactsDir,
    commit: option(args, "--commit"),
    manifestPath: option(args, "--manifest"),
    repository: option(args, "--repository"),
    sumsPath: option(args, "--sums"),
    version: option(args, "--version"),
  });
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).catch((error: unknown) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
