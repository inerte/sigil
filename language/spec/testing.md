# Sigil Testing Framework Specification
## Test-Driven AI Development (TDAI)

Version: 1.0.0
Last Updated: 2026-02-21

## Overview

Sigil introduces **Test-Driven AI Development (TDAI)** - a paradigm shift where:
1. **Humans describe intent** in natural language
2. **AI generates comprehensive tests** from that intent
3. **Humans review test explanations** (via semantic maps)
4. **AI generates implementation** to pass the tests
5. **Tests validate correctness** automatically

**Key insight**: In a machine-first language where AI generates code, tests become the specification. Humans validate tests (easy), AI generates code (hard).

## Philosophy

**Traditional TDD:**
```
Human writes test (hard) → Human writes code (hard) → 2× human effort
```

**Sigil TDAI:**
```
Human writes intent (easy) → AI writes tests (fast) → Human reviews (easy)
→ AI writes code (fast) → Tests validate (automatic) → Human reviews results (easy)
```

**Result**: Humans do easy parts (intent, review), AI does hard parts (writing code + tests).

## Test Syntax

### First-Class Test Construct

Tests are a **first-class language construct**, not just function calls:

```sigil
test "description"{
  // Test body with assertions
}
```

**Grammar addition:**
```ebnf
TestDecl = "test" , StringLiteral , "{" , Expr , "}" ;
```

### Example

```sigil
// fibonacci.sigil
λfibonacci(n:ℤ)→ℤ match n{0→0|1→1|n→fibonacci(n-1)+fibonacci(n-2)}

// tests/fibonacci.test.sigil
test "fibonacci base cases"{
  assert_eq(0,fibonacci(0)) and 
  assert_eq(1,fibonacci(1))
}

test "fibonacci known values"{
  assert_eq(5,fibonacci(5)) and 
  assert_eq(55,fibonacci(10)) and 
  assert_eq(6765,fibonacci(20))
}

test "fibonacci properties"{
  property("recurrence",gen_int(2,100),λn→fibonacci(n)=fibonacci(n-1)+fibonacci(n-2)) and 
  property("monotonic",gen_int(0,100),λn→fibonacci(n+1)≥fibonacci(n))
}

test "fibonacci edge cases"{
  assert_err(fibonacci(-1)) and 
  assert_ok(fibonacci(0))
}
```

## File Organization

### Convention

```
project/
  src/
    fibonacci.sigil           # Implementation
    fibonacci.sigil.map       # Semantic map (AI explanations)
  tests/
    fibonacci.test.sigil      # Tests
    fibonacci.test.sigil.map  # Test explanations (what each test validates)
    fibonacci.spec.txt       # Natural language spec (optional)
```

### Auto-Discovery

```bash
sigilc test                   # Finds all .test.sigil files
sigilc test fibonacci         # Run tests for fibonacci module
sigilc test --watch           # Continuous testing
```

## Assertion API

### Standard Assertions

From `std/test` module (auto-imported in test files):

```sigil
// Equality
λassert_eq[T](expected:T,actual:T)→𝕌
λassert_ne[T](a:T,b:T)→𝕌

// Booleans
λassert(condition:𝔹,msg:𝕊)→𝕌
λassert_true(value:𝔹)→𝕌
λassert_false(value:𝔹)→𝕌

// Option/Result
λassert_ok[T,E](result:Result[T,E])→T
λassert_err[T,E](result:Result[T,E])→E
λassert_some[T](option:Option[T])→T
λassert_none[T](option:Option[T])→𝕌

// Collections
λassert_contains[T](item:T,list:[T])→𝕌
λassert_empty[T](list:[T])→𝕌
λassert_length[T](expected:ℤ,list:[T])→𝕌

// Numeric
λassert_approx(expected:ℝ,actual:ℝ,epsilon:ℝ)→𝕌
λassert_greater(a:ℤ,b:ℤ)→𝕌
λassert_less(a:ℤ,b:ℤ)→𝕌

// Panics/Exceptions
λassert_panics[T](fn:λ()→T)→𝕌
```

### Combining Assertions

Use `and` (logical AND) to combine assertions in a single test:

```sigil
test "user validation"{
  assert_eq("Alice",user.name) and
  assert_eq(25,user.age) and
  assert_true(user.active)
}
```

## Property-Based Testing

### Property Testing API

```sigil
// Property testing
λproperty[T](name:𝕊,gen:Generator[T],prop:λ(T)→𝔹)→𝕌!Test
λcheck_all[T](values:[T],prop:λ(T)→𝔹)→𝕌

// Built-in generators
λgen_int(min:ℤ,max:ℤ)→Generator[ℤ]
λgen_float(min:ℝ,max:ℝ)→Generator[ℝ]
λgen_bool()→Generator[𝔹]
λgen_string(min_len:ℤ,max_len:ℤ)→Generator[𝕊]
λgen_list[T](gen:Generator[T],min_len:ℤ,max_len:ℤ)→Generator[[T]]
λgen_option[T](gen:Generator[T])→Generator[Option[T]]
```

### Property Test Example

```sigil
test "list reverse properties"{
  property("reverse twice is identity",gen_list(gen_int(0,100),0,20),
    λlist→reverse(reverse(list))=list) and
  property("reverse length unchanged",gen_list(gen_int(0,100),0,20),
    λlist→length(reverse(list))=length(list)) and
  property("reverse empty is empty",gen_list(gen_int(0,100),0,0),
    λlist→reverse([])=[])
}
```

**Benefits**:
- Catches edge cases humans miss
- Mathematical specification of behavior
- AI can generate counterexamples
- Self-documenting tests

## Effect Mocking

### Mocking IO Effects

```sigil
// std/test/mock module
λwithMockIo[T](mocks:[IoMock],fn:λ()→T!IO)→T
λwithMockNetwork[T](mocks:[NetworkMock],fn:λ()→T!Network)→T
λwithMock[E,T](effect:Effect[E],mocks:[E],fn:λ()→T!E)→T
```

### Example

```sigil
test "read_file handles missing file"{
  withMockIo([
    file_not_found("/missing.txt")→Err(IoError{msg:"not found"})
  ]){
    assert_err(read_file("/missing.txt"))
  }
}

test "fetch_url handles timeout"{
  withMockNetwork([
    timeout("https://api.example.com/users")→Err(TimeoutError)
  ]){
    assert_err(fetch_url("https://api.example.com/users"))
  }
}
```

## Test Semantic Maps

### Test Explanations

Every `.test.sigil` file has a corresponding `.test.sigil.map` that explains what each test validates:

**fibonacci.test.sigil.map:**
```json
{
  "version": 1,
  "file": "fibonacci.test.sigil",
  "generated_by": "claude-opus-4.6",
  "generated_at": "2026-02-21T10:00:00Z",
  "mappings": {
    "test_fibonacci_base_cases": {
      "range": [0, 85],
      "summary": "Validates the foundation of Fibonacci sequence",
      "explanation": "Tests that F(0)=0 and F(1)=1, which are the base cases that all other Fibonacci numbers depend on. Without these, the entire sequence would be incorrect.",
      "assertions": [
        {
          "code": "assert_eq(0,fibonacci(0))",
          "explanation": "First Fibonacci number must be 0",
          "rationale": "Mathematical definition of Fibonacci sequence"
        },
        {
          "code": "assert_eq(1,fibonacci(1))",
          "explanation": "Second Fibonacci number must be 1",
          "rationale": "Mathematical definition of Fibonacci sequence"
        }
      ],
      "importance": "critical"
    },
    "test_fibonacci_known_values": {
      "range": [86, 200],
      "summary": "Validates correct computation for known Fibonacci numbers",
      "explanation": "Tests several known values in the Fibonacci sequence to ensure the recursive formula is implemented correctly.",
      "assertions": [
        {"input": 5, "expected": 5, "rationale": "Well-known value"},
        {"input": 10, "expected": 55, "rationale": "Larger value tests recursion depth"},
        {"input": 20, "expected": 6765, "rationale": "Tests performance and correctness"}
      ],
      "importance": "high"
    },
    "test_fibonacci_properties": {
      "range": [201, 400],
      "summary": "Property-based tests verify mathematical properties",
      "explanation": "Rather than testing specific values, these tests verify that the function obeys the fundamental properties of Fibonacci numbers for randomly generated inputs.",
      "properties": [
        {
          "name": "recurrence relation",
          "explanation": "For any n≥2, F(n) must equal F(n-1) + F(n-2)",
          "coverage": "Tests 100 random values from 2 to 100"
        },
        {
          "name": "monotonic",
          "explanation": "Fibonacci sequence always increases: F(n+1) ≥ F(n)",
          "coverage": "Tests 100 random values from 0 to 100"
        }
      ],
      "importance": "high"
    }
  },
  "coverage": {
    "total_tests": 4,
    "total_assertions": 8,
    "property_tests": 2,
    "edge_case_coverage": "negative, zero, positive, large values"
  },
  "metadata": {
    "intent": "Comprehensive validation of fibonacci function",
    "generated_from": "natural language request: 'I need a fibonacci function'"
  }
}
```

## TDAI Workflow

### Complete Workflow Example

```bash
# Step 1: Human describes intent
$ sigilc generate fibonacci --desc "computes nth fibonacci number"

AI: Generating comprehensive test suite...
    ✓ Created: tests/fibonacci.test.sigil (4 tests, 8 assertions, 2 properties)
    ✓ Created: tests/fibonacci.test.sigil.map (explanations)

# Step 2: Human reviews test explanations
$ sigilc review tests/fibonacci.test.sigil.map

[IDE shows formatted explanation]
📝 Test Suite: fibonacci
   ✓ Base cases (critical importance)
     - F(0) = 0
     - F(1) = 1
   ✓ Known values (high importance)
     - F(5) = 5
     - F(10) = 55
     - F(20) = 6765
   ✓ Properties (high importance)
     - Recurrence: F(n) = F(n-1) + F(n-2)
     - Monotonic: F(n+1) ≥ F(n)
   ✓ Edge cases (medium importance)
     - Negative numbers → error
     - Zero → valid

Coverage: 4 tests, 8 assertions, 200 property checks
Estimated execution time: <10ms

# Step 3: Human approves or requests changes
$ sigilc approve tests/fibonacci.test.sigil

AI: Tests approved. Generating implementation...
    ✓ Created: src/fibonacci.sigil
    ✓ Created: src/fibonacci.sigil.map
    Running tests...

    ✓ test fibonacci base cases (2/2 passed)
    ✓ test fibonacci known values (3/3 passed)
    ✓ test fibonacci properties (200/200 passed)
    ✓ test fibonacci edge cases (2/2 passed)

    All tests passing! Implementation complete.

# Step 4: Human reviews semantic map of implementation
$ sigilc review src/fibonacci.sigil.map

[Shows AI explanation of generated code]
λfibonacci(n:ℤ)→ℤ

Summary: "Computes nth Fibonacci number using recursive approach"
Explanation: "Pattern matching on n: F(0)=0, F(1)=1, else F(n)=F(n-1)+F(n-2)"
Complexity: "O(2^n) time, O(n) space"
Warnings: ["Inefficient for large n", "Consider memoization"]

# Step 5: Done! Or iterate...
$ sigilc optimize fibonacci --criteria "improve time complexity"

AI: Adding memoization...
    ✓ Updated: src/fibonacci.sigil (now O(n) time)
    ✓ Updated: tests/fibonacci.test.sigil (added performance tests)

    ✓ All tests still passing
    ✓ New performance test: fibonacci(100) completes in <1ms
```

## Test Runner

### CLI Commands

```bash
# Run all tests
sigilc test

# Run specific test file
sigilc test fibonacci

# Run with verbose output
sigilc test --verbose

# Run with coverage report
sigilc test --coverage

# Watch mode (re-run on file changes)
sigilc test --watch

# Generate tests from spec
sigilc test generate fibonacci --spec "computes nth fibonacci number"

# Generate implementation from tests
sigilc generate fibonacci --from-tests tests/fibonacci.test.sigil

# AI fixes failing tests
sigilc fix fibonacci --tests
```

### Test Output

**Passing tests:**
```
✓ test fibonacci base cases (2/2 assertions passed) 1ms
✓ test fibonacci known values (3/3 assertions passed) 2ms
✓ test fibonacci properties (200/200 property checks passed) 45ms
✓ test fibonacci edge cases (2/2 assertions passed) 1ms

Tests: 4 passed, 4 total
Time: 49ms
```

**Failing tests:**
```
✗ test fibonacci base cases (1/2 assertions passed) 1ms
  ✓ assert_eq(0,fibonacci(0))
  ✗ assert_eq(1,fibonacci(1))
    Expected: 1
    Actual: 0
    Location: tests/fibonacci.test.sigil:3:3

Tests: 0 passed, 1 failed, 1 total
Time: 1ms
```

**With AI explanations:**
```bash
$ sigilc test --explain-failures

✗ test fibonacci base cases

Failure: assert_eq(1,fibonacci(1))
  Expected: 1
  Actual: 0

AI Explanation:
  The fibonacci function is returning 0 for input 1, but it should return 1.
  This suggests the second base case (n=1→1) is either missing or incorrect.

  Looking at your code:
    λfibonacci(n:ℤ)→ℤ match n{0→0|n→fibonacci(n-1)+fibonacci(n-2)}

  The problem: You only have one base case (0→0). When n=1, it falls through
  to the recursive case, which calls fibonacci(0) + fibonacci(-1).

  Suggested fix:
    λfibonacci(n:ℤ)→ℤ match n{0→0|1→1|n→fibonacci(n-1)+fibonacci(n-2)}
                                ^^^^ Add this base case
```

## Integration Tests

### Natural Language Specs

For large integration tests, write specs in natural language:

**checkout.integration.spec.txt:**
```
E2E checkout flow:
1. User adds item to cart
2. User proceeds to checkout
3. User enters valid credit card
4. User confirms order
5. Order is placed successfully
6. User receives confirmation email
7. Inventory is decremented

Expected: Order created, email sent, inventory updated
Edge cases: Out of stock, invalid payment, network failure
```

AI generates comprehensive integration test suite from this spec.

### Integration Test Example

```sigil
test "E2E checkout flow - happy path"{
  withMockNetwork([
    payment_api("credit_card_123")→Ok(PaymentSuccess{transaction_id:"tx_456"})
  ]){
    withMockIo([
      send_email("user@example.com","Order confirmed")→Ok(())
    ]){
      l cart=create_cart()
      l cart2=add_to_cart(cart,{id:1,name:"Widget",price:10})
      l order_result=checkout(cart2,{card:"credit_card_123",email:"user@example.com"})

      assert_ok(order_result) and 
      assert_eq("tx_456",order_result.transaction_id) and 
      assert_eq(1,count_emails_sent()) and 
      assert_eq(9,get_inventory_count(1))  // Was 10, now 9
    }
  }
}

test "E2E checkout flow - out of stock"{
  l cart=create_cart()
  l cart2=add_to_cart(cart,{id:99,name:"Rare Item",price:1000})
  l order_result=checkout(cart2,{card:"card_123",email:"user@example.com"})

  assert_err(order_result) and 
  assert_contains("out of stock",order_result.error.msg)
}
```

## Benchmarking

### Performance Tests

```sigil
// std/test/bench module
λbenchmark(name:𝕊,fn:λ()→𝕌)→Duration
λassert_faster_than(max_duration:Duration,fn:λ()→𝕌)→𝕌
λassert_slower_than(min_duration:Duration,fn:λ()→𝕌)→𝕌
```

### Example

```sigil
test "fibonacci performance"{
  assert_faster_than(1ms,λ→fibonacci(10)) and 
  assert_faster_than(100ms,λ→fibonacci(20))
}

test "memoized fibonacci is faster"{
  l slow_time=benchmark("naive",λ→fibonacci_naive(30))
  l fast_time=benchmark("memoized",λ→fibonacci_memo(30))

  assert(fast_time<slow_time/10,"`memoized should be 10x faster")
}
```

## Test Coverage

### Coverage Analysis

```bash
$ sigilc test --coverage

Coverage Report:
  fibonacci.sigil
    Lines:      10/10 (100%)
    Branches:   3/3 (100%)
    Functions:  1/1 (100%)

  Untested code: None

Overall: 100% coverage
```

### Coverage Visualization

```
src/fibonacci.sigil:
  1: λfibonacci(n:ℤ)→ℤ match n{        ✓ Executed
  2:   0→0|                      ✓ Executed (2 times)
  3:   1→1|                      ✓ Executed (2 times)
  4:   n→fibonacci(n-1)+         ✓ Executed (15 times)
  5:     fibonacci(n-2)
  6: }
```

## Why TDAI is Revolutionary

### Traditional TDD
- Human writes test (hard)
- Human writes code (hard)
- **2× human effort**

### Sigil TDAI
- Human writes intent (easy)
- AI writes tests (fast, comprehensive)
- Human reviews tests (easy - semantic maps explain)
- AI writes code (fast)
- Tests validate (automatic)
- Human reviews results (easy)

**Result**: Humans do easy parts, AI does hard parts

### Benefits

1. **Better Coverage**: AI generates edge cases humans forget
2. **Faster Development**: AI writes both tests and code
3. **Higher Quality**: Tests are specifications, not afterthoughts
4. **Easier Review**: Reviewing tests easier than reviewing code
5. **Living Documentation**: Test semantic maps explain intent

## References

1. Beck, K. (2003). "Test-Driven Development: By Example"
2. Freeman, S., & Pryce, N. (2009). "Growing Object-Oriented Software, Guided by Tests"
3. Hughes, J. (2007). "QuickCheck: A Lightweight Tool for Random Testing of Haskell Programs"
4. Claessen, K., & Hughes, J. (2000). "QuickCheck: A Property-Based Testing Framework"

---

**Next**: See `tools/test-runner/` for test runner implementation.
