import React, { useEffect, useRef, useState } from 'react';
import sigilSource from '../../src/game2048.lib.sigil?raw';
import { FN_SPANS } from './sigilSource';

type Props = {
  activeFn: string | null;
};

const SOURCE_LINES = sigilSource.split('\n');

// Pre-build a map: line number (1-indexed) → which function owns it.
// Each line belongs to at most one function (the narrowest enclosing span wins, but
// in practice these are all non-overlapping top-level declarations).
const LINE_TO_FN = new Map<number, string>();
for (const span of FN_SPANS) {
  for (let ln = span.start; ln <= span.end; ln++) {
    LINE_TO_FN.set(ln, span.label);
  }
}

// A minimal, regex-based syntax colourer for Sigil source.
// Returns an array of { text, cls } tokens for one line.
type Token = { text: string; cls: string };

function tokenizeLine(line: string): Token[] {
  if (line.trim() === '') return [{ text: '\u00a0', cls: '' }];

  const tokens: Token[] = [];
  let rest = line;

  // Patterns in priority order
  const patterns: Array<[RegExp, string]> = [
    [/^⟦[^⟧]*⟧/,                         'st-comment'],   // ⟦ comment ⟧
    [/^"(?:[^"\\]|\\.)*"/,                'st-string'],    // "string"
    [/^(λ|µ|§|•|¶|†|※)/,                 'st-sigil'],     // rooted sigils
    [/^(ensures|match|requires|where|from|and|or|not)\b/, 'st-kw'],
    [/^(true|false)\b/,                   'st-bool'],
    [/^[A-Z][A-Za-z0-9]*/,               'st-type'],      // Types / constructors
    [/^[a-z][A-Za-z0-9]*/,               'st-ident'],     // identifiers
    [/^\d+(?:\.\d+)?/,                   'st-num'],       // numbers
    [/^[⧺≥≤≠⇒→←⊕↦]/,                    'st-op'],        // unicode operators
    [/^[+\-*/%=<>!&|^~#@:,.()[\]{}]/,   'st-punct'],     // ascii punctuation
    [/^./,                               ''],              // fallback: one char, no class
  ];

  while (rest.length > 0) {
    let matched = false;
    for (const [re, cls] of patterns) {
      const m = rest.match(re);
      if (m) {
        tokens.push({ text: m[0], cls });
        rest = rest.slice(m[0].length);
        matched = true;
        break;
      }
    }
    if (!matched) {
      tokens.push({ text: rest[0], cls: '' });
      rest = rest.slice(1);
    }
  }

  return tokens;
}

// Memoize tokenized lines — they never change.
const TOKENIZED = SOURCE_LINES.map(tokenizeLine);

export function TracePanel({ activeFn }: Props): JSX.Element {
  const codeRef = useRef<HTMLDivElement>(null);
  const [follow, setFollow] = useState(false);

  // Only highlight and scroll when follow is on.
  const activeLines = new Set<number>();
  if (follow && activeFn) {
    const span = FN_SPANS.find(s => s.label === activeFn);
    if (span) {
      for (let i = span.start; i <= span.end; i++) activeLines.add(i);
    }
  }

  useEffect(() => {
    if (!follow || !activeFn || !codeRef.current) return;
    const span = FN_SPANS.find(s => s.label === activeFn);
    if (!span) return;
    const el = codeRef.current.querySelector<HTMLElement>(`[data-line="${span.start}"]`);
    el?.scrollIntoView({ block: 'center', behavior: 'smooth' });
  }, [activeFn, follow]);

  return (
    <aside className="trace-panel">
      <div className="trace-header">
        <span className="trace-title">Sigil source</span>
        <span className={`trace-pill ${follow && activeFn ? 'trace-pill-active' : ''}`}>
          {follow && activeFn ? activeFn : `${SOURCE_LINES.length} lines`}
        </span>
        <button
          className={`trace-follow-btn ${follow ? 'trace-follow-btn-on' : ''}`}
          onClick={() => setFollow((f) => !f)}
          title="Toggle live highlighting and auto-scroll"
        >
          {follow ? 'following' : 'follow'}
        </button>
      </div>

      <div className="code-view" ref={codeRef}>
        {SOURCE_LINES.map((_, idx) => {
          const lineNum = idx + 1;
          const isActive = activeLines.has(lineNum);
          const tokens = TOKENIZED[idx];
          return (
            <div
              key={lineNum}
              data-line={lineNum}
              className={`code-line${isActive ? ' code-line-active' : ''}`}
            >
              <span className="code-lnum">{lineNum}</span>
              <span className="code-text" aria-hidden="false">
                {tokens.map((tok, ti) =>
                  tok.cls
                    ? <span key={ti} className={tok.cls}>{tok.text}</span>
                    : tok.text
                )}
              </span>
            </div>
          );
        })}
      </div>
    </aside>
  );
}
