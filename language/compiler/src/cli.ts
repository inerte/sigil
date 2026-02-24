#!/usr/bin/env node

/**
 * Sigil Compiler CLI
 */

import { readFileSync, writeFileSync, mkdirSync, readdirSync, statSync, existsSync } from 'fs';
import { dirname, basename, resolve, relative, join } from 'path';
import { spawn } from 'child_process';
import { pathToFileURL, fileURLToPath } from 'url';
import { tokenize } from './lexer/lexer.js';
import { tokenToString } from './lexer/token.js';
import { parse } from './parser/parser.js';
import { compile } from './codegen/javascript.js';
import { validateCanonicalForm } from './validator/canonical.js';
import { validateSurfaceForm } from './validator/surface-form.js';
import { validateExterns } from './validator/extern-validator.js';
import { typeCheck } from './typechecker/index.js';
import { formatType } from './typechecker/errors.js';
import type { InferenceType } from './typechecker/types.js';
import type * as AST from './parser/ast.js';
import { generateSemanticMap, enhanceWithClaude } from './mapgen/index.js';

type SigilProjectLayout = {
  src: string;
  tests: string;
  out: string;
};

type SigilProjectConfig = {
  root: string;
  layout: SigilProjectLayout;
};

type LoadedSigilModule = {
  id: string; // canonical module id (src/foo, stdlib/bar) or file path fallback for roots
  filePath: string;
  source: string;
  ast: ReturnType<typeof parse>;
  project?: SigilProjectConfig;
};

type ModuleGraph = {
  modules: Map<string, LoadedSigilModule>;
  topoOrder: string[]; // dependency-first
};

const LANGUAGE_ROOT_DIR = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');

function defaultProjectLayout(): SigilProjectLayout {
  return { src: 'src', tests: 'tests', out: '.local' };
}

function findSigilProjectRoot(startPath: string): string | null {
  let current = resolve(startPath);
  if (existsSync(current) && !statSync(current).isDirectory()) {
    current = dirname(current);
  }

  while (true) {
    if (existsSync(join(current, 'sigil.json'))) {
      return current;
    }
    const parent = dirname(current);
    if (parent === current) return null;
    current = parent;
  }
}

function getSigilProjectConfig(startPath: string): SigilProjectConfig | null {
  const root = findSigilProjectRoot(startPath);
  if (!root) return null;
  const raw = JSON.parse(readFileSync(join(root, 'sigil.json'), 'utf-8')) as any;
  const layout = {
    ...defaultProjectLayout(),
    ...(raw.layout ?? {})
  };
  return { root, layout };
}

function isPathWithin(parentDir: string, candidatePath: string): boolean {
  const rel = relative(resolve(parentDir), resolve(candidatePath));
  return rel === '' || (!rel.startsWith('..') && !rel.startsWith(`..${process.platform === 'win32' ? '\\' : '/'}`));
}

async function main() {
  const args = process.argv.slice(2);

  if (args.length === 0) {
    console.error('Usage: sigilc <command> [options]');
    console.error('');
    console.error('Commands:');
    console.error('  lex <file>        Tokenize a Sigil file');
    console.error('  parse <file>      Parse a Sigil file and show AST');
    console.error('  compile <file>    Compile a Sigil file to TypeScript');
    console.error('  run <file>        Compile and run a Sigil file');
    console.error('  test [path]       Run Sigil tests from ./tests (JSON output by default)');
    console.error('  help              Show this help message');
    process.exit(1);
  }

  const command = args[0];

  switch (command) {
    case 'lex':
      lexCommand(args.slice(1));
      break;
    case 'parse':
      parseCommand(args.slice(1));
      break;
    case 'compile':
      await compileCommand(args.slice(1));
      break;
    case 'run':
      await runCommand(args.slice(1));
      break;
    case 'test':
      await testCommand(args.slice(1));
      break;
    case 'help':
      console.log('Sigil Compiler v0.1.0');
      console.log('');
      console.log('Commands:');
      console.log('  lex <file>        Tokenize a Sigil file and print tokens');
      console.log('  parse <file>      Parse a Sigil file and show AST');
      console.log('  compile <file>    Compile a Sigil file to TypeScript');
      console.log('  run <file>        Compile and run a Sigil file');
      console.log('  test [path]       Run Sigil tests from the current Sigil project tests/ (JSON by default)');
      console.log('  help              Show this help message');
      console.log('');
      console.log('Output locations:');
      console.log('  Sigil project files → <project>/.local/... (detected via sigil.json)');
      console.log('  Non-project files  → .local/... (legacy fallback)');
      console.log('');
      console.log('Options:');
      console.log('  -o <file>         Specify custom output location');
      console.log('  --show-types      Display inferred types after type checking');
      console.log('  --json            JSON test output (default for sigilc test)');
      console.log('  --human           Human-readable test output');
      console.log('  --match <text>    Filter tests by substring (sigilc test)');
      break;
    default:
      console.error(`Unknown command: ${command}`);
      process.exit(1);
  }
}

function getTestsRootForPath(pathHint: string): string {
  const project = getSigilProjectConfig(pathHint) ?? getSigilProjectConfig(process.cwd());
  if (project) {
    return join(project.root, project.layout.tests);
  }
  return resolve(process.cwd(), 'tests');
}

function pathIsUnderTests(filename: string): boolean {
  const testsRoot = getTestsRootForPath(filename);
  const filePath = resolve(process.cwd(), filename);
  return isPathWithin(testsRoot, filePath);
}

function ensureNoTestsOutsideTestsDir(ast: ReturnType<typeof parse>, filename: string): void {
  const hasTests = ast.declarations.some(d => d.type === 'TestDecl');
  if (hasTests && !pathIsUnderTests(filename)) {
    throw new Error(`Test declarations are only allowed under ./tests (canonical project layout). Found test in ${filename}`);
  }
}

function isSigilImportPath(modulePath: string): boolean {
  return modulePath.startsWith('src/') || modulePath.startsWith('stdlib/');
}

function resolveSigilImportToFile(
  importerFile: string,
  importerProject: SigilProjectConfig | undefined,
  moduleId: string
): { moduleId: string; filePath: string; project?: SigilProjectConfig } {
  if (moduleId.startsWith('src/')) {
    if (!importerProject) {
      throw new Error(`Project import '${moduleId}' requires a Sigil project root (sigil.json). Importer: ${importerFile}`);
    }
    return {
      moduleId,
      filePath: join(importerProject.root, `${moduleId}.sigil`),
      project: importerProject,
    };
  }
  if (moduleId.startsWith('stdlib/')) {
    return {
      moduleId,
      filePath: join(LANGUAGE_ROOT_DIR, `${moduleId}.sigil`),
      project: importerProject,
    };
  }
  throw new Error(`Invalid Sigil import '${moduleId}'. Canonical Sigil imports are only 'src/...' and 'stdlib/...'`);
}

function buildModuleGraph(entryFile: string): ModuleGraph {
  const modules = new Map<string, LoadedSigilModule>();
  const topoOrder: string[] = [];
  const visiting = new Set<string>();
  const visitStack: string[] = [];

  const visit = (filePath: string, logicalId?: string, inheritedProject?: SigilProjectConfig): void => {
    const absFile = resolve(filePath);
    const moduleKey = logicalId ?? absFile;
    if (modules.has(moduleKey)) return;
    if (visiting.has(moduleKey)) {
      const startIdx = visitStack.indexOf(moduleKey);
      const cycle = (startIdx >= 0 ? visitStack.slice(startIdx) : [moduleKey]).concat(moduleKey);
      throw new Error(`Import cycle detected: ${cycle.join(' -> ')}`);
    }
    visiting.add(moduleKey);
    visitStack.push(moduleKey);

    const source = readFileSync(absFile, 'utf-8');
    validateSurfaceForm(source, absFile);
    const tokens = tokenize(source);
    const ast = parse(tokens);
    ensureNoTestsOutsideTestsDir(ast, absFile);
    validateCanonicalForm(ast);
    const project = getSigilProjectConfig(absFile) ?? inheritedProject;
    const mod: LoadedSigilModule = { id: moduleKey, filePath: absFile, source, ast, project: project ?? undefined };

    for (const decl of ast.declarations) {
      if (decl.type !== 'ImportDecl') continue;
      const importedId = decl.modulePath.join('/');
      if (!isSigilImportPath(importedId)) continue;
      const resolved = resolveSigilImportToFile(absFile, project ?? undefined, importedId);
      if (!existsSync(resolved.filePath)) {
        throw new Error(`Sigil import '${importedId}' not found for ${absFile}. Expected: ${resolved.filePath}`);
      }
      visit(resolved.filePath, resolved.moduleId, resolved.project);
    }

    visiting.delete(moduleKey);
    visitStack.pop();
    modules.set(moduleKey, mod);
    topoOrder.push(moduleKey);
  };

  visit(entryFile);
  return { modules, topoOrder };
}

function declIsExported(decl: AST.Declaration): boolean {
  return (decl.type === 'FunctionDecl' || decl.type === 'ConstDecl' || decl.type === 'TypeDecl') && !!decl.isExported;
}

function buildImportedNamespacesForModule(
  module: LoadedSigilModule,
  exportedNamespaces: Map<string, InferenceType>
): Map<string, InferenceType> {
  const imported = new Map<string, InferenceType>();
  for (const decl of module.ast.declarations) {
    if (decl.type !== 'ImportDecl') continue;
    const moduleId = decl.modulePath.join('/');
    if (!isSigilImportPath(moduleId)) continue;
    const nsType = exportedNamespaces.get(moduleId);
    if (nsType) {
      imported.set(moduleId, nsType);
    }
  }
  return imported;
}

function typeCheckModuleGraph(graph: ModuleGraph): Map<string, Map<string, InferenceType>> {
  const moduleTypes = new Map<string, Map<string, InferenceType>>();
  const exportedNamespaces = new Map<string, InferenceType>();

  for (const moduleId of graph.topoOrder) {
    const mod = graph.modules.get(moduleId)!;
    const importedNamespaces = buildImportedNamespacesForModule(mod, exportedNamespaces);
    const types = typeCheck(mod.ast, mod.source, { importedNamespaces });
    moduleTypes.set(moduleId, types);

    const fields = new Map<string, InferenceType>();
    for (const decl of mod.ast.declarations) {
      if (!declIsExported(decl)) continue;
      if (decl.type === 'FunctionDecl' || decl.type === 'ConstDecl') {
        const t = types.get(decl.name);
        if (t) fields.set(decl.name, t);
      }
      // Exported types are handled syntactically/canonically now; cross-module type refs are a future extension.
    }
    exportedNamespaces.set(moduleId, { kind: 'record', fields });
  }

  return moduleTypes;
}

function getModuleOutputPath(entryFile: string, mod: LoadedSigilModule, rootProject?: SigilProjectConfig, rootOutputOverride?: string): string {
  if (rootOutputOverride && resolve(mod.filePath) === resolve(entryFile)) {
    return rootOutputOverride;
  }

  if (mod.id.startsWith('stdlib/') && rootProject) {
    return join(rootProject.root, rootProject.layout.out, `${mod.id}.ts`);
  }

  return getSmartOutputPath(mod.filePath);
}

async function compileModuleGraph(entryFile: string, rootOutputOverride?: string): Promise<{
  graph: ModuleGraph;
  moduleTypes: Map<string, Map<string, InferenceType>>;
  outputs: Map<string, string>;
  rootModule: LoadedSigilModule;
}> {
  const graph = buildModuleGraph(entryFile);
  const moduleTypes = typeCheckModuleGraph(graph);
  const rootModule = graph.modules.get(resolve(entryFile)) ?? graph.modules.get(entryFile) ?? (() => { throw new Error('Root module missing'); })();
  const rootProject = getSigilProjectConfig(entryFile) ?? undefined;
  const outputs = new Map<string, string>();

  for (const moduleId of graph.topoOrder) {
    const mod = graph.modules.get(moduleId)!;
    await validateExterns(mod.ast);
    const outputFile = getModuleOutputPath(entryFile, mod, rootProject, rootOutputOverride);
    const outputDir = dirname(outputFile);
    if (outputDir !== '.') {
      mkdirSync(outputDir, { recursive: true });
    }
    const tsCode = compile(mod.ast, {
      sourceFile: mod.filePath,
      outputFile,
      projectRoot: rootProject?.root ?? mod.project?.root,
    });
    writeFileSync(outputFile, tsCode, 'utf-8');
    outputs.set(moduleId, outputFile);
  }

  return { graph, moduleTypes, outputs, rootModule };
}

function collectSigilFiles(rootPath: string): string[] {
  const results: string[] = [];
  const st = statSync(rootPath);
  if (st.isFile()) {
    if (rootPath.endsWith('.sigil')) {
      results.push(rootPath);
    }
    return results;
  }
  for (const entry of readdirSync(rootPath)) {
    const full = join(rootPath, entry);
    const est = statSync(full);
    if (est.isDirectory()) {
      results.push(...collectSigilFiles(full));
    } else if (est.isFile() && full.endsWith('.sigil')) {
      results.push(full);
    }
  }
  return results;
}

function lexCommand(args: string[]) {
  if (args.length === 0) {
    console.error('Usage: sigilc lex <file>');
    process.exit(1);
  }

  const filename = args[0];

  try {
    const source = readFileSync(filename, 'utf-8');

    // Validate surface form (formatting) before tokenizing
    validateSurfaceForm(source, filename);

    const tokens = tokenize(source);

    console.log(`Tokens for ${filename}:`);
    console.log('');

    for (const token of tokens) {
      console.log(tokenToString(token));
    }

    console.log('');
    console.log(`Total tokens: ${tokens.length}`);
  } catch (error) {
    if (error instanceof Error) {
      console.error(`Error: ${error.message}`);
    } else {
      console.error(`Unknown error: ${error}`);
    }
    process.exit(1);
  }
}

function parseCommand(args: string[]) {
  if (args.length === 0) {
    console.error('Usage: sigilc parse <file>');
    process.exit(1);
  }

  const filename = args[0];

  try {
    const source = readFileSync(filename, 'utf-8');

    // Validate surface form (formatting) before tokenizing
    validateSurfaceForm(source, filename);

    const tokens = tokenize(source);

    console.log(`Parsing ${filename}...`);
    console.log(`Total tokens: ${tokens.length}`);

    const ast = parse(tokens);
    ensureNoTestsOutsideTestsDir(ast, filename);

    console.log('');
    console.log(`AST for ${filename}:`);
    console.log('');
    console.log(JSON.stringify(ast, null, 2));
  } catch (error) {
    if (error instanceof Error) {
      console.error(`Error: ${error.message}`);
      console.error(error.stack);
    } else {
      console.error(`Unknown error: ${error}`);
    }
    process.exit(1);
  }
}

/**
 * Determine smart output location based on input file location
 */
function getSmartOutputPath(inputFile: string): string {
  const project = getSigilProjectConfig(inputFile);
  if (project && isPathWithin(project.root, resolve(process.cwd(), inputFile))) {
    const relToProject = relative(project.root, resolve(process.cwd(), inputFile)).replace(/\\/g, '/');
    return join(project.root, project.layout.out, relToProject.replace(/\.sigil$/, '.ts'));
  }

  // examples/*.sigil → examples/*.ts (beside source, for documentation)
  if (inputFile.startsWith('examples/')) {
    return inputFile.replace(/\.sigil$/, '.ts');
  }

  // Everything else → .local/ (keeps root clean)
  // src/**/*.sigil → .local/src/**/*.ts
  // *.sigil → .local/*.ts
  return `.local/${inputFile.replace(/\.sigil$/, '.ts')}`;
}

async function compileCommand(args: string[]) {
  if (args.length === 0) {
    console.error('Usage: sigilc compile <file> [-o output.ts]');
    process.exit(1);
  }

  const filename = args[0];

  // Check for -o flag first
  const outputIndex = args.indexOf('-o');
  let outputFile: string;

  if (outputIndex !== -1 && args[outputIndex + 1]) {
    outputFile = args[outputIndex + 1];
  } else {
    // Use smart defaults
    outputFile = getSmartOutputPath(filename);
  }

  try {
    const { graph, moduleTypes, outputs } = await compileModuleGraph(filename, outputFile);
    const rootKey = resolve(filename);
    const rootModule = graph.modules.get(rootKey) ?? (() => { throw new Error(`Root module not loaded: ${filename}`); })();
    const ast = rootModule.ast;
    const source = rootModule.source;
    const types = moduleTypes.get(rootKey) ?? new Map<string, InferenceType>();
    outputFile = outputs.get(rootKey) ?? outputFile;

    // Type check results for root already available
    const showTypes = args.includes('--show-types');

    // If --show-types flag, display inferred types
    if (showTypes) {
      console.log('\n✓ Type checked successfully\n');
      console.log('Inferred types:');
      for (const [name, type] of types) {
        const typeStr = formatType(type);
        console.log(`  ${name} : ${typeStr}`);
      }
      console.log();
    }

    console.log(`✓ Compiled ${filename} → ${outputFile}`);

    // Generate semantic map
    const mapFile = filename.replace('.sigil', '.sigil.map');
    generateSemanticMap(ast, types, source, mapFile);
    console.log(`✓ Generated basic semantic map → ${mapFile}`);

    // Enhance with Claude Code CLI
    enhanceWithClaude(filename, mapFile);
    if (process.env.SIGIL_ENABLE_MAP_ENHANCE === '1') {
      console.log(`✓ Enhanced semantic map with AI documentation`);
    } else {
      console.log('✓ Skipped AI semantic map enhancement (set SIGIL_ENABLE_MAP_ENHANCE=1 to enable)');
    }
  } catch (error) {
    if (error instanceof Error) {
      console.error(`Error: ${error.message}`);
      console.error(error.stack);
    } else {
      console.error(`Unknown error: ${error}`);
    }
    process.exit(1);
  }
}

type CompiledTestModule = {
  sourceFile: string;
  outputFile: string;
};

async function compileToTypeScriptFile(filename: string, outputFile?: string): Promise<CompiledTestModule> {
  const finalOutput = outputFile ?? getSmartOutputPath(filename);
  const { outputs } = await compileModuleGraph(filename, finalOutput);
  const rootOut = outputs.get(resolve(filename)) ?? finalOutput;
  return { sourceFile: filename, outputFile: rootOut };
}

async function runCommand(args: string[]) {
  if (args.length === 0) {
    console.error('Usage: sigilc run <file>');
    process.exit(1);
  }

  const filename = args[0];
  const outputFile = getSmartOutputPath(filename);
  const runnerFile = outputFile.replace(/\.ts$/, '.run.ts');

  try {
    // Compile root module and imported Sigil dependencies into .local/
    const { outputs } = await compileModuleGraph(filename, outputFile);
    const actualOutput = outputs.get(resolve(filename)) ?? outputFile;

    // Create runner that calls main()
    const runnerImport = `./${basename(actualOutput, '.ts')}`;
    const runnerCode = `import { main } from '${runnerImport}';

if (typeof main !== 'function') {
  console.error('Error: No main() function found in ${filename}');
  console.error('Add a main() function to make this program runnable.');
  process.exit(1);
}

// Call main and handle the result
const result = main();

// If main returns a value (not Unit/undefined), show it
if (result !== undefined) {
  console.log(result);
}
`;

    writeFileSync(runnerFile, runnerCode, 'utf-8');

    console.log(`✓ Compiled ${filename} → ${actualOutput}`);
    console.log('');

    // Run the wrapper with Node + tsx loader (avoids tsx CLI IPC/daemon issues in sandboxed environments)
    const nodeProcess = spawn('pnpm', ['exec', 'node', '--import', 'tsx', runnerFile], {
      stdio: 'inherit',
      shell: false,
    });

    nodeProcess.on('exit', (code) => {
      process.exit(code || 0);
    });

    nodeProcess.on('error', (error) => {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
        console.error('Failed to run: pnpm, node, and/or tsx is not available on PATH.');
        console.error('Install tsx with: pnpm add -D tsx');
      } else {
        console.error(`Failed to run: ${error.message}`);
      }
      process.exit(1);
    });
  } catch (error) {
    if (error instanceof Error) {
      console.error(`Error: ${error.message}`);
      console.error(error.stack);
    } else {
      console.error(`Unknown error: ${error}`);
    }
    process.exit(1);
  }
}

async function runGeneratedTestModule(moduleFile: string, matchText: string | null): Promise<any> {
  const runnerDir = join(dirname(moduleFile), '__sigil_test');
  mkdirSync(runnerDir, { recursive: true });
  const unique = `${process.pid}_${Date.now()}_${Math.floor(Math.random() * 1_000_000)}`;
  const runnerFile = `${runnerDir}/${basename(moduleFile, '.ts')}.${unique}.runner.ts`;
  const moduleUrl = pathToFileURL(resolve(process.cwd(), moduleFile)).href;
  const runnerCode =
    `const moduleUrl = ${JSON.stringify(moduleUrl)};\n` +
    `const discoverMod = await import(moduleUrl);\n` +
    `const tests = Array.isArray(discoverMod.__sigil_tests) ? discoverMod.__sigil_tests : [];\n` +
    `const matchText = ${JSON.stringify(matchText)};\n` +
    `const selected = matchText ? tests.filter((t) => String(t.name).includes(matchText)) : tests;\n` +
    `const results = [];\n` +
    `const startSuite = Date.now();\n` +
    `for (const t of selected) {\n` +
    `  const start = Date.now();\n` +
    `  try {\n` +
    `    const freshMod = await import(moduleUrl + '?sigil_test=' + encodeURIComponent(String(t.id)) + '&ts=' + Date.now() + '_' + Math.random());\n` +
    `    const freshTests = Array.isArray(freshMod.__sigil_tests) ? freshMod.__sigil_tests : [];\n` +
    `    const freshTest = freshTests.find((x) => x.id === t.id);\n` +
    `    if (!freshTest) { throw new Error('Test not found in isolated module reload: ' + String(t.id)); }\n` +
    `    const value = await freshTest.fn();\n` +
    `    if (value === true) {\n` +
    `      results.push({ id: t.id, file: String(t.id).split('::')[0], name: t.name, status: 'pass', durationMs: Date.now()-start, location: t.location, declaredEffects: t.declaredEffects ?? [], assertion: t.assertion ?? null });\n` +
    `    } else if (value && typeof value === 'object' && 'ok' in value) {\n` +
    `      if (value.ok === true) {\n` +
    `        results.push({ id: t.id, file: String(t.id).split('::')[0], name: t.name, status: 'pass', durationMs: Date.now()-start, location: t.location, declaredEffects: t.declaredEffects ?? [], assertion: t.assertion ?? null });\n` +
    `      } else {\n` +
    `        results.push({ id: t.id, file: String(t.id).split('::')[0], name: t.name, status: 'fail', durationMs: Date.now()-start, location: t.location, declaredEffects: t.declaredEffects ?? [], assertion: t.assertion ?? null, failure: value.failure ?? { kind: 'assert_false', message: 'Test body evaluated to ⊥' } });\n` +
    `      }\n` +
    `    } else {\n` +
    `      results.push({ id: t.id, file: String(t.id).split('::')[0], name: t.name, status: 'fail', durationMs: Date.now()-start, location: t.location, declaredEffects: t.declaredEffects ?? [], assertion: t.assertion ?? null, failure: { kind: 'assert_false', message: 'Test body evaluated to ⊥' } });\n` +
    `    }\n` +
    `  } catch (e) {\n` +
    `    results.push({ id: t.id, file: String(t.id).split('::')[0], name: t.name, status: 'error', durationMs: Date.now()-start, location: t.location, declaredEffects: t.declaredEffects ?? [], assertion: t.assertion ?? null, failure: { kind: 'exception', message: e instanceof Error ? e.message : String(e) } });\n` +
    `  }\n` +
    `}\n` +
    `console.log(JSON.stringify({ results, discovered: tests.length, selected: selected.length, durationMs: Date.now()-startSuite }));\n`;
  writeFileSync(runnerFile, runnerCode, 'utf-8');

  const data = await new Promise<string>((resolveOut, reject) => {
    let stdout = '';
    let stderr = '';
    const p = spawn('pnpm', ['exec', 'node', '--import', 'tsx', runnerFile], {
      stdio: ['ignore', 'pipe', 'pipe'],
      shell: false,
    });
    p.stdout.on('data', (d) => { stdout += d.toString(); });
    p.stderr.on('data', (d) => { stderr += d.toString(); });
    p.on('error', reject);
    p.on('exit', (code) => {
      if (code !== 0) {
        reject(new Error(stderr || `Test runner exited with code ${code}`));
        return;
      }
      resolveOut(stdout.trim());
    });
  });

  return JSON.parse(data);
}

async function testCommand(args: string[]) {
  const human = args.includes('--human');
  const jsonMode = !human;
  const matchIndex = args.indexOf('--match');
  const matchText = matchIndex !== -1 && args[matchIndex + 1] ? args[matchIndex + 1] : null;
  const pathArg = args.find((a, i) => {
    if (a.startsWith('--')) return false;
    if (matchIndex !== -1 && i === matchIndex + 1) return false;
    return true;
  });
  const rootPath = pathArg ?? getTestsRootForPath(process.cwd());

  try {
    if (!pathIsUnderTests(rootPath)) {
      throw new Error(`sigilc test only accepts paths under ./tests. Got: ${rootPath}`);
    }

    if (!existsSync(rootPath)) {
      const empty = {
        formatVersion: 1,
        command: 'sigilc test',
        ok: true,
        summary: { files: 0, discovered: 0, selected: 0, passed: 0, failed: 0, errored: 0, skipped: 0, durationMs: 0 },
        results: []
      };
      if (jsonMode) {
        process.stdout.write(JSON.stringify(empty) + '\n');
      } else {
        console.log('No tests found (./tests does not exist).');
      }
      process.exit(0);
    }

    const files = collectSigilFiles(rootPath).sort();
    const started = Date.now();
    const allResults: any[] = [];
    let discovered = 0;
    let selected = 0;

    const fileRuns = await Promise.all(files.map(async (file) => {
      const out = getSmartOutputPath(file);
      const compiled = await compileToTypeScriptFile(file, out);
      const moduleResult = await runGeneratedTestModule(compiled.outputFile, matchText);
      return { file, moduleResult };
    }));

    for (const { moduleResult } of fileRuns) {
      discovered += moduleResult.discovered ?? 0;
      selected += moduleResult.selected ?? 0;
      allResults.push(...(moduleResult.results ?? []));
    }

    allResults.sort((a, b) => {
      const fileCmp = String(a.file).localeCompare(String(b.file));
      if (fileCmp !== 0) return fileCmp;
      const aLine = a.location?.start?.line ?? 0;
      const bLine = b.location?.start?.line ?? 0;
      if (aLine !== bLine) return aLine - bLine;
      const aCol = a.location?.start?.column ?? 0;
      const bCol = b.location?.start?.column ?? 0;
      if (aCol !== bCol) return aCol - bCol;
      return String(a.name).localeCompare(String(b.name));
    });

    const passed = allResults.filter(r => r.status === 'pass').length;
    const failed = allResults.filter(r => r.status === 'fail').length;
    const errored = allResults.filter(r => r.status === 'error').length;
    const payload = {
      formatVersion: 1,
      command: 'sigilc test',
      ok: failed === 0 && errored === 0,
      summary: {
        files: files.length,
        discovered,
        selected,
        passed,
        failed,
        errored,
        skipped: 0,
        durationMs: Date.now() - started
      },
      results: allResults
    };

    if (jsonMode) {
      process.stdout.write(JSON.stringify(payload) + '\n');
    } else {
      console.log(`${payload.ok ? 'PASS' : 'FAIL'} ${passed}/${selected} tests passed`);
      for (const r of allResults) {
        if (r.status !== 'pass') {
          console.log(`${r.status.toUpperCase()}: ${r.name} (${r.file})${r.failure?.message ? ` - ${r.failure.message}` : ''}`);
        }
      }
    }
    process.exit(payload.ok ? 0 : 1);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (jsonMode) {
      process.stdout.write(JSON.stringify({
        formatVersion: 1,
        command: 'sigilc test',
        ok: false,
        summary: { files: 0, discovered: 0, selected: 0, passed: 0, failed: 0, errored: 1, skipped: 0, durationMs: 0 },
        results: [],
        error: { kind: 'runner_error', message }
      }) + '\n');
    } else {
      console.error(`Error: ${message}`);
    }
    process.exit(2);
  }
}

main();
