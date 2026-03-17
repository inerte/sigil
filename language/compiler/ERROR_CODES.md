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
**Example:** `i stdlib.list` or `i stdlib/list` => should be `i stdlib::list`
**How to fix:** Use :: (U+22C5) for namespace separation

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

### SIGIL-CANON-DUPLICATE-IMPORT
**Description:** Duplicate import statement.
**Message:** "Duplicate import declaration: \"module\""
**How to fix:** Remove duplicate import

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
**How to fix:** Use `↦`, `⊳`, or a wrapper plus accumulator helper with one final reverse

### SIGIL-CANON-RECURSION-ALL-CLONE
**Description:** Exact recursive all clone detected.
**Message:** "Recursive function 'name' is a hand-rolled all"
**Example:** `λallPositive(xs:[Int])=>Bool match xs{[]=>true|[x,.rest]=>isPositive(x) and allPositive(rest)}`
**How to fix:** Use `stdlib::list.all(pred,xs)`

### SIGIL-CANON-RECURSION-ANY-CLONE
**Description:** Exact recursive any clone detected.
**Message:** "Recursive function 'name' is a hand-rolled any"
**Example:** `λanyEven(xs:[Int])=>Bool match xs{[]=>false|[x,.rest]=>isEven(x) or anyEven(rest)}`
**How to fix:** Use `stdlib::list.any(pred,xs)`

### SIGIL-CANON-RECURSION-MAP-CLONE
**Description:** Exact recursive map clone detected.
**Message:** "Recursive function 'name' is a hand-rolled map"
**Example:** `λdouble(xs:[Int])=>[Int] match xs{[]=>[]|[x,.rest]=>[x*2]⧺double(rest)}`
**How to fix:** Use `xs↦f`

### SIGIL-CANON-RECURSION-FILTER-CLONE
**Description:** Exact recursive filter clone detected.
**Message:** "Recursive function 'name' is a hand-rolled filter"
**Example:** `λevens(xs:[Int])=>[Int] match xs{[]=>[]|[x,.rest]=>match isEven(x){true=>[x]⧺evens(rest)|false=>evens(rest)}}`
**How to fix:** Use `xs⊳pred`

### SIGIL-CANON-RECURSION-FIND-CLONE
**Description:** Exact recursive find clone detected.
**Message:** "Recursive function 'name' is a hand-rolled find"
**Example:** `λfindEven(xs:[Int])=>Option[Int] match xs{[]=>None()|[x,.rest]=>match isEven(x){true=>Some(x)|false=>findEven(rest)}}`
**How to fix:** Use `stdlib::list.find(pred,xs)`

### SIGIL-CANON-RECURSION-REVERSE-CLONE
**Description:** Exact recursive reverse clone detected.
**Message:** "Recursive function 'name' is a hand-rolled reverse"
**Example:** `λreverse(xs:[Int])=>[Int] match xs{[]=>[]|[x,.rest]=>reverse(rest)⧺[x]}`
**How to fix:** Use `stdlib::list.reverse`

### SIGIL-CANON-RECURSION-FOLD-CLONE
**Description:** Exact recursive fold clone detected.
**Message:** "Recursive function 'name' is a hand-rolled fold"
**Example:** `λsum(xs:[Int])=>Int match xs{[]=>0|[x,.rest]=>x+sum(rest)}`
**How to fix:** Use `⊕` or `stdlib::list.fold`

### SIGIL-CANON-MATCH-BOOLEAN
**Description:** Cannot pattern match on boolean expression.
**Message:** "Cannot pattern match on boolean expression"
**Example:** `match (x<5){true=>...|false=>...}` => use `(x<5)=>...|...`
**How to fix:** Use if-expression syntax: `(condition)=>thenBranch|elseBranch`

### SIGIL-CANON-MATCH-TUPLE-BOOLEAN
**Description:** Cannot pattern match on tuple containing booleans.
**Message:** "Cannot pattern match on tuple containing booleans"
**How to fix:** Pattern match discriminates on structure, not boolean values

### SIGIL-CANON-PARAM-ORDER
**Description:** Function parameters out of alphabetical order.
**Message:** "Parameter 'X' out of alphabetical order in function 'name'"
**Example:** `λf(z:Int,a:Int)=>Int=a+z` => should be `λf(a:Int,z:Int)=>Int=a+z`
**How to fix:** Sort parameters alphabetically by name

### SIGIL-CANON-EFFECT-ORDER
**Description:** Function effects out of alphabetical order.
**Message:** "Effect 'X' out of alphabetical order in function 'name'"
**Example:** `λf()=>!IO !Error Unit=()` => should be `λf()=>!Error !IO Unit=()`
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

### SIGIL-CANON-DECL-CATEGORY-ORDER
**Description:** Declarations out of category order.
**Message:** "Declarations out of category order"
**Expected order:** types => externs => imports => consts => functions => tests
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
**Description:** Cannot resolve import.
**Message:** "cannot resolve import: path"
**How to fix:** Check import path exists

### SIGIL-CLI-IMPORT-CYCLE
**Description:** Circular import detected.
**Message:** "import cycle detected"
**How to fix:** Remove circular dependency

### SIGIL-CLI-INVALID-IMPORT
**Description:** Invalid import module ID.
**Message:** "invalid sigil import module id"
**How to fix:** Use valid import syntax

### SIGIL-CLI-PROJECT-ROOT-REQUIRED
**Description:** Project import requires sigil project root.
**Message:** "project import requires sigil project root"
**How to fix:** Ensure project has proper structure

## Runtime Errors (SIGIL-RUNTIME-*, SIGIL-RUN-*)

### SIGIL-RUNTIME-CHILD-EXIT
**Description:** Child process exited with nonzero status.
**Message:** "child process exited with nonzero status"
**How to fix:** Check runtime errors in your code

### SIGIL-RUN-ENGINE-NOT-FOUND
**Description:** Runtime engine (Node.js, Deno, etc) not found.
**Message:** "runtime engine not available"
**How to fix:** Install Node.js or Deno

## Total Error Codes: 56

- Lexer: 9 codes
- Parser: 5 codes
- Canonical: 29 codes
- Typecheck: 2 codes
- Mutability: 1 code
- CLI: 8 codes
- Runtime: 2 codes
