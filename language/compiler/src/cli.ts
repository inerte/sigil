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
import { qualifyTypeDef } from './typechecker/bidirectional.js';
import { formatType } from './typechecker/errors.js';
import type { InferenceType } from './typechecker/types.js';
import type { TypeInfo } from './typechecker/index.js';
import type * as AST from './parser/ast.js';
import { generateSemanticMap, enhanceWithClaude } from './mapgen/index.js';
import { SigilDiagnosticError, isSigilDiagnosticError } from './diagnostics/error.js';
import type { CommandEnvelope, Diagnostic, SigilPhase } from './diagnostics/types.js';
import { diagnostic } from './diagnostics/helpers.js';

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

/**
 * Convert internal module path (/ separator) to file path (/ separator)
 * Example: "src/types" → "src/types"
 */
function modulePathToFilePath(moduleId: string): string {
  return moduleId;
}

/**
 * Convert an absolute file path to a logical module ID
 * Returns undefined if the file is not a stdlib or src module
 */
function filePathToModuleId(absFilePath: string, project?: SigilProjectConfig): string | undefined {
  // Check if it's a stdlib module
  if (absFilePath.startsWith(LANGUAGE_ROOT_DIR)) {
    const relativePath = relative(LANGUAGE_ROOT_DIR, absFilePath);
    if (relativePath.endsWith('.sigil')) {
      const withoutExt = relativePath.slice(0, -6); // Remove .sigil
      return withoutExt;
    }
  }

  // Check if it's a src module (project module)
  if (project && absFilePath.startsWith(project.root)) {
    const relativePath = relative(project.root, absFilePath);
    if (relativePath.endsWith('.sigil')) {
      const withoutExt = relativePath.slice(0, -6); // Remove .sigil
      return withoutExt;
    }
  }

  return undefined;
}

function hasFlag(args: string[], flag: string): boolean {
  return args.includes(flag);
}

function rejectRemovedJsonFlag(args: string[]): void {
  if (hasFlag(args, '--json')) {
    throw new SigilDiagnosticError(diagnostic('SIGIL-CLI-UNSUPPORTED-OPTION', 'cli', 'unsupported option', {
      found: '--json',
      expected: '--human'
    }));
  }
}

function stripFlag(args: string[], flag: string): string[] {
  return args.filter(a => a !== flag);
}

function formatLocation(loc?: Diagnostic['location']): string {
  if (!loc) return '';
  return `${loc.file}:${loc.start.line}:${loc.start.column}`;
}

function renderHumanEnvelope(envelope: CommandEnvelope): string {
  if (envelope.ok) {
    const parts = [`${envelope.command} OK`];
    if (envelope.phase) parts.push(`phase=${envelope.phase}`);
    return parts.join(' ');
  }
  const err = envelope.error;
  if (!err) return `${envelope.command} FAIL`;
  const bits = [err.code];
  const loc = formatLocation(err.location);
  if (loc) bits.push(loc);
  bits.push(err.message);
  if (err.found !== undefined || err.expected !== undefined) {
    bits.push(`(found ${JSON.stringify(err.found)}, expected ${JSON.stringify(err.expected)})`);
  }
  return bits.join(' ');
}

function emitEnvelope<T>(envelope: CommandEnvelope<T>, human: boolean): never {
  if (human) {
    if (envelope.command === 'sigilc parse' && envelope.ok && envelope.data && typeof envelope.data === 'object' && 'ast' in (envelope.data as any)) {
      console.log(renderHumanEnvelope(envelope));
      console.log(JSON.stringify((envelope.data as any).ast, null, 2));
    } else if (envelope.command === 'sigilc lex' && envelope.ok && envelope.data && typeof envelope.data === 'object' && Array.isArray((envelope.data as any).tokens)) {
      console.log(renderHumanEnvelope(envelope));
      for (const t of (envelope.data as any).tokens) {
        console.log(`${t.type}(${t.lexeme}) at ${t.start.line}:${t.start.column}`);
      }
    } else if (envelope.command === 'sigilc run' && envelope.ok && envelope.data && typeof envelope.data === 'object') {
      const runtime = (envelope.data as any).runtime;
      if (runtime?.stdout) process.stdout.write(runtime.stdout);
      if (runtime?.stderr) process.stderr.write(runtime.stderr);
      console.log(renderHumanEnvelope(envelope));
    } else {
      console.log(renderHumanEnvelope(envelope));
    }
  } else {
    process.stdout.write(JSON.stringify(envelope) + '\n');
  }
  process.exit(envelope.ok ? 0 : 1);
}

function unknownToDiagnostic(error: unknown, phase: SigilPhase = 'cli', filename?: string): Diagnostic {
  if (isSigilDiagnosticError(error)) {
    const d = { ...error.diagnostic };
    if (filename && d.location && d.location.file === '<unknown>') {
      d.location = { ...d.location, file: filename };
    }
    return d;
  }
  if (error instanceof Error) {
    return diagnostic('SIGIL-CLI-UNEXPECTED', phase, error.message);
  }
  return diagnostic('SIGIL-CLI-UNEXPECTED', phase, String(error));
}

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
    emitEnvelope({
      formatVersion: 1,
      command: 'sigilc',
      ok: false,
      phase: 'cli',
      error: diagnostic('SIGIL-CLI-USAGE', 'cli', 'missing command', {
        expected: ['lex', 'parse', 'compile', 'run', 'test', 'help']
      })
    }, false);
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
      console.log('  lex <file>        Tokenize a Sigil file (JSON by default)');
      console.log('  parse <file>      Parse a Sigil file (JSON by default)');
      console.log('  compile <file>    Compile a Sigil file to TypeScript (JSON by default)');
      console.log('  run <file>        Compile and run a Sigil file (JSON by default)');
      console.log('  test [path]       Run Sigil tests from the current Sigil project tests/ (JSON by default)');
      console.log('  help              Show this help message');
      console.log('');
      console.log('Output locations:');
      console.log('  Sigil project files → <project>/.local/... (detected via sigil.json)');
      console.log('  Non-project files  → .local/... (legacy fallback)');
      console.log('');
      console.log('Options:');
      console.log('  -o <file>         Specify custom output location');
      console.log('  --show-types      Include inferred types in compile JSON output');
      console.log('  --human           Human-readable output (derived from JSON payloads)');
      console.log('  --match <text>    Filter tests by substring (sigilc test)');
      break;
    default:
      emitEnvelope({
        formatVersion: 1,
        command: 'sigilc',
        ok: false,
        phase: 'cli',
        error: diagnostic('SIGIL-CLI-UNKNOWN-COMMAND', 'cli', 'unknown command', {
          found: command,
          expected: ['lex', 'parse', 'compile', 'run', 'test', 'help']
        })
      }, hasFlag(args, '--human'));
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
    throw new SigilDiagnosticError(diagnostic('SIGIL-CANON-TEST-PATH', 'canonical', 'test declarations are only allowed under project tests/', {
      details: { file: filename, testsRoot: getTestsRootForPath(filename) }
    }));
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
  // Internal module IDs already use slash separators.
  const filePathStr = modulePathToFilePath(moduleId);

  if (moduleId.startsWith('src/')) {
    if (!importerProject) {
      throw new SigilDiagnosticError(diagnostic('SIGIL-CLI-PROJECT-ROOT-REQUIRED', 'cli', 'project import requires sigil project root', {
        details: { moduleId, importerFile }
      }));
    }
    return {
      moduleId,
      filePath: join(importerProject.root, `${filePathStr}.sigil`),
      project: importerProject,
    };
  }
  if (moduleId.startsWith('stdlib/')) {
    return {
      moduleId,
      filePath: join(LANGUAGE_ROOT_DIR, `${filePathStr}.sigil`),
      project: importerProject,
    };
  }
  throw new SigilDiagnosticError(diagnostic('SIGIL-CLI-INVALID-IMPORT', 'cli', 'invalid sigil import module id', {
    found: moduleId,
    expected: ['src/...', 'stdlib/...']
  }));
}

function buildModuleGraph(entryFile: string): ModuleGraph {
  const modules = new Map<string, LoadedSigilModule>();
  const topoOrder: string[] = [];
  const visiting = new Set<string>();
  const visitStack: string[] = [];

  const visit = (filePath: string, logicalId?: string, inheritedProject?: SigilProjectConfig): void => {
    const absFile = resolve(filePath);
    // Determine project early to compute logical ID
    const project = getSigilProjectConfig(absFile) ?? inheritedProject;
    // Compute logical ID if not provided
    const computedLogicalId = logicalId ?? filePathToModuleId(absFile, project ?? undefined);
    const moduleKey = computedLogicalId ?? absFile;
    if (modules.has(moduleKey)) return;
    if (visiting.has(moduleKey)) {
      const startIdx = visitStack.indexOf(moduleKey);
      const cycle = (startIdx >= 0 ? visitStack.slice(startIdx) : [moduleKey]).concat(moduleKey);
      throw new SigilDiagnosticError(diagnostic('SIGIL-CLI-IMPORT-CYCLE', 'cli', 'import cycle detected', {
        details: { cycle }
      }));
    }
    visiting.add(moduleKey);
    visitStack.push(moduleKey);

    const source = readFileSync(absFile, 'utf-8');
    validateSurfaceForm(source, absFile);
    const tokens = tokenize(source);
    const ast = parse(tokens, absFile);
    ensureNoTestsOutsideTestsDir(ast, absFile);
    validateCanonicalForm(ast, absFile);
    const mod: LoadedSigilModule = { id: moduleKey, filePath: absFile, source, ast, project: project ?? undefined };

    for (const decl of ast.declarations) {
      if (decl.type !== 'ImportDecl') continue;
      const importedId = decl.modulePath.join('/');
      if (!isSigilImportPath(importedId)) continue;
      const resolved = resolveSigilImportToFile(absFile, project ?? undefined, importedId);
      if (!existsSync(resolved.filePath)) {
        throw new SigilDiagnosticError(diagnostic('SIGIL-CLI-IMPORT-NOT-FOUND', 'cli', 'sigil import not found', {
          details: { importedId, importerFile: absFile, expectedFile: resolved.filePath }
        }));
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

function buildImportedTypeRegistriesForModule(
  module: LoadedSigilModule,
  exportedTypeRegistries: Map<string, Map<string, TypeInfo>>
): Map<string, Map<string, TypeInfo>> {
  const imported = new Map<string, Map<string, TypeInfo>>();

  for (const decl of module.ast.declarations) {
    if (decl.type !== 'ImportDecl') continue;
    const moduleId = decl.modulePath.join('/');

    // Only track Sigil imports (not externs)
    if (!isSigilImportPath(moduleId)) continue;

    const typeRegistry = exportedTypeRegistries.get(moduleId);
    if (typeRegistry && typeRegistry.size > 0) {
      imported.set(moduleId, typeRegistry);
    }
  }

  return imported;
}

function typeCheckModuleGraph(graph: ModuleGraph): Map<string, Map<string, InferenceType>> {
  const moduleTypes = new Map<string, Map<string, InferenceType>>();
  const exportedNamespaces = new Map<string, InferenceType>();
  const exportedTypeRegistries = new Map<string, Map<string, TypeInfo>>();

  for (const moduleId of graph.topoOrder) {
    const mod = graph.modules.get(moduleId)!

    // Build imported type registries for this module
    const importedTypeRegistries = buildImportedTypeRegistriesForModule(
      mod,
      exportedTypeRegistries
    );

    const importedNamespaces = buildImportedNamespacesForModule(mod, exportedNamespaces);
    const types = typeCheck(mod.ast, mod.source, {
      importedNamespaces,
      importedTypeRegistries,
      sourceFile: mod.filePath
    });
    moduleTypes.set(moduleId, types);

    // Build exported namespace (values)
    const fields = new Map<string, InferenceType>();
    for (const decl of mod.ast.declarations) {
      if (!declIsExported(decl)) continue;
      if (decl.type === 'FunctionDecl' || decl.type === 'ConstDecl') {
        const t = types.get(decl.name);
        if (t) fields.set(decl.name, t);
      }
    }
    exportedNamespaces.set(moduleId, { kind: 'record', fields });

    // Build exported type registry with qualified field types
    const typeRegistry = new Map<string, TypeInfo>();

    // First pass: build local type registry for qualification lookup
    const localTypeRegistry = new Map<string, TypeInfo>();
    for (const decl of mod.ast.declarations) {
      if (decl.type === 'TypeDecl') {
        localTypeRegistry.set(decl.name, {
          typeParams: decl.typeParams,
          definition: decl.definition  // Raw AST, just for lookup
        });
      }
    }

    // Second pass: export with qualified field types
    for (const decl of mod.ast.declarations) {
      if (decl.type === 'TypeDecl' && decl.isExported) {
        // Qualify all unqualified type references in the definition
        const qualifiedDef = qualifyTypeDef(
          decl.definition,
          moduleId,
          localTypeRegistry,
          decl.typeParams  // Don't qualify type parameters
        );

        typeRegistry.set(decl.name, {
          typeParams: decl.typeParams,
          definition: qualifiedDef
        });
      }
    }

    exportedTypeRegistries.set(moduleId, typeRegistry);
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
  const rootProject = getSigilProjectConfig(entryFile) ?? undefined;
  const absEntryFile = resolve(entryFile);
  const entryLogicalId = filePathToModuleId(absEntryFile, rootProject);
  const rootModuleKey = entryLogicalId ?? absEntryFile;
  const rootModule = graph.modules.get(rootModuleKey) ?? (() => { throw new Error('Root module missing: ' + rootModuleKey); })();
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
  const human = hasFlag(args, '--human');
  const cleaned = stripFlag(args, '--human');
  try {
    rejectRemovedJsonFlag(args);
    if (cleaned.length === 0) {
      emitEnvelope({ formatVersion: 1, command: 'sigilc lex', ok: false, phase: 'cli', error: diagnostic('SIGIL-CLI-USAGE', 'cli', 'missing file argument') }, human);
    }

    const filename = cleaned[0];
    const source = readFileSync(filename, 'utf-8');

    // Validate surface form (formatting) before tokenizing
    validateSurfaceForm(source, filename);

    const tokens = tokenize(source);
    emitEnvelope({
      formatVersion: 1,
      command: 'sigilc lex',
      ok: true,
      phase: 'lexer',
      data: {
        file: filename,
        summary: { tokens: tokens.length },
        tokens: tokens.map(t => ({
          type: t.type,
          lexeme: t.value,
          start: t.start,
          end: t.end,
          text: tokenToString(t)
        }))
      }
    }, human);
  } catch (error) {
    const filename = cleaned[0];
    emitEnvelope({ formatVersion: 1, command: 'sigilc lex', ok: false, phase: 'lexer', error: unknownToDiagnostic(error, 'lexer', filename) }, human);
  }
}

function parseCommand(args: string[]) {
  const human = hasFlag(args, '--human');
  const cleaned = stripFlag(args, '--human');
  try {
    rejectRemovedJsonFlag(args);
    if (cleaned.length === 0) {
      emitEnvelope({ formatVersion: 1, command: 'sigilc parse', ok: false, phase: 'cli', error: diagnostic('SIGIL-CLI-USAGE', 'cli', 'missing file argument') }, human);
    }

    const filename = cleaned[0];
    const source = readFileSync(filename, 'utf-8');

    // Validate surface form (formatting) before tokenizing
    validateSurfaceForm(source, filename);

    const tokens = tokenize(source);

    const ast = parse(tokens, filename);
    ensureNoTestsOutsideTestsDir(ast, filename);
    emitEnvelope({
      formatVersion: 1,
      command: 'sigilc parse',
      ok: true,
      phase: 'parser',
      data: {
        file: filename,
        summary: { tokens: tokens.length, declarations: ast.declarations.length },
        ast
      }
    }, human);
  } catch (error) {
    const filename = cleaned[0];
    emitEnvelope({ formatVersion: 1, command: 'sigilc parse', ok: false, phase: 'parser', error: unknownToDiagnostic(error, 'parser', filename) }, human);
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
  const human = hasFlag(args, '--human');
  const cleanedArgs = stripFlag(args, '--human');
  let outputFile = '';
  const filename = cleanedArgs[0];

  try {
    rejectRemovedJsonFlag(args);
    if (cleanedArgs.length === 0) {
      emitEnvelope({ formatVersion: 1, command: 'sigilc compile', ok: false, phase: 'cli', error: diagnostic('SIGIL-CLI-USAGE', 'cli', 'missing file argument') }, human);
    }

    // Check for -o flag first
    const outputIndex = cleanedArgs.indexOf('-o');
    if (outputIndex !== -1 && cleanedArgs[outputIndex + 1]) {
      outputFile = cleanedArgs[outputIndex + 1];
    } else {
      outputFile = getSmartOutputPath(filename);
    }

    const { graph, moduleTypes, outputs } = await compileModuleGraph(filename, outputFile);
    const absFilename = resolve(filename);
    const rootProject = getSigilProjectConfig(filename) ?? undefined;
    const filenameLogicalId = filePathToModuleId(absFilename, rootProject);
    const rootKey = filenameLogicalId ?? absFilename;
    const rootModule = graph.modules.get(rootKey) ?? (() => { throw new Error(`Root module not loaded: ${filename} (key: ${rootKey})`); })();
    const ast = rootModule.ast;
    const source = rootModule.source;
    const types = moduleTypes.get(rootKey) ?? new Map<string, InferenceType>();
    outputFile = outputs.get(rootKey) ?? outputFile;

    // Type check results for root already available
    const showTypes = cleanedArgs.includes('--show-types');

    // Generate semantic map
    const mapFile = filename.replace('.sigil', '.sigil.map');
    generateSemanticMap(ast, types, source, mapFile);

    // Enhance with Claude Code CLI
    enhanceWithClaude(filename, mapFile);
    emitEnvelope({
      formatVersion: 1,
      command: 'sigilc compile',
      ok: true,
      phase: 'codegen',
      data: {
        input: filename,
        outputs: {
          rootTs: outputFile,
          allModules: graph.topoOrder.map((moduleId) => {
            const mod = graph.modules.get(moduleId)!;
            return {
              moduleId,
              sourceFile: mod.filePath,
              outputFile: outputs.get(moduleId) ?? '',
            };
          })
        },
        project: rootModule.project ? { root: rootModule.project.root, layout: rootModule.project.layout } : undefined,
        typecheck: {
          ok: true,
          inferred: showTypes ? Array.from(types.entries()).map(([name, type]) => ({ name, type: formatType(type) })) : []
        },
        semanticMap: {
          path: mapFile,
          generated: true,
          aiEnhanced: process.env.SIGIL_ENABLE_MAP_ENHANCE === '1'
        }
      }
    }, human);
  } catch (error) {
    emitEnvelope({ formatVersion: 1, command: 'sigilc compile', ok: false, phase: 'codegen', error: unknownToDiagnostic(error, 'codegen', filename) }, human);
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
  const human = hasFlag(args, '--human');
  const cleaned = stripFlag(args, '--human');
  const filename = cleaned[0];
  const outputFile = filename ? getSmartOutputPath(filename) : '';
  const runnerFile = outputFile ? outputFile.replace(/\.ts$/, '.run.ts') : '';

  try {
    rejectRemovedJsonFlag(args);
    if (cleaned.length === 0) {
      emitEnvelope({ formatVersion: 1, command: 'sigilc run', ok: false, phase: 'cli', error: diagnostic('SIGIL-CLI-USAGE', 'cli', 'missing file argument') }, human);
    }
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

// Call main and handle the result (all Sigil functions are async)
const result = await main();

// If main returns a value (not Unit/undefined), show it
if (result !== undefined) {
  console.log(result);
}
`;

    writeFileSync(runnerFile, runnerCode, 'utf-8');

    const started = Date.now();
    const runtime = await new Promise<{ stdout: string; stderr: string; exitCode: number }>((resolveRun, rejectRun) => {
      let stdout = '';
      let stderr = '';
      const nodeProcess = spawn('pnpm', ['exec', 'node', '--import', 'tsx', runnerFile], {
        stdio: ['ignore', 'pipe', 'pipe'],
        shell: false,
      });
      nodeProcess.stdout.on('data', d => { stdout += d.toString(); });
      nodeProcess.stderr.on('data', d => { stderr += d.toString(); });
      nodeProcess.on('exit', (code) => resolveRun({ stdout, stderr, exitCode: code ?? 0 }));
      nodeProcess.on('error', (error) => rejectRun(error));
    });

    if (runtime.exitCode !== 0) {
      emitEnvelope({
        formatVersion: 1,
        command: 'sigilc run',
        ok: false,
        phase: 'runtime',
        error: diagnostic('SIGIL-RUNTIME-CHILD-EXIT', 'runtime', 'child process exited with nonzero status', {
          details: { exitCode: runtime.exitCode, stdout: runtime.stdout, stderr: runtime.stderr }
        })
      }, human);
    }

    emitEnvelope({
      formatVersion: 1,
      command: 'sigilc run',
      ok: true,
      phase: 'runtime',
      data: {
        compile: {
          input: filename,
          output: actualOutput,
          runnerFile,
        },
        runtime: {
          engine: 'node+tsx',
          exitCode: runtime.exitCode,
          durationMs: Date.now() - started,
          stdout: runtime.stdout,
          stderr: runtime.stderr,
        }
      }
    }, human);
  } catch (error) {
    let d = unknownToDiagnostic(error, 'runtime', filename);
    if (error instanceof Error && (error as NodeJS.ErrnoException).code === 'ENOENT') {
      d = diagnostic('SIGIL-RUN-ENGINE-NOT-FOUND', 'runtime', 'runtime engine not available', {
        details: { required: ['pnpm', 'node', 'tsx'] }
      });
    }
    emitEnvelope({ formatVersion: 1, command: 'sigilc run', ok: false, phase: 'runtime', error: d }, human);
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
    rejectRemovedJsonFlag(args);
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
    const diag = unknownToDiagnostic(error, 'cli');
    if (jsonMode) {
      process.stdout.write(JSON.stringify({
        formatVersion: 1,
        command: 'sigilc test',
        ok: false,
        summary: { files: 0, discovered: 0, selected: 0, passed: 0, failed: 0, errored: 1, skipped: 0, durationMs: 0 },
        results: [],
        error: diag
      }) + '\n');
    } else {
      console.error(renderHumanEnvelope({ formatVersion: 1, command: 'sigilc test', ok: false, phase: diag.phase, error: diag }));
    }
    process.exit(2);
  }
}

main();
