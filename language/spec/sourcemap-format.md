# Mint Semantic Source Map Format Specification

Version: 1.0.0
Last Updated: 2026-02-21

## Overview

Semantic source maps (.mint.map) are the **killer feature** of Mint. They provide AI-generated human-readable explanations of dense Mint code, similar to how JavaScript source maps connect minified code to original source.

## Philosophy

```
Traditional:     Source Code ────────────→ Minified Code
                (human readable)        (machine optimized)
                        ↕ source map

Mint:           Dense Code ──────────────→ Semantic Map
                (machine optimized)      (human explanation)
```

## File Format

Semantic maps are JSON files with the `.mint.map` extension, stored alongside `.mint` source files.

### Basic Structure

```json
{
  "version": 1,
  "file": "example.mint",
  "generated_by": "claude-opus-4.6",
  "generated_at": "2026-02-21T10:00:00Z",
  "mappings": {
    "identifier_or_range": {
      "range": [startOffset, endOffset],
      "summary": "Brief one-line description",
      "explanation": "Detailed multi-line explanation",
      "complexity": "Big-O complexity (for algorithms)",
      "warnings": ["Array of potential issues"],
      "examples": ["Array of usage examples"],
      "metadata": {}
    }
  },
  "metadata": {
    "intent": "Overall purpose of this file",
    "category": "File category",
    "tested": true,
    "performance_profile": "Performance characteristics"
  }
}
```

## Schema Definition

### Root Object

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `version` | integer | Yes | Schema version (currently 1) |
| `file` | string | Yes | Name of the .mint file this maps to |
| `generated_by` | string | Yes | AI model that generated this map |
| `generated_at` | ISO8601 timestamp | Yes | When this map was generated |
| `mappings` | object | Yes | Map of identifiers/ranges to explanations |
| `metadata` | object | No | File-level metadata |
| `dependencies` | array[string] | No | List of imported modules |
| `exports` | array[string] | No | List of exported identifiers |

### Mapping Entry

Each key in `mappings` can be:
1. An identifier (e.g., `"fibonacci"`)
2. A range descriptor (e.g., `"line_5"`, `"expr_at_42"`)

Each mapping entry has:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `range` | [int, int] | Yes | Character offsets [start, end) in source file |
| `summary` | string | Yes | One-line description (max 100 chars) |
| `explanation` | string | Yes | Detailed explanation (markdown supported) |
| `type` | string | No | Type signature if applicable |
| `complexity` | string | No | Time/space complexity (e.g., "O(n log n)") |
| `warnings` | array[string] | No | Potential issues or gotchas |
| `examples` | array[string] | No | Usage examples |
| `related` | array[string] | No | Related function/type names |
| `metadata` | object | No | Additional structured data |

### Metadata Object

File-level metadata:

| Field | Type | Description |
|-------|------|-------------|
| `intent` | string | Overall purpose of this file |
| `category` | string | File category (e.g., "pure_function", "io_handler") |
| `tested` | boolean | Whether tests exist for this file |
| `performance_profile` | string | Performance characteristics |
| `author` | string | Original author (if applicable) |
| `license` | string | License (if different from project) |

### Standard Categories

Recommended `category` values:

- `pure_function` - Pure mathematical functions
- `io_handler` - Functions with IO effects
- `data_structure` - Type definitions
- `algorithm` - Complex algorithms
- `api_endpoint` - HTTP/API handlers
- `utility` - Helper functions
- `test` - Test code

## Examples

### Example 1: Simple Function

**fibonacci.mint:**
```mint
λfibonacci(n:ℤ)→ℤ≡n{0→0|1→1|n→fibonacci(n-1)+fibonacci(n-2)}
```

**fibonacci.mint.map:**
```json
{
  "version": 1,
  "file": "fibonacci.mint",
  "generated_by": "claude-opus-4.6",
  "generated_at": "2026-02-21T10:00:00Z",
  "mappings": {
    "fibonacci": {
      "range": [0, 67],
      "summary": "Computes the nth Fibonacci number recursively",
      "explanation": "This function calculates Fibonacci numbers using the classic recursive approach. Base cases return 0 for n=0 and 1 for n=1. For other values, it sums the previous two Fibonacci numbers.\n\nThe Fibonacci sequence: 0, 1, 1, 2, 3, 5, 8, 13, ...",
      "type": "λ(ℤ)→ℤ",
      "complexity": "O(2^n) time, O(n) space",
      "warnings": [
        "Inefficient for large n due to exponential time complexity",
        "No memoization - recalculates same values repeatedly",
        "Consider iterative or memoized version for n > 30"
      ],
      "examples": [
        "fibonacci(0) = 0",
        "fibonacci(1) = 1",
        "fibonacci(5) = 5",
        "fibonacci(10) = 55"
      ],
      "related": ["factorial"],
      "metadata": {
        "pure": true,
        "recursive": true,
        "tail_recursive": false
      }
    },
    "match_arm_0": {
      "range": [22, 26],
      "summary": "Base case: F(0) = 0",
      "explanation": "When n is 0, return 0 as the first Fibonacci number"
    },
    "match_arm_1": {
      "range": [27, 31],
      "summary": "Base case: F(1) = 1",
      "explanation": "When n is 1, return 1 as the second Fibonacci number"
    },
    "match_arm_2": {
      "range": [32, 67],
      "summary": "Recursive case: F(n) = F(n-1) + F(n-2)",
      "explanation": "For any other n, compute by adding the two previous Fibonacci numbers. This creates a binary tree of recursive calls."
    }
  },
  "metadata": {
    "intent": "Classic recursive Fibonacci implementation for educational purposes",
    "category": "pure_function",
    "tested": true,
    "performance_profile": "exponential_time"
  }
}
```

### Example 2: HTTP Handler

**handler.mint:**
```mint
λhandle_request(req:Request)→Response!Error≡req.path{"/users"→get_users(req)|"/health"→Ok(Response{status:200,body:"OK"})|_→Err(Error{code:404,msg:"Not found"})}
```

**handler.mint.map:**
```json
{
  "version": 1,
  "file": "handler.mint",
  "generated_by": "claude-opus-4.6",
  "generated_at": "2026-02-21T10:00:00Z",
  "mappings": {
    "handle_request": {
      "range": [0, 170],
      "summary": "Routes HTTP requests to appropriate handlers",
      "explanation": "This function implements a simple HTTP router using pattern matching on the request path.\n\nSupported routes:\n- /users: Returns list of users (delegated to get_users)\n- /health: Returns 200 OK for health checks\n- Other paths: Returns 404 Not Found error",
      "type": "λ(Request)→Response!Error",
      "warnings": [
        "No authentication - all routes are public",
        "No rate limiting",
        "Error responses don't include CORS headers"
      ],
      "examples": [
        "handle_request({path: \"/users\", method: \"GET\"}) → get_users(...)",
        "handle_request({path: \"/health\", method: \"GET\"}) → Ok({status: 200, body: \"OK\"})",
        "handle_request({path: \"/unknown\", method: \"GET\"}) → Err({code: 404, msg: \"Not found\"})"
      ],
      "related": ["get_users", "Response", "Error"],
      "metadata": {
        "effects": ["IO", "Error"],
        "http_methods": ["GET"],
        "routes": ["/users", "/health"]
      }
    },
    "users_route": {
      "range": [45, 66],
      "summary": "Route: GET /users",
      "explanation": "When request path is /users, delegates to get_users function which fetches all users from the database"
    },
    "health_route": {
      "range": [67, 107],
      "summary": "Route: GET /health (health check)",
      "explanation": "Simple health check endpoint that always returns 200 OK. Used by load balancers and monitoring systems."
    },
    "not_found_route": {
      "range": [108, 170],
      "summary": "Catch-all: 404 Not Found",
      "explanation": "Default case for unrecognized routes. Returns HTTP 404 error with descriptive message."
    }
  },
  "metadata": {
    "intent": "HTTP request router for web server",
    "category": "api_endpoint",
    "tested": true,
    "performance_profile": "constant_time"
  }
}
```

### Example 3: Type Definition

**types.mint:**
```mint
t Option[T]=Some(T)|None
t Result[T,E]=Ok(T)|Err(E)
t User={id:ℤ,name:𝕊,email:𝕊,active:𝔹}
```

**types.mint.map:**
```json
{
  "version": 1,
  "file": "types.mint",
  "generated_by": "claude-opus-4.6",
  "generated_at": "2026-02-21T10:00:00Z",
  "mappings": {
    "Option": {
      "range": [0, 27],
      "summary": "Represents an optional value that may or may not exist",
      "explanation": "Option[T] is a sum type with two variants:\n- Some(T): Contains a value of type T\n- None: Represents absence of a value\n\nThis is Mint's safe alternative to null/undefined. All optional values must be explicitly handled.",
      "examples": [
        "Some(5) : Option[ℤ]",
        "None : Option[𝕊]",
        "≡maybe_value{Some(v)→v|None→0}"
      ],
      "related": ["Result"],
      "metadata": {
        "kind": "sum_type",
        "variants": ["Some", "None"]
      }
    },
    "Result": {
      "range": [28, 55],
      "summary": "Represents a computation that may succeed or fail",
      "explanation": "Result[T,E] is a sum type for error handling:\n- Ok(T): Successful result containing value of type T\n- Err(E): Error case containing error of type E\n\nMint uses Result instead of exceptions for explicit error handling.",
      "examples": [
        "Ok(42) : Result[ℤ,𝕊]",
        "Err(\"Division by zero\") : Result[ℤ,𝕊]",
        "≡result{Ok(v)→v|Err(e)→panic(e)}"
      ],
      "related": ["Option"],
      "metadata": {
        "kind": "sum_type",
        "variants": ["Ok", "Err"]
      }
    },
    "User": {
      "range": [56, 95],
      "summary": "Represents a user account",
      "explanation": "User is a product type (record) with fields:\n- id: Unique integer identifier\n- name: User's display name\n- email: User's email address\n- active: Whether the account is active",
      "examples": [
        "{id:1,name:\"Alice\",email:\"alice@example.com\",active:⊤}",
        "user.name (* Access field *)"
      ],
      "metadata": {
        "kind": "product_type",
        "fields": ["id", "name", "email", "active"]
      }
    }
  },
  "metadata": {
    "intent": "Core type definitions for the application",
    "category": "data_structure",
    "tested": false
  }
}
```

### Example 4: Complex Algorithm

**quicksort.mint:**
```mint
λquicksort[T](list:[T],cmp:λ(T,T)→𝔹)→[T]≡list{[]→[]|[p,.rest]→l smaller=filter(λx→cmp(x,p),rest);l greater=filter(λx→¬cmp(x,p),rest);quicksort(smaller,cmp)++[p]++quicksort(greater,cmp)}
```

**quicksort.mint.map:**
```json
{
  "version": 1,
  "file": "quicksort.mint",
  "generated_by": "claude-opus-4.6",
  "generated_at": "2026-02-21T10:00:00Z",
  "mappings": {
    "quicksort": {
      "range": [0, 200],
      "summary": "Sorts a list using the quicksort algorithm",
      "explanation": "Implements the classic quicksort algorithm:\n\n1. If list is empty, return empty list (base case)\n2. Choose first element as pivot\n3. Partition remaining elements into:\n   - smaller: elements where cmp(x, pivot) is true\n   - greater: elements where cmp(x, pivot) is false\n4. Recursively sort smaller and greater\n5. Concatenate: sorted(smaller) ++ [pivot] ++ sorted(greater)\n\nThe comparison function cmp determines sort order:\n- For ascending: λ(a,b)→a<b\n- For descending: λ(a,b)→a>b",
      "type": "∀T.λ([T],λ(T,T)→𝔹)→[T]",
      "complexity": "O(n log n) average, O(n²) worst case time; O(log n) space",
      "warnings": [
        "Worst case O(n²) when list is already sorted",
        "Not stable sort - equal elements may be reordered",
        "Not in-place - creates new lists (functional implementation)",
        "Consider using stdlib sort for production (uses introsort)"
      ],
      "examples": [
        "quicksort([3,1,4,1,5],λ(a,b)→a<b) = [1,1,3,4,5]",
        "quicksort([3,1,4,1,5],λ(a,b)→a>b) = [5,4,3,1,1]",
        "quicksort([\"zebra\",\"apple\",\"banana\"],λ(a,b)→a<b) = [\"apple\",\"banana\",\"zebra\"]"
      ],
      "related": ["filter", "mergesort"],
      "metadata": {
        "pure": true,
        "recursive": true,
        "algorithm": "divide_and_conquer",
        "stable": false
      }
    },
    "base_case": {
      "range": [50, 54],
      "summary": "Base case: empty list is already sorted",
      "explanation": "If input list is empty, return empty list"
    },
    "partition_step": {
      "range": [55, 150],
      "summary": "Partition list into smaller and greater elements",
      "explanation": "Choose first element (p) as pivot, then filter remaining elements:\n- smaller: elements where cmp(x,p) returns true\n- greater: elements where cmp(x,p) returns false\n\nThis effectively partitions the list around the pivot."
    },
    "combine_step": {
      "range": [151, 200],
      "summary": "Recursively sort and combine",
      "explanation": "Recursively sort smaller and greater partitions, then concatenate:\nsorted(smaller) ++ [pivot] ++ sorted(greater)\n\nThis is the divide-and-conquer recombination step."
    }
  },
  "metadata": {
    "intent": "Educational implementation of quicksort algorithm",
    "category": "algorithm",
    "tested": true,
    "performance_profile": "average_nlogn_worst_n2"
  }
}
```

## IDE Integration

### Hover Tooltip Format

When hovering over code in an IDE, show:

```
┌─────────────────────────────────────────┐
│ λfibonacci(n:ℤ)→ℤ                       │
├─────────────────────────────────────────┤
│ Computes the nth Fibonacci number       │
│ recursively                              │
│                                          │
│ Complexity: O(2^n) time, O(n) space     │
│                                          │
│ ⚠ Inefficient for large n              │
└─────────────────────────────────────────┘
```

### Detailed View Panel

When selecting code, show in side panel:

```markdown
# fibonacci

**Type**: λ(ℤ)→ℤ

## Summary
Computes the nth Fibonacci number recursively

## Explanation
This function calculates Fibonacci numbers using the classic
recursive approach. Base cases return 0 for n=0 and 1 for n=1.
For other values, it sums the previous two Fibonacci numbers.

## Complexity
- Time: O(2^n)
- Space: O(n)

## Warnings
- Inefficient for large n due to exponential time complexity
- No memoization - recalculates same values repeatedly
- Consider iterative or memoized version for n > 30

## Examples
```mint
fibonacci(0) = 0
fibonacci(5) = 5
fibonacci(10) = 55
```

## Related
- factorial
```

## Generation Guidelines

### For AI Map Generators

When generating semantic maps:

1. **Accuracy**: Ensure explanations match actual code behavior
2. **Clarity**: Write for developers unfamiliar with the code
3. **Completeness**: Cover all major code paths and edge cases
4. **Warnings**: Flag potential issues (performance, security, bugs)
5. **Examples**: Provide concrete usage examples
6. **Complexity**: Include Big-O analysis for algorithms
7. **Type Info**: Always include type signatures
8. **Related**: Link to related functions/types

### Quality Checklist

- [ ] Summary is clear and under 100 characters
- [ ] Explanation covers what, why, and how
- [ ] Type signature is accurate
- [ ] Complexity analysis included (if applicable)
- [ ] Warnings flag real issues
- [ ] Examples are runnable and correct
- [ ] Related functions are relevant
- [ ] Metadata is accurate

## Versioning

### Version 1 (Current)

Current schema as documented above.

### Future Versions

Potential additions:
- Version 2: Add `diagrams` field for visual explanations
- Version 3: Add `performance_metrics` from profiling
- Version 4: Add `test_coverage` data
- Version 5: Add `call_graph` for functions

## Validation

### JSON Schema

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["version", "file", "generated_by", "generated_at", "mappings"],
  "properties": {
    "version": {"type": "integer", "const": 1},
    "file": {"type": "string"},
    "generated_by": {"type": "string"},
    "generated_at": {"type": "string", "format": "date-time"},
    "mappings": {
      "type": "object",
      "additionalProperties": {
        "type": "object",
        "required": ["range", "summary", "explanation"],
        "properties": {
          "range": {
            "type": "array",
            "items": {"type": "integer"},
            "minItems": 2,
            "maxItems": 2
          },
          "summary": {"type": "string", "maxLength": 100},
          "explanation": {"type": "string"},
          "type": {"type": "string"},
          "complexity": {"type": "string"},
          "warnings": {
            "type": "array",
            "items": {"type": "string"}
          },
          "examples": {
            "type": "array",
            "items": {"type": "string"}
          },
          "related": {
            "type": "array",
            "items": {"type": "string"}
          },
          "metadata": {"type": "object"}
        }
      }
    },
    "metadata": {"type": "object"}
  }
}
```

## See Also

- [JavaScript Source Maps](https://sourcemaps.info/spec.html) - Inspiration for this format
- [LSP Specification](https://microsoft.github.io/language-server-protocol/) - IDE integration
- [Mint Grammar](grammar.ebnf) - Language syntax

---

**Next**: See `tools/mapgen/` for the semantic map generator implementation.
