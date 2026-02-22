λmap[T,U](fn:λ(T)→U,list:[T])→[U]≡list{[]→[]|[x,.xs]→[fn(x),.map(fn,xs)]}
λfilter[T](pred:λ(T)→𝔹,list:[T])→[T]≡list{[]→[]|[x,.xs]→≡pred(x){⊤→[x,.filter(pred,xs)]|⊥→filter(pred,xs)}}
λreduce[T,U](fn:λ(U,T)→U,init:U,list:[T])→U≡list{[]→init|[x,.xs]→reduce(fn,fn(init,x),xs)}
