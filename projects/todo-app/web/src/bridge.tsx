import React, { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import * as sigilRaw from './generated/todo-domain';

type Todo = { id: number; text: string; done: boolean };
type Filter = 'all' | 'active' | 'completed';

type SigilDomain = {
  canAdd: (text: string) => Promise<boolean>;
  addTodo: (id: number, text: string, todos: Todo[]) => Promise<Todo[]>;
  toggleTodo: (targetId: number, todos: Todo[]) => Promise<Todo[]>;
  deleteTodo: (targetId: number, todos: Todo[]) => Promise<Todo[]>;
  editTodo: (newText: string, targetId: number, todos: Todo[]) => Promise<Todo[]>;
  clearCompleted: (todos: Todo[]) => Promise<Todo[]>;
  isVisible: (done: boolean, filter: string) => Promise<boolean>;
  completedCount: (todos: Todo[]) => Promise<number>;
  remainingCount: (completed: number, total: number) => Promise<number>;
};

const sigil = sigilRaw as unknown as SigilDomain;
const STORAGE_KEY = 'sigil.todo-react.v1';

type PersistedState = { todos: Todo[]; nextId: number };

function loadState(): PersistedState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { todos: [], nextId: 1 };
    const parsed = JSON.parse(raw) as Partial<PersistedState>;
    const todos = Array.isArray(parsed.todos) ? (parsed.todos as Todo[]) : [];
    const maxId = todos.reduce((m, t) => (typeof t.id === 'number' && t.id > m ? t.id : m), 0);
    const nextId = typeof parsed.nextId === 'number' && parsed.nextId > maxId ? parsed.nextId : maxId + 1;
    return { todos, nextId };
  } catch {
    return { todos: [], nextId: 1 };
  }
}

function saveState(state: PersistedState): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function TodoApp(): JSX.Element {
  const initial = loadState();
  const [todos, setTodos] = useState<Todo[]>(initial.todos);
  const [nextId, setNextId] = useState<number>(initial.nextId);
  const [draft, setDraft] = useState('');
  const [filter, setFilter] = useState<Filter>('all');
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editingDraft, setEditingDraft] = useState('');
  const [visible, setVisible] = useState<Todo[]>(initial.todos);
  const [completedCount, setCompletedCount] = useState(0);
  const [activeCount, setActiveCount] = useState(initial.todos.length);
  const [appError, setAppError] = useState<string | null>(null);
  const todosRef = useRef(todos);
  const nextIdRef = useRef(nextId);

  useEffect(() => {
    todosRef.current = todos;
    nextIdRef.current = nextId;
    saveState({ todos, nextId });
  }, [todos, nextId]);

  useEffect(() => {
    let cancelled = false;

    async function deriveView(): Promise<void> {
      try {
        const visibility = await Promise.all(
          todos.map((todo) => sigil.isVisible(todo.done, filter))
        );
        const doneCount = await sigil.completedCount(todos);
        const remaining = await sigil.remainingCount(doneCount, todos.length);
        if (cancelled) return;
        setVisible(todos.filter((_, index) => visibility[index]));
        setCompletedCount(doneCount);
        setActiveCount(remaining);
        setAppError(null);
      } catch (error) {
        if (cancelled) return;
        setAppError(errorMessage(error));
      }
    }

    void deriveView();
    return () => {
      cancelled = true;
    };
  }, [filter, todos]);

  async function submitAdd(): Promise<void> {
    const text = draft.trim();
    if (!(await sigil.canAdd(text))) return;
    try {
      const id = nextIdRef.current;
      const nextTodos = await sigil.addTodo(id, text, todosRef.current);
      setTodos(nextTodos);
      setNextId(id + 1);
      setDraft('');
      setAppError(null);
    } catch (error) {
      setAppError(errorMessage(error));
    }
  }

  async function submitEdit(id: number): Promise<void> {
    const text = editingDraft.trim();
    if (!(await sigil.canAdd(text))) return;
    try {
      const nextTodos = await sigil.editTodo(text, id, todosRef.current);
      setTodos(nextTodos);
      setEditingId(null);
      setEditingDraft('');
      setAppError(null);
    } catch (error) {
      setAppError(errorMessage(error));
    }
  }

  return (
    <div className="todo-shell">
      <header className="todo-header">
        <h1>Sigil TODO (React + TS Bridge)</h1>
        <p>Sigil owns deterministic list transforms. React + TypeScript own UI, events, and localStorage.</p>
      </header>
      {appError ? <p className="empty-state">App error: {appError}</p> : null}
      <div className="todo-controls">
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => { if (e.key === 'Enter') void submitAdd(); }}
          placeholder="Write a task and press Enter"
          aria-label="New todo"
        />
        <button className="primary" onClick={() => void submitAdd()}>Add Todo</button>
      </div>
      <div className="todo-toolbar">
        <div className="filter-group" role="tablist" aria-label="Filters">
          {(['all', 'active', 'completed'] as const).map((value) => (
            <button key={value} data-active={filter === value} onClick={() => setFilter(value)} role="tab" aria-selected={filter === value}>
              {value[0].toUpperCase() + value.slice(1)}
            </button>
          ))}
        </div>
        <div className="filter-group">
          <span aria-live="polite">{activeCount} active / {completedCount} done</span>
          <button
            className="danger"
            onClick={() => {
              void sigil.clearCompleted(todosRef.current)
                .then((nextTodos) => {
                  setTodos(nextTodos);
                  setAppError(null);
                })
                .catch((error) => setAppError(errorMessage(error)));
            }}
          >
            Clear Completed
          </button>
        </div>
      </div>
      {visible.length === 0 ? (
        <p className="empty-state">No todos in this filter yet.</p>
      ) : (
        <ul className="todo-list">
          {visible.map((todo) => {
            const isEditing = editingId === todo.id;
            return (
              <li key={todo.id} className={`todo-item${todo.done ? ' done' : ''}`}>
                <input
                  type="checkbox"
                  checked={todo.done}
                  onChange={() => {
                    void sigil.toggleTodo(todo.id, todosRef.current)
                      .then((nextTodos) => {
                        setTodos(nextTodos);
                        setAppError(null);
                      })
                      .catch((error) => setAppError(errorMessage(error)));
                  }}
                  aria-label={`Toggle ${todo.text}`}
                />
                {isEditing ? (
                  <input
                    className="todo-text-input"
                    value={editingDraft}
                    onChange={(e) => setEditingDraft(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') void submitEdit(todo.id);
                      if (e.key === 'Escape') { setEditingId(null); setEditingDraft(''); }
                    }}
                    autoFocus
                    aria-label={`Edit ${todo.text}`}
                  />
                ) : (
                  <span className="todo-text">{todo.text}</span>
                )}
                <div className="todo-item-actions">
                  {isEditing ? (
                    <>
                      <button onClick={() => void submitEdit(todo.id)}>Save</button>
                      <button onClick={() => { setEditingId(null); setEditingDraft(''); }}>Cancel</button>
                    </>
                  ) : (
                    <>
                      <button onClick={() => { setEditingId(todo.id); setEditingDraft(todo.text); }}>Edit</button>
                      <button
                        className="danger"
                        onClick={() => {
                          void sigil.deleteTodo(todo.id, todosRef.current)
                            .then((nextTodos) => {
                              setTodos(nextTodos);
                              setAppError(null);
                            })
                            .catch((error) => setAppError(errorMessage(error)));
                        }}
                      >
                        Delete
                      </button>
                    </>
                  )}
                </div>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}

export function mountTodoApp(element: HTMLElement): void {
  createRoot(element).render(<TodoApp />);
}
