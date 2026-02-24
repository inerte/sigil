export t Todo={id:ℤ,text:𝕊,done:𝔹}

export λcanAdd(text:𝕊)→𝔹=text≠""

export λaddTodo(todos:[Todo],id:ℤ,text:𝕊)→[Todo]=[Todo{id:id,text:text,done:⊥}]⧺todos

export λdeleteTodo(todos:[Todo],targetId:ℤ)→[Todo]=todos⊳λ(todo:Todo)→𝔹=todo.id≠targetId

export λclearCompleted(todos:[Todo])→[Todo]=todos⊳λ(todo:Todo)→𝔹=¬todo.done

export λtoggleTodo(todos:[Todo],targetId:ℤ)→[Todo]=todos↦λ(todo:Todo)→Todo≡todo.id=targetId{
  ⊤→Todo{id:todo.id,text:todo.text,done:¬todo.done}|
  ⊥→todo
}

export λeditTodo(todos:[Todo],targetId:ℤ,newText:𝕊)→[Todo]=todos↦λ(todo:Todo)→Todo≡todo.id=targetId{
  ⊤→Todo{id:todo.id,text:newText,done:todo.done}|
  ⊥→todo
}

export λisVisible(filter:𝕊,done:𝔹)→𝔹≡filter{
  "all"→⊤|
  "active"→¬done|
  "completed"→done|
  _→⊤
}

export λcompletedCount(todos:[Todo])→ℤ=todos⊕(λ(acc:ℤ,todo:Todo)→ℤ≡todo.done{
  ⊤→acc+1|
  ⊥→acc
})⊕0

export λremainingCount(total:ℤ,completed:ℤ)→ℤ=total-completed
