import React, { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import * as sigilRaw from './generated/game-2048';
import { TracePanel } from './tracePanel';
import { CALL_GRAPH } from './sigilSource';

type Cell = { value: number; x: number; y: number };
type Direction = 'up' | 'down' | 'left' | 'right';
type Game = { board: Cell[]; score: number; size: number; status: string };
type Spawn = { value: number; x: number; y: number };

type SigilDomain = {
  applyMove: (direction: string, game: Game) => Promise<Game>;
  newGame: (initialSpawns: Spawn[]) => Promise<Game>;
  spawnTile: (game: Game, spawn: Spawn) => Promise<Game>;
};

const sigil = sigilRaw as unknown as SigilDomain;

function boardRows(game: Game): Cell[][] {
  const rows: Cell[][] = [];
  for (let y = 0; y < game.size; y += 1) {
    rows.push(game.board.filter((cell) => cell.y === y).sort((a, b) => a.x - b.x));
  }
  return rows;
}

function cellTone(value: number): string {
  return value === 0 ? 'empty' : `v-${value}`;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function sameGameState(left: Game, right: Game): boolean {
  return left.score === right.score
    && left.status === right.status
    && left.board.length === right.board.length
    && left.board.every((cell, index) => {
      const other = right.board[index];
      return cell.x === other.x && cell.y === other.y && cell.value === other.value;
    });
}

function randomSpawn(game: Game): Spawn | null {
  const empty = game.board.filter((cell) => cell.value === 0);
  if (empty.length === 0) return null;
  const slot = empty[Math.floor(Math.random() * empty.length)];
  return {
    value: Math.random() < 0.9 ? 2 : 4,
    x: slot.x,
    y: slot.y,
  };
}

async function spawnRandomTile(game: Game): Promise<Game> {
  const next = randomSpawn(game);
  if (!next) return game;
  return sigil.spawnTile(game, next);
}

function statusText(game: Game): string {
  switch (game.status) {
    case 'won':
      return '2048 reached. Restart to chase a higher score.';
    case 'lost':
      return 'No legal moves left. Restart for a new board.';
    default:
      return 'Use arrows or WASD to slide the board.';
  }
}

// How long each function stays highlighted before moving to the next.
const STEP_MS = 1000;
// Brief hold on the last function before clearing.
const TAIL_MS = 350;

function TwentyFortyEightApp(): JSX.Element {
  const [game, setGame] = useState<Game | null>(null);
  const [appError, setAppError] = useState<string | null>(null);
  const gameRef = useRef<Game | null>(null);

  // Trace state — single active function name at a time.
  const [activeFn, setActiveFn] = useState<string | null>(null);
  const sequenceTimers = useRef<ReturnType<typeof setTimeout>[]>([]);

  useEffect(() => {
    gameRef.current = game;
  }, [game]);

  function cancelSequence(): void {
    for (const t of sequenceTimers.current) clearTimeout(t);
    sequenceTimers.current = [];
  }

  // Schedule a sequence starting at offsetMs from now. Does NOT cancel existing timers,
  // so callers can chain sequences back-to-back without losing the first one.
  function scheduleSequence(graph: string, offsetMs: number): void {
    const fns = CALL_GRAPH[graph] ?? [];
    fns.forEach((fn, i) => {
      sequenceTimers.current.push(
        setTimeout(() => setActiveFn(fn), offsetMs + i * STEP_MS)
      );
    });
    sequenceTimers.current.push(
      setTimeout(() => setActiveFn(null), offsetMs + fns.length * STEP_MS + TAIL_MS)
    );
  }

  function sequenceDuration(graph: string): number {
    return (CALL_GRAPH[graph]?.length ?? 0) * STEP_MS;
  }

  async function freshGame(): Promise<Game> {
    cancelSequence();
    scheduleSequence('new', 0);
    let next = await sigil.newGame([]);
    next = await spawnRandomTile(next);
    return spawnRandomTile(next);
  }

  useEffect(() => {
    void freshGame()
      .then((nextGame) => {
        setGame(nextGame);
        setAppError(null);
      })
      .catch((error) => setAppError(errorMessage(error)));
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  async function restart(): Promise<void> {
    try {
      const nextGame = await freshGame();
      setGame(nextGame);
      setAppError(null);
    } catch (error) {
      setAppError(errorMessage(error));
    }
  }

  async function applyMove(direction: Direction): Promise<void> {
    const current = gameRef.current;
    if (!current || current.status !== 'playing') return;
    try {
      cancelSequence();
      scheduleSequence(direction, 0);
      const moved = await sigil.applyMove(direction, current);
      if (moved.status === 'playing' && !sameGameState(current, moved)) {
        // Chain the spawn sequence after the move sequence — no cancel in between.
        scheduleSequence('spawn', sequenceDuration(direction));
        const nextGame = await spawnRandomTile(moved);
        setGame(nextGame);
      } else {
        setGame(moved);
      }
      setAppError(null);
    } catch (error) {
      setAppError(errorMessage(error));
    }
  }

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent): void {
      switch (event.key) {
        case 'ArrowUp':
        case 'w':
        case 'W':
          event.preventDefault();
          void applyMove('up');
          break;
        case 'ArrowDown':
        case 's':
        case 'S':
          event.preventDefault();
          void applyMove('down');
          break;
        case 'ArrowLeft':
        case 'a':
        case 'A':
          event.preventDefault();
          void applyMove('left');
          break;
        case 'ArrowRight':
        case 'd':
        case 'D':
          event.preventDefault();
          void applyMove('right');
          break;
        default:
          break;
      }
    }

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  if (!game) {
    return (
      <div className="twenty48-shell">
        <p className="twenty48-banner">Loading Sigil 2048...</p>
      </div>
    );
  }

  return (
    <div className="twenty48-shell">
      <header className="twenty48-header">
        <div>
          <p className="eyebrow">Projects / Sigil 2048</p>
          <h1>Sigil 2048</h1>
          <p className="subtitle">Sigil owns the board rules, merges, score, and game-over state. The browser bridge only handles input and random tile spawning.</p>
        </div>
        <button className="restart" onClick={() => void restart()}>Restart</button>
      </header>

      <div className="twenty48-body">
        <div className="twenty48-main">
          <section className="twenty48-toolbar">
            <div className="score-stack">
              <div className="score-card">
                <span>Score</span>
                <strong>{game.score}</strong>
              </div>
              <div className="score-card muted">
                <span>Status</span>
                <strong>{game.status}</strong>
              </div>
            </div>
            <p className="status-copy">{statusText(game)}</p>
          </section>
          {appError ? <p className="twenty48-banner error">App error: {appError}</p> : null}
          <section className="twenty48-board" aria-label="2048 board" style={{ gridTemplateColumns: `repeat(${game.size}, minmax(0, 1fr))` }}>
            {boardRows(game).flat().map((cell) => (
              <div className="tile" data-tone={cellTone(cell.value)} key={`${cell.x}-${cell.y}`}>
                {cell.value === 0 ? '' : cell.value}
              </div>
            ))}
          </section>
          <section className="twenty48-controls" aria-label="Move controls">
            <button type="button" onClick={() => void applyMove('up')}>Up</button>
            <div className="row-controls">
              <button type="button" onClick={() => void applyMove('left')}>Left</button>
              <button type="button" onClick={() => void applyMove('down')}>Down</button>
              <button type="button" onClick={() => void applyMove('right')}>Right</button>
            </div>
          </section>
          <p className="twenty48-banner muted">
            Arrow keys and WASD work too. A new tile only spawns after a move that actually changes the board.
          </p>
        </div>

        <TracePanel activeFn={activeFn} />
      </div>
    </div>
  );
}

export function mount2048App(element: HTMLElement): void {
  createRoot(element).render(<TwentyFortyEightApp />);
}
