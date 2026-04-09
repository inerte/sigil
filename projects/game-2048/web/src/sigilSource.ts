// Span data extracted from game-2048.span.json — function name → Sigil source line range (1-indexed, inclusive).
export type FnSpan = { label: string; start: number; end: number };

export const FN_SPANS: readonly FnSpan[] = [
  { label: 'applyMove',           start: 3,   end: 17  },
  { label: 'buildBoard',          start: 19,  end: 22  },
  { label: 'buildRow',            start: 24,  end: 27  },
  { label: 'cellAt',              start: 29,  end: 35  },
  { label: 'columnValues',        start: 37,  end: 40  },
  { label: 'compressValues',      start: 42,  end: 42  },
  { label: 'containsWinningTile', start: 44,  end: 44  },
  { label: 'emptyGame',           start: 46,  end: 48  },
  { label: 'hasEmptyCell',        start: 50,  end: 50  },
  { label: 'hasLegalMove',        start: 52,  end: 52  },
  { label: 'hasMergeNeighbor',    start: 54,  end: 63  },
  { label: 'horizontalMatch',     start: 65,  end: 68  },
  { label: 'mergeCompressed',     start: 70,  end: 83  },
  { label: 'moveColumnsDown',     start: 85,  end: 91  },
  { label: 'moveColumnsUp',       start: 93,  end: 99  },
  { label: 'moveRowsLeft',        start: 101, end: 107 },
  { label: 'moveRowsRight',       start: 109, end: 115 },
  { label: 'newGame',             start: 117, end: 117 },
  { label: 'normalizeLine',       start: 119, end: 122 },
  { label: 'reverseInts',         start: 124, end: 124 },
  { label: 'rowValues',           start: 126, end: 129 },
  { label: 'setValue',            start: 131, end: 134 },
  { label: 'spawnTile',           start: 136, end: 151 },
  { label: 'spawnTiles',          start: 153, end: 156 },
  { label: 'statusForBoard',      start: 158, end: 164 },
  { label: 'valueAt',             start: 166, end: 169 },
  { label: 'verticalMatch',       start: 171, end: 174 },
  { label: 'writeColumn',         start: 176, end: 179 },
  { label: 'writeRow',            start: 181, end: 184 },
  { label: 'zeroes',              start: 186, end: 189 },
];

// Which Sigil functions are reachable for each game action.
// Order is roughly depth-first call order; used only for the trace log label.
export const CALL_GRAPH: Record<string, readonly string[]> = {
  left: [
    'applyMove', 'moveRowsLeft', 'rowValues', 'valueAt', 'cellAt',
    'normalizeLine', 'compressValues', 'mergeCompressed', 'zeroes',
    'writeRow', 'setValue',
    'statusForBoard', 'containsWinningTile',
    'hasLegalMove', 'hasEmptyCell', 'hasMergeNeighbor', 'horizontalMatch', 'verticalMatch',
  ],
  right: [
    'applyMove', 'moveRowsRight', 'reverseInts', 'rowValues', 'valueAt', 'cellAt',
    'normalizeLine', 'compressValues', 'mergeCompressed', 'zeroes',
    'writeRow', 'setValue',
    'statusForBoard', 'containsWinningTile',
    'hasLegalMove', 'hasEmptyCell', 'hasMergeNeighbor', 'horizontalMatch', 'verticalMatch',
  ],
  up: [
    'applyMove', 'moveColumnsUp', 'columnValues', 'valueAt', 'cellAt',
    'normalizeLine', 'compressValues', 'mergeCompressed', 'zeroes',
    'writeColumn', 'setValue',
    'statusForBoard', 'containsWinningTile',
    'hasLegalMove', 'hasEmptyCell', 'hasMergeNeighbor', 'horizontalMatch', 'verticalMatch',
  ],
  down: [
    'applyMove', 'moveColumnsDown', 'reverseInts', 'columnValues', 'valueAt', 'cellAt',
    'normalizeLine', 'compressValues', 'mergeCompressed', 'zeroes',
    'writeColumn', 'setValue',
    'statusForBoard', 'containsWinningTile',
    'hasLegalMove', 'hasEmptyCell', 'hasMergeNeighbor', 'horizontalMatch', 'verticalMatch',
  ],
  // bridge calls sigil.spawnTile directly — spawnTiles never runs in the browser flow.
  spawn: [
    'spawnTile', 'cellAt', 'setValue', 'valueAt',
    'statusForBoard', 'containsWinningTile',
    'hasLegalMove', 'hasEmptyCell', 'hasMergeNeighbor', 'horizontalMatch', 'verticalMatch',
  ],
  // bridge calls sigil.newGame([]) — spawnTiles runs but immediately returns (empty list),
  // and the two subsequent spawns are bridge-driven and show up as separate spawn sequences.
  new: [
    'newGame', 'emptyGame', 'buildBoard', 'buildRow', 'spawnTiles',
  ],
};
