import React, { useEffect, useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';
import * as sigilRaw from './generated/todo-domain';

type Todo = { id: number; text: string; done: boolean };
type Filter = 'all' | 'active' | 'completed';

type SigilDomain = {
  canAdd: (text: string) => boolean;
  addTodo: (todos: Todo[], id: number, text: string) => Todo[];
  toggleTodo: (todos: Todo[], targetId: number) => Todo[];
  deleteTodo: (todos: Todo[], targetId: number) => Todo[];
  editTodo: (todos: Todo[], targetId: number, newText: string) => Todo[];
  clearCompleted: (todos: Todo[]) => Todo[];
  isVisible: (filter: string, done: boolean) => boolean;
  completedCount: (todos: Todo[]) => number;
  remainingCount: (total: number, completed: number) => number;
};

const sigil = sigilRaw as unknown as MintDomain;
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

function TodoApp(): JSX.Element {
  const initial = loadState();
  const [todos, setTodos] = useState<Todo[]>(initial.todos);
  const [nextId, setNextId] = useState<number>(initial.nextId);
  const [draft, setDraft] = useState('');
  const [filter, setFilter] = useState<Filter>('all');
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editingDraft, setEditingDraft] = useState('');

  useEffect(() => {
    saveState({ todos, nextId });
  }, [todos, nextId]);

  const visible = useMemo(() => {
    return todos.filter((todo) => sigil.isVisible(filter, todo.done));
  }, [filter, todos]);

  function submitAdd(): void {
    const text = draft.trim();
    if (!sigil.canAdd(text)) return;
    setTodos((prev) => sigil.addTodo(prev, nextId, text));
    setNextId((n) => n + 1);
    setDraft('');
  }

  function submitEdit(id: number): void {
    const text = editingDraft.trim();
    if (!sigil.canAdd(text)) return;
    setTodos((prev) => sigil.editTodo(prev, id, text));
    setEditingId(null);
    setEditingDraft('');
  }

  const completedCount = sigil.completedCount(todos);
  const activeCount = sigil.remainingCount(todos.length, completedCount);

  return (
    <div className="todo-shell">
      <header className="todo-header">
        <h1>Mint TODO (React + TS Bridge)</h1>
        <p>Mint owns deterministic list transforms. React + TypeScript own UI, events, and localStorage.</p>
      </header>
      <div className="todo-controls">
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => { if (e.key === 'Enter') submitAdd(); }}
          placeholder="Write a task and press Enter"
          aria-label="New todo"
        />
        <button className="primary" onClick={submitAdd}>Add Todo</button>
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
          <button className="danger" onClick={() => setTodos((prev) => sigil.clearCompleted(prev))}>Clear Completed</button>
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
                <input type="checkbox" checked={todo.done} onChange={() => setTodos((prev) => sigil.toggleTodo(prev, todo.id))} aria-label={`Toggle ${todo.text}`} />
                {isEditing ? (
                  <input
                    className="todo-text-input"
                    value={editingDraft}
                    onChange={(e) => setEditingDraft(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') submitEdit(todo.id);
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
                      <button onClick={() => submitEdit(todo.id)}>Save</button>
                      <button onClick={() => { setEditingId(null); setEditingDraft(''); }}>Cancel</button>
                    </>
                  ) : (
                    <>
                      <button onClick={() => { setEditingId(todo.id); setEditingDraft(todo.text); }}>Edit</button>
                      <button className="danger" onClick={() => setTodos((prev) => sigil.deleteTodo(prev, todo.id))}>Delete</button>
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
