⟦ Test len() and in_bounds predicate ⟧

e console
i stdlib/list_utils
i stdlib/list_predicates

λmain()→𝕌=console.log("len([1,2,3]): " ++ stdlib/list_utils.len([1,2,3]) ++ ", in_bounds(1,[1,2,3]): " ++ stdlib/list_predicates.in_bounds(1,[1,2,3]) ++ ", in_bounds(5,[1,2,3]): " ++ stdlib/list_predicates.in_bounds(5,[1,2,3]))
