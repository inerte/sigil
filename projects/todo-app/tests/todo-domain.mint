i src/todo-domain

t Todo={id:ℤ,text:𝕊,done:𝔹}

λlenTodos(todos:[Todo])→ℤ≡todos{
  []→0|
  [_,.rest]→1+lenTodos(rest)
}

test "todo add prepends item" {
  ≡src/todo-domain.addTodo([],1,"Task"){
    [todo]→todo.id=1∧todo.text="Task"∧todo.done=⊥|
    _→⊥
  }
}

test "todo toggle flips done flag" {
  src/todo-domain.toggleTodo([Todo{id:1,text:"Task",done:⊥}],1)[0].done=⊤
}

test "todo edit updates text" {
  src/todo-domain.editTodo([Todo{id:1,text:"Old",done:⊥}],1,"New")[0].text="New"
}

test "todo delete removes target" {
  ≡src/todo-domain.deleteTodo([Todo{id:1,text:"A",done:⊥},Todo{id:2,text:"B",done:⊥}],1){
    [todo]→todo.id=2∧todo.text="B"|
    _→⊥
  }
}

test "todo clearCompleted keeps active only" {
  ≡src/todo-domain.clearCompleted([Todo{id:1,text:"A",done:⊤},Todo{id:2,text:"B",done:⊥}]){
    [todo]→todo.id=2∧todo.done=⊥|
    _→⊥
  }
}

test "todo completedCount counts completed" {
  src/todo-domain.completedCount([Todo{id:1,text:"A",done:⊤},Todo{id:2,text:"B",done:⊥},Todo{id:3,text:"C",done:⊤}])=2
}

test "todo delete reduces length" {
  lenTodos(src/todo-domain.deleteTodo([Todo{id:1,text:"A",done:⊥},Todo{id:2,text:"B",done:⊥}],1))=1
}
