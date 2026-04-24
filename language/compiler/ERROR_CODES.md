# Sigil Error Codes Reference

Complete list of the current Sigil compiler error codes.

## Error Format

All errors follow the format:
```
CODE file:line:column message (found X, expected Y)
```

Example:
```
SIGIL-LEX-TAB test.sigil:5:10 tab characters not allowed (use spaces for indentation)
```

## Lexer Errors (SIGIL-LEX-*)

### SIGIL-LEX-TAB
**Description:** Tab characters are not allowed in Sigil source code.
**Message:** "tab characters not allowed (use spaces for indentation)"
**How to fix:** Replace tabs with spaces

### SIGIL-LEX-CRLF
**Description:** Standalone carriage return (\r) without newline (\n) is not allowed.
**Message:** "standalone carriage return not allowed"
**How to fix:** Use \n for line endings (LF), not \r\n (CRLF)

### SIGIL-LEX-UNTERMINATED-STRING
**Description:** String literal is not closed.
**Message:** "unterminated string literal"
**How to fix:** Add closing " to your string

### SIGIL-LEX-UNTERMINATED-COMMENT
**Description:** Multi-line comment ⟦...⟧ is not closed.
**Message:** "unterminated multi-line comment"
**How to fix:** Add closing ⟧ to your comment

### SIGIL-LEX-EMPTY-CHAR
**Description:** Character literal '' contains no character.
**Message:** "empty character literal"
**How to fix:** Add a character between the quotes: 'a'

### SIGIL-LEX-CHAR-LENGTH
**Description:** Character literal contains more than one character.
**Message:** "character literal must contain exactly one character"
**How to fix:** Use a string "abc" or single character 'a'

### SIGIL-LEX-UNTERMINATED-CHAR
**Description:** Character literal is not closed.
**Message:** "unterminated character literal"
**How to fix:** Add closing ' to your character

### SIGIL-LEX-INVALID-ESCAPE
**Description:** Invalid escape sequence in string or character literal.
**Message:** "invalid escape sequence: \X"
**Valid escapes:** \n \t \r \\ \" \'
**How to fix:** Use a valid escape sequence

### SIGIL-LEX-UNEXPECTED-CHAR
**Description:** Unexpected character in source code.
**Message:** "unexpected character: X (U+XXXX)"
**How to fix:** Remove or replace the unexpected character

### SIGIL-LEX-LEGACY-BOOL
**Description:** Legacy Unicode boolean literal is no longer valid Sigil syntax.
**Message:** "use \"true\" instead of \"⊤\"" or "use \"false\" instead of \"⊥\""
**How to fix:** Replace `⊤` with `true` and `⊥` with `false`

## Parser Errors (SIGIL-PARSE-*)

### SIGIL-PARSE-CONST-NAME
**Description:** Constant name must be lowercase.
**Message:** "invalid constant name"
**Example:** `c Pi=(3.14:Float)` => should be `c pi=(3.14:Float)`
**How to fix:** Use lowercase for constant names

### SIGIL-PARSE-CONST-UNTYPED
**Description:** Constant value must have type ascription.
**Message:** "const value must use type ascription: c name=(value:Type)"
**Example:** `c x=5` => should be `c x=(5:Int)`
**How to fix:** Wrap value with type ascription (value:Type)

### SIGIL-PARSE-NS-SEP
**Description:** Invalid namespace separator.
**Message:** "invalid namespace separator"
**Example:** `§httpClient.headers.empty` => should be `§httpClient.headers::empty`
**How to fix:** Use a Sigil root at the front and `::` only for deeper nested module segments

### SIGIL-PARSE-LOCAL-BINDING
**Description:** Invalid local binding keyword.
**Message:** "invalid local binding keyword"
**Example:** `let x=5` => should be `l x=(5:Int)`
**How to fix:** Use `l` not `let` for local bindings

### SIGIL-PARSE-UNEXPECTED-TOKEN
**Description:** Unexpected token in source code.
**Message:** "unexpected token"
**How to fix:** Check syntax, the parser expected a different token

## Canonical Form Errors (SIGIL-CANON-*)

### SIGIL-CANON-DUPLICATE-TYPE
**Description:** Duplicate type declaration with same name.
**Message:** "Duplicate type declaration: \"Name\""
**How to fix:** Remove duplicate type declaration

### SIGIL-CANON-DUPLICATE-EXTERN
**Description:** Duplicate extern declaration with same name.
**Message:** "Duplicate extern declaration: \"Name\""
**How to fix:** Remove duplicate extern declaration

### SIGIL-CANON-DUPLICATE-CONST
**Description:** Duplicate constant declaration.
**Message:** "Duplicate const declaration: \"name\""
**How to fix:** Remove duplicate constant

### SIGIL-CANON-DUPLICATE-FUNCTION
**Description:** Duplicate function declaration.
**Message:** "Duplicate function declaration: \"name\""
**How to fix:** Remove duplicate function or rename

### SIGIL-CANON-DUPLICATE-TEST
**Description:** Duplicate test block with same name.
**Message:** "Duplicate test declaration: \"name\""
**How to fix:** Remove duplicate test or rename

### SIGIL-CANON-EOF-NEWLINE
**Description:** File must end with a newline character.
**Message:** "file must end with newline"
**How to fix:** Add \n at end of file

### SIGIL-CANON-TRAILING-WHITESPACE
**Description:** Line has trailing whitespace.
**Message:** "trailing whitespace"
**How to fix:** Remove spaces/tabs at end of line

### SIGIL-CANON-BLANK-LINES
**Description:** Multiple consecutive blank lines.
**Message:** "multiple consecutive blank lines"
**How to fix:** Use at most one blank line between declarations

### SIGIL-CANON-LIB-NO-MAIN
**Description:** Library files (.lib.sigil) cannot have main() function.
**Message:** "Library files cannot have main() function"
**How to fix:** Remove main() or rename file to .sigil

### SIGIL-CANON-EXEC-NEEDS-MAIN
**Description:** Executable files (.sigil) must have main() function.
**Message:** "Executable files must have main() function"
**How to fix:** Add main()=>Unit=() or rename to .lib.sigil

### SIGIL-CANON-TEST-NEEDS-MAIN
**Description:** Test files must have main() function.
**Message:** "Test files must have main() function"
**How to fix:** Add main()=>Unit=() to test file

### SIGIL-CANON-TEST-LOCATION
**Description:** Test blocks must be in files under tests/ directory.
**Message:** "test declarations only allowed under project tests/ directory"
**How to fix:** Move test blocks to tests/*.sigil files

### SIGIL-CANON-TEST-PATH
**Description:** Test file path invalid (similar to TEST-LOCATION).
**Message:** "Test declarations only allowed under project tests/ directory"
**How to fix:** Move test file to tests/ directory

### SIGIL-CANON-FILENAME-CASE
**Description:** Filename does not start with a lowercase letter.
**Message:** "filenames must start with a lowercase letter"
**Example:** UserService.lib.sigil => userService.lib.sigil
**How to fix:** Rename file to lowerCamelCase

### SIGIL-CANON-FILENAME-INVALID-CHAR
**Description:** Filename contains invalid characters (underscore, hyphen, space, etc).
**Message:** "filenames cannot contain X"
**Example:** user_service.lib.sigil => userService.lib.sigil
**How to fix:** Use lowerCamelCase only

### SIGIL-CANON-FILENAME-FORMAT
**Description:** Filename format violation (not lowerCamelCase or starts with a digit).
**Message:** Various format error messages
**How to fix:** Follow lowerCamelCase format

### SIGIL-CANON-IDENTIFIER-FORM
**Description:** Value-level identifier is not lowerCamelCase.
**Message:** "value identifiers must be lowerCamelCase"
**How to fix:** Rename the identifier to lowerCamelCase

### SIGIL-CANON-TYPE-NAME-FORM
**Description:** Type declaration name is not UpperCamelCase.
**Message:** "type names must be UpperCamelCase"
**How to fix:** Rename the type to UpperCamelCase

### SIGIL-CANON-CONSTRUCTOR-NAME-FORM
**Description:** Constructor name is not UpperCamelCase.
**Message:** "constructor names must be UpperCamelCase"
**How to fix:** Rename the constructor to UpperCamelCase

### SIGIL-CANON-TYPE-VAR-FORM
**Description:** Type variable is not UpperCamelCase.
**Message:** "type variables must be UpperCamelCase"
**How to fix:** Rename the type variable to UpperCamelCase

### SIGIL-CANON-RECORD-FIELD-FORM
**Description:** Record field is not lowerCamelCase.
**Message:** "record fields must be lowerCamelCase"
**How to fix:** Rename the field to lowerCamelCase

### SIGIL-CANON-MODULE-PATH-FORM
**Description:** Module path segment is not lowerCamelCase.
**Message:** "module path segments must be lowerCamelCase"
**How to fix:** Rename the module/file stem to lowerCamelCase

### SIGIL-CANON-RECURSION-ACCUMULATOR
**Description:** Accumulator-passing style detected.
**Message:** "Accumulator-passing style detected in function 'name'"
**Example:** `λfact(n:Int,acc:Int)=>Int match n{0=>acc|n=>fact(n-1,n*acc)}`
**How to fix:** Use simple recursion without accumulator parameters

### SIGIL-CANON-RECURSION-COLLECTION-NONSTRUCTURAL
**Description:** Recursive function on collection doesn't use structural recursion.
**Message:** "Recursive function 'name' has collection parameter but doesn't use structural recursion"
**How to fix:** Match on list structure: `match list{[]=>...,[x⧺xs]=>...}`

### SIGIL-CANON-RECURSION-CPS
**Description:** Continuation-Passing Style (CPS) detected.
**Message:** "Recursive function 'name' returns a function type"
**How to fix:** Return a value, not a function

### SIGIL-CANON-RECURSION-APPEND-RESULT
**Description:** Recursive function appends to the recursive result.
**Message:** "Recursive function 'name' appends to the recursive result"
**Example:** `λreverse(xs:[Int])=>[Int] match xs{[]=>[]|[x,.rest]=>reverse(rest)⧺[x]}`
**How to fix:** Use `map`, `filter`, `reduce`, or a wrapper plus accumulator helper with one final reverse

### SIGIL-CANON-RECURSION-ALL-CLONE
**Description:** Exact recursive all clone detected.
**Message:** "Recursive function 'name' is a hand-rolled all"
**Example:** `λallPositive(xs:[Int])=>Bool match xs{[]=>true|[x,.rest]=>isPositive(x) and allPositive(rest)}`
**How to fix:** Use `§list.all(pred,xs)`

### SIGIL-CANON-RECURSION-ANY-CLONE
**Description:** Exact recursive any clone detected.
**Message:** "Recursive function 'name' is a hand-rolled any"
**Example:** `λanyEven(xs:[Int])=>Bool match xs{[]=>false|[x,.rest]=>isEven(x) or anyEven(rest)}`
**How to fix:** Use `§list.any(pred,xs)`

### SIGIL-CANON-RECURSION-MAP-CLONE
**Description:** Exact recursive map clone detected.
**Message:** "Recursive function 'name' is a hand-rolled map"
**Example:** `λdouble(xs:[Int])=>[Int] match xs{[]=>[]|[x,.rest]=>[x*2]⧺double(rest)}`
**How to fix:** Use `xs map f`

### SIGIL-CANON-RECURSION-FILTER-CLONE
**Description:** Exact recursive filter clone detected.
**Message:** "Recursive function 'name' is a hand-rolled filter"
**Example:** `λevens(xs:[Int])=>[Int] match xs{[]=>[]|[x,.rest]=>match isEven(x){true=>[x]⧺evens(rest)|false=>evens(rest)}}`
**How to fix:** Use `xs filter pred`

### SIGIL-CANON-RECURSION-FIND-CLONE
**Description:** Exact recursive find clone detected.
**Message:** "Recursive function 'name' is a hand-rolled find"
**Example:** `λfindEven(xs:[Int])=>Option[Int] match xs{[]=>None()|[x,.rest]=>match isEven(x){true=>Some(x)|false=>findEven(rest)}}`
**How to fix:** Use `§list.find(pred,xs)`

### SIGIL-CANON-RECURSION-FLATMAP-CLONE
**Description:** Exact recursive flatMap clone detected.
**Message:** "Recursive function 'name' is a hand-rolled flatMap"
**Example:** `λexplode(xs:[Int])=>[Int] match xs{[]=>[]|[x,.rest]=>digits(x)⧺explode(rest)}`
**How to fix:** Use `§list.flatMap(fn,xs)`

### SIGIL-CANON-RECURSION-REVERSE-CLONE
**Description:** Exact recursive reverse clone detected.
**Message:** "Recursive function 'name' is a hand-rolled reverse"
**Example:** `λreverse(xs:[Int])=>[Int] match xs{[]=>[]|[x,.rest]=>reverse(rest)⧺[x]}`
**How to fix:** Use `§list.reverse`

### SIGIL-CANON-RECURSION-FOLD-CLONE
**Description:** Exact recursive fold clone detected.
**Message:** "Recursive function 'name' is a hand-rolled fold"
**Example:** `λsum(xs:[Int])=>Int match xs{[]=>0|[x,.rest]=>x+sum(rest)}`
**How to fix:** Use `xs reduce fn from init` or `§list.fold`

### SIGIL-CANON-BRANCHING-SELF-RECURSION
**Description:** Non-canonical sibling self-calls over the same directly reduced parameter.
**Message:** "Recursive function 'name' uses non-canonical branching self-recursion"
**Example:** `fib(n-1)+fib(n-2)`
**Why it is rejected:** This exact shape duplicates work instead of following one canonical recursion path.
**How to fix:** Use a wrapper plus helper accumulator/state-threading function, or another canonical helper shape that performs one recursive step at a time.

### SIGIL-CANON-TRAVERSAL-FILTER-COUNT
**Description:** Filter followed by length is a non-canonical counting shape.
**Message:** "filter followed by length is not canonical"
**Example:** `#(xs filter pred)`
**How to fix:** Use `§list.countIf(pred,xs)`

### SIGIL-CANON-PARAM-ORDER
**Description:** Function parameters out of alphabetical order.
**Message:** "Parameter 'X' out of alphabetical order in function 'name'"
**Example:** `λf(z:Int,a:Int)=>Int=a+z` => should be `λf(a:Int,z:Int)=>Int=a+z`
**How to fix:** Sort parameters alphabetically by name

### SIGIL-CANON-EFFECT-ORDER
**Description:** Function effects out of alphabetical order.
**Message:** "Effect 'X' out of alphabetical order in function 'name'"
**Example:** `λf()=>!Process!Fs Unit=()` => should be `λf()=>!Fs!Process Unit=()`
**How to fix:** Sort effects alphabetically

### SIGIL-CANON-RECORD-TYPE-FIELD-ORDER
**Description:** Product type fields out of alphabetical order.
**Message:** "Record type field 'X' out of alphabetical order in 'TypeName'"
**Example:** `t User={name:String,age:Int}` => should be `t User={age:Int,name:String}`
**How to fix:** Sort record type fields alphabetically by field name

### SIGIL-CANON-RECORD-LITERAL-FIELD-ORDER
**Description:** Record literal fields out of alphabetical order.
**Message:** "Record literal field 'X' out of alphabetical order"
**Example:** `User{name:\"A\",age:1}` => should be `User{age:1,name:\"A\"}`
**How to fix:** Sort record literal fields alphabetically by field name

### SIGIL-CANON-RECORD-PATTERN-FIELD-ORDER
**Description:** Record pattern fields out of alphabetical order.
**Message:** "Record pattern field 'X' out of alphabetical order"
**Example:** `match p{{name,age}=>...}` => should be `match p{{age,name}=>...}`
**How to fix:** Sort record pattern fields alphabetically by field name

### SIGIL-CANON-NO-SHADOWING
**Description:** Local binding shadows an existing local binding from the same or an enclosing lexical scope.
**Message:** "Binding 'name' shadows an existing X binding"
**Example:** `λf(value:Int)=>Int=l value=(2:Int);value`
**How to fix:** Use a new name instead of rebinding an existing local, parameter, lambda parameter, or pattern binding

### SIGIL-CANON-LET-UNTYPED
**Description:** Let binding must have type ascription.
**Message:** "Let binding 'name' must have type ascription"
**Example:** `l x=5` => should be `l x=(5:Int)`
**How to fix:** Use type ascription: l name=(value:Type)

### SIGIL-CANON-DEAD-PURE-DISCARD
**Description:** Wildcard sequencing discards a pure expression.
**Message:** "Wildcard sequencing must not discard pure expressions"
**Example:** `l _=((1+2):Int)` => should be deleted or rewritten to use the value
**How to fix:** Use the value, inline it into a real use, or delete it. `l _=(...)` is reserved for sequencing observable effects.

### SIGIL-CANON-DECL-CATEGORY-ORDER
**Description:** Declarations out of category order.
**Message:** "Declarations out of category order"
**Expected order:** types => externs => consts => functions => tests
**How to fix:** Reorder declarations by category

### SIGIL-CANON-DECL-EXPORT-ORDER
**Description:** Exported declarations must come before non-exported.
**Message:** "Declarations with 'export' must come before non-exported declarations"
**How to fix:** Move exported declarations to top of category

### SIGIL-CANON-DECL-ALPHABETICAL
**Description:** Declarations within category not alphabetical.
**Message:** "Declarations within category must be alphabetical"
**Example:** Functions `bar`, `foo`, `add` => should be `add`, `bar`, `foo`
**How to fix:** Sort declarations alphabetically within each category

### SIGIL-CANON-EXTERN-MEMBER-ORDER
**Description:** Extern members not in alphabetical order.
**Message:** "Extern members must be in alphabetical order"
**How to fix:** Sort extern member declarations alphabetically

## Type Checker Errors (SIGIL-TYPE-*)

### SIGIL-TYPE-ERROR
**Description:** Generic type error.
**Message:** Various type mismatch messages
**How to fix:** Ensure types match expected types

### SIGIL-TYPE-MODULE-NOT-EXPORTED
**Description:** Trying to access non-exported module member.
**Message:** "Module member not exported"
**How to fix:** Export the member in the module or don't access it

### SIGIL-TYPE-MATCH-NON-EXHAUSTIVE
**Description:** Match expression leaves part of the scrutinee space uncovered.
**Message:** "Non-exhaustive match expression"
**How to fix:** Add the missing arm(s) suggested in the compile error details, or add a canonical catch-all arm when the uncovered space is intentionally broad

### SIGIL-TYPE-MATCH-REDUNDANT-PATTERN
**Description:** Match arm is already fully covered by earlier arms.
**Message:** "Redundant pattern in match expression"
**How to fix:** Remove the redundant arm or rewrite earlier arms/guards so the arm covers a real remaining case

### SIGIL-TYPE-MATCH-UNREACHABLE-ARM
**Description:** Match arm appears after earlier arms have already covered the full scrutinee space.
**Message:** "Unreachable match arm"
**How to fix:** Remove the dead arm or rewrite the earlier arms if the intended branching order was different

### SIGIL-TYPE-UNREACHABLE-CODE
**Description:** Code appears after an expression that is guaranteed to terminate in the same sequence.
**Message:** "Unreachable code after terminating expression"
**How to fix:** Remove the dead code, or rewrite the earlier expression so it can continue on that path

## Mutability Errors (SIGIL-MUTABILITY-*)

### SIGIL-MUTABILITY-INVALID
**Description:** Invalid mutability usage.
**Message:** Various mutability error messages
**How to fix:** Check mutability rules

## CLI Errors (SIGIL-CLI-*)

### SIGIL-CLI-USAGE
**Description:** Missing command or arguments.
**Message:** "missing command", "missing file argument", etc
**How to fix:** Provide required command/arguments

### SIGIL-CLI-UNKNOWN-COMMAND
**Description:** Unknown command provided.
**Message:** "unknown command"
**How to fix:** Use valid command: compile, run, test, parse, lex

### SIGIL-CLI-UNSUPPORTED-OPTION
**Description:** Unsupported option provided.
**Message:** "unsupported option"
**How to fix:** Remove unsupported option

### SIGIL-CLI-UNEXPECTED
**Description:** Unexpected CLI error.
**Message:** Various error messages
**How to fix:** Check error message for details

### SIGIL-CLI-IMPORT-NOT-FOUND
**Description:** Cannot resolve rooted module reference.
**Message:** "cannot resolve module: path"
**How to fix:** Check the rooted module path exists

### SIGIL-CLI-IMPORT-CYCLE
**Description:** Circular module dependency detected.
**Message:** "module cycle detected"
**How to fix:** Remove circular module dependency

### SIGIL-CLI-INVALID-IMPORT
**Description:** Invalid Sigil module ID.
**Message:** "invalid sigil module id"
**How to fix:** Use a valid rooted module path

### SIGIL-CLI-PROJECT-ROOT-REQUIRED
**Description:** Project module reference requires sigil project root.
**Message:** "project module reference requires sigil project root"
**How to fix:** Ensure project has proper structure

### SIGIL-CLI-PROJECT-INIT-INVALID-NAME
**Description:** Init target directory name cannot be converted into a canonical lowerCamel Sigil project name.
**Message:** "target directory name `...` cannot be converted into a lowerCamel Sigil project name"
**How to fix:** Rename the target directory so it derives to a lowerCamel ASCII name

### SIGIL-CLI-PROJECT-INIT-CONFLICT
**Description:** Init target already contains Sigil project metadata or incompatible scaffold paths.
**Message:** "target already contains sigil.json" or "target already contains non-directory scaffold path `src`"
**How to fix:** Do not overwrite an existing `sigil.json`, and ensure `src`, `tests`, and `.local` are directories if they already exist

## Runtime Errors (SIGIL-RUNTIME-*, SIGIL-RUN-*)

### SIGIL-RUNTIME-CHILD-EXIT
**Description:** Child process exited with nonzero status.
**Message:** "child process exited with nonzero status"
**How to fix:** Check runtime errors in your code

### SIGIL-RUN-ENGINE-NOT-FOUND
**Description:** Runtime engine (Node.js, Deno, etc) not found.
**Message:** "runtime engine not available"
**How to fix:** Install Node.js or Deno

## Protocol Errors (SIGIL-PROTO-*)

### SIGIL-PROTO-UNKNOWN-TYPE
**Description:** A `protocol` declaration references a type that is not declared in the same file.
**How to fix:** Declare the type with `t TypeName=...` before the protocol declaration.

### SIGIL-PROTO-UNKNOWN-STATE
**Description:** A state name used in a `requires`/`ensures` clause is not a valid state in the handle's protocol.
**How to fix:** Check the protocol declaration for the type's valid state names.

### SIGIL-PROTO-STATE-VIOLATION
**Description:** A function call's `requires` state clause could not be proven — the handle may be in the wrong state.
**How to fix:** Ensure the handle is in the required state before calling this function. The Z3 model in the error shows what state was inferred.

### SIGIL-PROTO-MISSING-CONTRACT
**Description:** A function listed in a protocol's `via` clause lacks matching `requires`/`ensures` state annotations.
**How to fix:** Add `requires handle.state=StateA` and `ensures handle.state=StateB` to the function declaration.

### SIGIL-PROTO-DUPLICATE
**Description:** Two `protocol` declarations exist for the same type.
**How to fix:** Keep only one `protocol` declaration per type.

## Total Error Codes: 64

- Lexer: 9 codes
- Parser: 5 codes
- Canonical: 27 codes
- Typecheck: 5 codes
- Mutability: 1 code
- CLI: 10 codes
- Runtime: 2 codes
- Protocol: 5 codes
