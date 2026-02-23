#!/usr/bin/env node

/**
 * Mint Compiler CLI
 */

import { readFileSync, writeFileSync, mkdirSync } from 'fs';
import { dirname, basename } from 'path';
import { spawn } from 'child_process';
import { tokenize } from './lexer/lexer.js';
import { tokenToString } from './lexer/token.js';
import { parse } from './parser/parser.js';
import { compile } from './codegen/javascript.js';
import { validateCanonicalForm } from './validator/canonical.js';
import { validateExterns } from './validator/extern-validator.js';
import { typeCheck } from './typechecker/index.js';
import { formatType } from './typechecker/errors.js';
import { generateSemanticMap, enhanceWithClaude } from './mapgen/index.js';

async function main() {
  const args = process.argv.slice(2);

  if (args.length === 0) {
    console.error('Usage: mintc <command> [options]');
    console.error('');
    console.error('Commands:');
    console.error('  lex <file>        Tokenize a Mint file');
    console.error('  parse <file>      Parse a Mint file and show AST');
    console.error('  compile <file>    Compile a Mint file to JavaScript');
    console.error('  run <file>        Compile and run a Mint file');
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
    case 'help':
      console.log('Mint Compiler v0.1.0');
      console.log('');
      console.log('Commands:');
      console.log('  lex <file>        Tokenize a Mint file and print tokens');
      console.log('  parse <file>      Parse a Mint file and show AST');
      console.log('  compile <file>    Compile a Mint file to JavaScript');
      console.log('  run <file>        Compile and run a Mint file');
      console.log('  help              Show this help message');
      console.log('');
      console.log('Output locations:');
      console.log('  examples/*.mint   → examples/*.js (beside source)');
      console.log('  src/*.mint        → .local/src/*.js');
      console.log('  *.mint            → .local/*.js');
      console.log('');
      console.log('Options:');
      console.log('  -o <file>         Specify custom output location');
      console.log('  --show-types      Display inferred types after type checking');
      break;
    default:
      console.error(`Unknown command: ${command}`);
      process.exit(1);
  }
}

function lexCommand(args: string[]) {
  if (args.length === 0) {
    console.error('Usage: mintc lex <file>');
    process.exit(1);
  }

  const filename = args[0];

  try {
    const source = readFileSync(filename, 'utf-8');
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
    console.error('Usage: mintc parse <file>');
    process.exit(1);
  }

  const filename = args[0];

  try {
    const source = readFileSync(filename, 'utf-8');
    const tokens = tokenize(source);

    console.log(`Parsing ${filename}...`);
    console.log(`Total tokens: ${tokens.length}`);

    const ast = parse(tokens);

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
  // examples/*.mint → examples/*.js (beside source, for documentation)
  if (inputFile.startsWith('examples/')) {
    return inputFile.replace(/\.mint$/, '.js');
  }

  // Everything else → .local/ (keeps root clean)
  // src/**/*.mint → .local/src/**/*.js
  // *.mint → .local/*.js
  return `.local/${inputFile.replace(/\.mint$/, '.js')}`;
}

async function compileCommand(args: string[]) {
  if (args.length === 0) {
    console.error('Usage: mintc compile <file> [-o output.js]');
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
    const source = readFileSync(filename, 'utf-8');
    const tokens = tokenize(source);
    const ast = parse(tokens);

    // Validate canonical form (enforces ONE way)
    validateCanonicalForm(ast);

    // Type check (ALWAYS - no exceptions)
    const showTypes = args.includes('--show-types');
    const types = typeCheck(ast, source);

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

    const jsCode = compile(ast, filename);

    // Validate externals BEFORE writing file (link-time validation)
    await validateExterns(ast);

    // Ensure output directory exists
    const outputDir = dirname(outputFile);
    if (outputDir !== '.') {
      mkdirSync(outputDir, { recursive: true });
    }

    writeFileSync(outputFile, jsCode, 'utf-8');

    console.log(`✓ Compiled ${filename} → ${outputFile}`);

    // Generate semantic map
    const mapFile = filename.replace('.mint', '.mint.map');
    generateSemanticMap(ast, types, source, mapFile);
    console.log(`✓ Generated basic semantic map → ${mapFile}`);

    // Enhance with Claude Code CLI
    enhanceWithClaude(filename, mapFile);
    console.log(`✓ Enhanced semantic map with AI documentation`);
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

async function runCommand(args: string[]) {
  if (args.length === 0) {
    console.error('Usage: mintc run <file>');
    process.exit(1);
  }

  const filename = args[0];
  const baseName = basename(filename, '.mint');
  const outputFile = `.local/${baseName}.js`;
  const runnerFile = `.local/${baseName}.run.js`;

  try {
    // Compile to .local/
    const source = readFileSync(filename, 'utf-8');
    const tokens = tokenize(source);
    const ast = parse(tokens);

    // Validate canonical form (enforces ONE way)
    validateCanonicalForm(ast);

    // Type check (should always happen!)
    typeCheck(ast, source);

    const jsCode = compile(ast, filename);

    // Validate externals BEFORE writing file (link-time validation)
    await validateExterns(ast);

    // Ensure .local exists
    mkdirSync('.local', { recursive: true });
    writeFileSync(outputFile, jsCode, 'utf-8');

    // Create runner that calls main()
    const runnerCode = `import { main } from './${baseName}.js';

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

    console.log(`✓ Compiled ${filename} → ${outputFile}`);
    console.log('');

    // Run the wrapper with Node.js
    const nodeProcess = spawn('node', [runnerFile], {
      stdio: 'inherit',
      shell: false,
    });

    nodeProcess.on('exit', (code) => {
      process.exit(code || 0);
    });

    nodeProcess.on('error', (error) => {
      console.error(`Failed to run: ${error.message}`);
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

main();
