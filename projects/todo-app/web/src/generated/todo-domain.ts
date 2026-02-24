// type Todo (erased)

export function canAdd(text) {
  return (text !== "");
}

export function addTodo(todos, id, text) {
  return [{ "id": id, "text": text, "done": false }].concat(todos);
}

export function deleteTodo(todos, targetId) {
  return todos.filter(((todo) => (todo.id !== targetId)));
}

export function clearCompleted(todos) {
  return todos.filter(((todo) => !todo.done));
}

export function toggleTodo(todos, targetId) {
  return todos.map(((todo) => (() => {
  const __match = (todo.id === targetId);
  if (__match === true) {
    return { "id": todo.id, "text": todo.text, "done": !todo.done };
  }
  else if (__match === false) {
    return todo;
  }
  throw new Error('Match failed: no pattern matched');
})()));
}

export function editTodo(todos, targetId, newText) {
  return todos.map(((todo) => (() => {
  const __match = (todo.id === targetId);
  if (__match === true) {
    return { "id": todo.id, "text": newText, "done": todo.done };
  }
  else if (__match === false) {
    return todo;
  }
  throw new Error('Match failed: no pattern matched');
})()));
}

export function isVisible(filter, done) {
  return (() => {
  const __match = filter;
  if (__match === "all") {
    return true;
  }
  else if (__match === "active") {
    return !done;
  }
  else if (__match === "completed") {
    return done;
  }
  else {
    return true;
  }
  throw new Error('Match failed: no pattern matched');
})();
}

export function completedCount(todos) {
  return todos.reduce(((acc, todo) => (() => {
  const __match = todo.done;
  if (__match === true) {
    return (acc + 1);
  }
  else if (__match === false) {
    return acc;
  }
  throw new Error('Match failed: no pattern matched');
})()), 0);
}

export function remainingCount(total, completed) {
  return (total - completed);
}

