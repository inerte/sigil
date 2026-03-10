---
title: Why Sigil Switched Primitive Types
date: 2026-03-10
author: Sigil Team
category: Language Design, Token Efficiency
slug: why-sigil-switched-primitive-types
---

# Why Sigil Switched Primitive Types

## Summary

Sigil now writes primitive types as:

- `Int`
- `Float`
- `Bool`
- `String`
- `Char`
- `Unit`
- `Never`

instead of the old Unicode glyphs:

- `ℤ`
- `ℝ`
- `𝔹`
- `𝕊`
- `ℂ`
- `𝕌`
- `∅`

This was not a style-only change.

On the tokenizer that matters most in this repo, OpenAI's `cl100k_base`, the old
Unicode primitive spellings were consistently more expensive than the new ASCII
forms. Lowercase and capitalized ASCII were effectively tied, so Sigil picked the
capitalized set because it gives the language a cleaner visual rule:

- types look like types
- values stay lowercase, like `true` and `false`

## What We Measured

The numbers below come from:

```bash
node language/benchmarks/tools/primitive-switch-benchmark.js
```

That script takes real Sigil files, rewrites primitive types back to the old
Unicode spellings in memory, and retokenizes both versions. The published numbers
here use `cl100k_base` as the official baseline.

## Example 1: Fibonacci

Before:

```sigil
λfib(n:ℤ)→ℤ match n{0→0|1→1|value→fib(value-1)+fib(value-2)}
λmain()→ℤ=fib(10)
```

After:

```sigil
λfib(n:Int)→Int match n{0→0|1→1|value→fib(value-1)+fib(value-2)}
λmain()→Int=fib(10)
```

Full-file token count:

- before: `44`
- after: `40`
- delta: `-4` tokens (`-9.1%`)

## Example 2: GCD

Before:

```sigil
λgcd(a:ℤ,b:ℤ)→ℤ match b{0→a|divisor→gcd(divisor,a%divisor)}
λmain()→ℤ=gcd(48,18)
```

After:

```sigil
λgcd(a:Int,b:Int)→Int match b{0→a|divisor→gcd(divisor,a%divisor)}
λmain()→Int=gcd(48,18)
```

Full-file token count:

- before: `45`
- after: `39`
- delta: `-6` tokens (`-13.3%`)

## Example 3: Palindrome Check

Before:

```sigil
λisPalindrome(s:𝕊)→𝔹=s=reverse(s)
λmain()→𝔹=isPalindrome("racecar")
λreverse(s:𝕊)→𝕊=s
```

After:

```sigil
λisPalindrome(s:String)→Bool=s=reverse(s)
λmain()→Bool=isPalindrome("racecar")
λreverse(s:String)→String=s
```

Full-file token count:

- before: `45`
- after: `33`
- delta: `-12` tokens (`-26.7%`)

This is the clearest case in the set. `𝔹` and `𝕊` were especially weak under
current tokenization.

## Example 4: Todo Domain

Before:

```sigil
t Todo={done:𝔹,id:ℤ,text:𝕊}

λaddTodo(id:ℤ,text:𝕊,todos:[Todo])→[Todo]=[Todo{done:false,id:id,text:text}]⧺todos

λcanAdd(text:𝕊)→𝔹=text≠""
```

After:

```sigil
t Todo={done:Bool,id:Int,text:String}

λaddTodo(id:Int,text:String,todos:[Todo])→[Todo]=[Todo{done:false,id:id,text:text}]⧺todos

λcanAdd(text:String)→Bool=text≠""
```

Full-file token count:

- before: `383`
- after: `337`
- delta: `-46` tokens (`-12.0%`)

This is the kind of example that matters more than toy expressions. The savings
compound when a real module repeats primitive types across records, signatures,
and helpers.

## Example 5: Option and Result Workflow

Before:

```sigil
λgetOrDefault(fallback:ℤ,res:Result[ℤ,𝕊])→ℤ match res{
  Ok(value)→value|
  Err(_)→fallback
}
```

After:

```sigil
λgetOrDefault(fallback:Int,res:Result[Int,String])→Int match res{
  Ok(value)→value|
  Err(_)→fallback
}
```

Full-file token count:

- before: `325`
- after: `294`
- delta: `-31` tokens (`-9.5%`)

## Why Not Lowercase?

Because lowercase did not buy us anything on token cost.

For the local tokenizer set used in this repo, `int` and `Int` were effectively
tied. Same for `bool` and `Bool`, `string` and `String`, and the rest.

That meant the real decision was about language shape. Sigil chose:

- capitalized primitive types
- lowercase boolean values

So the surface stays consistent:

- `Bool`
- `String`
- `User`
- `Todo`
- `true`
- `false`

## What This Change Means

Sigil is still a canonical language. It still wants one spelling for each thing.

But canonicality does not mean every unusual surface form is worth keeping
forever. When a dense spelling loses on real tokenizers, and a familiar spelling
wins without introducing ambiguity, the practical answer is to switch.

That is what happened here.
