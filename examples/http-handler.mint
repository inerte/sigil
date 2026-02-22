t Request={path:𝕊,method:𝕊,body:𝕊}
t Response={status:ℤ,body:𝕊,headers:{𝕊:𝕊}}
t Error={code:ℤ,msg:𝕊}

λhandle_request(req:Request)→Result[Response,Error]≡req.path{"/users"→get_users(req)|"/health"→Ok(Response{status:200,body:"OK",headers:{}})|_→Err(Error{code:404,msg:"Not found"})}

λget_users(req:Request)→Result[Response,Error]=Ok(Response{status:200,body:"[{\"id\":1,\"name\":\"Alice\"}]",headers:{"Content-Type":"application/json"}})
