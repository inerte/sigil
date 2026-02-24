#!/usr/bin/env node
/**
 * Benchmark Comparison Tool
 *
 * Compares algorithm implementations across languages.
 *
 * Usage:
 *   node benchmarks/tools/compare.ts benchmarks/algorithms/factorial
 */
import * as fs from 'fs';
import * as path from 'path';
import { compareImplementations, generateComparisonTable, calculateEfficiency } from './count-tokens.js';
function findImplementations(algorithmDir) {
    const files = [];
    if (!fs.existsSync(algorithmDir)) {
        throw new Error(`Directory not found: ${algorithmDir}`);
    }
    const entries = fs.readdirSync(algorithmDir);
    for (const entry of entries) {
        const fullPath = path.join(algorithmDir, entry);
        const ext = path.extname(entry);
        if (['.sigil', '.ts', '.py', '.rs', '.hs'].includes(ext)) {
            files.push(fullPath);
        }
    }
    return files;
}
function main() {
    const args = process.argv.slice(2);
    if (args.length === 0) {
        console.error('Usage: compare.ts <algorithm-directory>');
        console.error('Example: compare.ts benchmarks/algorithms/factorial');
        process.exit(1);
    }
    const algorithmDir = args[0];
    const algorithmName = path.basename(algorithmDir);
    console.log(`\n# ${algorithmName} - Token Comparison\n`);
    try {
        const files = findImplementations(algorithmDir);
        if (files.length === 0) {
            console.error('No implementation files found');
            process.exit(1);
        }
        console.log(`Found ${files.length} implementation(s):\n`);
        files.forEach(f => console.log(`  - ${path.basename(f)}`));
        console.log();
        const results = compareImplementations(files);
        const table = generateComparisonTable(results);
        const efficiency = calculateEfficiency(results);
        console.log('## Metrics\n');
        console.log(table);
        console.log('\n## Efficiency (vs TypeScript baseline)\n');
        console.log('| Language | Efficiency | Interpretation |');
        console.log('|----------|------------|----------------|');
        for (const [lang, eff] of efficiency) {
            const pct = ((eff - 1) * 100).toFixed(1);
            const interpretation = eff > 1
                ? `${pct}% more compact`
                : eff < 1
                    ? `${Math.abs(parseFloat(pct))}% more verbose`
                    : 'baseline';
            console.log(`| ${lang} | ${eff.toFixed(3)} | ${interpretation} |`);
        }
        console.log();
    }
    catch (error) {
        console.error('Error:', error);
        process.exit(1);
    }
}
main();
