⟦
  Mint Standard Library - I/O Operations

  File system and console I/O using Node.js FFI.
  All functions have !IO effect annotation.
⟧

e fs/promises
e console

⟦ ========================================================================
   FILE OPERATIONS
   ======================================================================== ⟧

⟦ Read file as UTF-8 string ⟧
λread_file(path:𝕊)→!IO 𝕊=fs/promises.readFile(path,"utf8")

⟦ Write string to file (overwrites) ⟧
λwrite_file(path:𝕊,content:𝕊)→!IO 𝕌=fs/promises.writeFile(path,content,"utf8")

⟦ Append string to file ⟧
λappend_file(path:𝕊,content:𝕊)→!IO 𝕌=fs/promises.appendFile(path,content,"utf8")

⟦ Check if file exists ⟧
λfile_exists(path:𝕊)→!IO 𝔹=fs/promises.access(path).then(()→⊤).catch(()→⊥)

⟦ Delete file ⟧
λdelete_file(path:𝕊)→!IO 𝕌=fs/promises.unlink(path)

⟦ Create directory ⟧
λmake_dir(path:𝕊)→!IO 𝕌=fs/promises.mkdir(path)

⟦ List directory contents ⟧
λlist_dir(path:𝕊)→!IO [𝕊]=fs/promises.readdir(path)

⟦ ========================================================================
   CONSOLE OPERATIONS
   ======================================================================== ⟧

⟦ Print to stdout (with newline) ⟧
λprintln(msg:𝕊)→!IO 𝕌=console.log(msg)

⟦ Print to stdout (without newline) ⟧
λprint(msg:𝕊)→!IO 𝕌=process.stdout.write(msg)

⟦ Print to stderr ⟧
λeprintln(msg:𝕊)→!IO 𝕌=console.error(msg)

⟦ Print warning ⟧
λwarn(msg:𝕊)→!IO 𝕌=console.warn(msg)

⟦ Print debug info ⟧
λdebug(msg:𝕊)→!IO 𝕌=console.debug(msg)
