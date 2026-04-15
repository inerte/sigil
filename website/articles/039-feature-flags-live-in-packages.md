---
title: Feature Flags Live in Packages, Values Live in Config
date: 2026-04-12
author: Sigil Language Team
slug: feature-flags-live-in-packages
---

# Feature Flags Live in Packages, Values Live in Config

Sigil now has first-class `featureFlag` declarations.

That raises a design question immediately: where should those declarations live?

The answer is that the declaration and the current value are different things.

The declaration is a shared typed contract. It says that a flag exists, what type it returns, when it was introduced, and what value is safe to use as the fallback.

The current value is environment-specific runtime policy. It says which contexts match special rules, which matching contexts should get a fixed value immediately, and how a percentage rollout should behave in `test` or `prod`.

Those two concerns should not be stored in the same place.

## The Declaration Belongs in `src/flags.lib.sigil`

Projects and publishable packages now have a canonical home for feature flags:

```text
src/flags.lib.sigil
```

That file may contain only `featureFlag` declarations.

The declaration shape is intentionally small:

```sigil module projects/featureFlagStorefrontFlags/src/flags.lib.sigil
featureFlag NewCheckout:Bool
  createdAt "2026-04-12T14-00-00Z"
  default false
```

Variant-valued flags use named sum types:

```sigil module
t CheckoutColor=Citrus()|Control()|Ocean()

featureFlag CheckoutColorChoice:CheckoutColor
  createdAt "2026-04-12T14-00-00Z"
  default Control()
```

The important part is that these declarations are typed values, not string keys. Application code and configuration refer to them directly.

## The Value Belongs in `config/<env>.lib.sigil`

The live value surface is not part of the declaration.

Instead, the selected environment exports a `flags` value that application code reads through `•config.flags`.

That keeps the split explicit:

- `src/flags.lib.sigil` defines which flags exist
- `config/<env>.lib.sigil` defines the current ordered rules and rollout actions

The canonical evaluation surface is `§featureFlags`.

For example:

```sigil expr
§featureFlags.get(
  context,
  ☴featureFlagStorefrontFlags::flags.NewCheckout,
  •config.flags
)
```

That third argument is a typed `§featureFlags.Set[...]`, not an ad hoc map of strings to booleans.

## Packages Are the Right Home for Shared Flag Contracts

This is why packages needed to land before feature flags.

A company usually wants one place that defines:

- the canonical flag names
- the value types for multi-variant flags
- the shared context types used by consumers

That is exactly what a small internal package is good at.

The new `projects/featureFlagStorefrontFlags` package demonstrates that shape:

- `src/types.lib.sigil` defines `FlagContext`, `Site`, and the variant enums
- `src/flags.lib.sigil` defines the canonical flag declarations
- consumers import those flags through nested public package paths like
  `☴featureFlagStorefrontFlags::flags.NewCheckout`

This avoids a common failure mode in existing flag systems: every service has its own string literal for the same flag, and those literals silently drift.

In Sigil, the package owns the typed contract once.

## Deterministic Rollout Stays in the Evaluation Engine

The declaration itself does not describe rollout policy.

That policy lives in the environment-selected config set passed to
`§featureFlags.get`.

Current evaluation order is:

1. first matching rule wins
2. `Value(...)` returns immediately
3. `Rollout(...)` deterministically buckets for that key
4. declaration default

This keeps the shared contract stable while letting each environment choose its own current behavior.

The example storefront app uses all four patterns:

- a first-match user-specific rule for `"dev-user"`
- an internal-user rule
- a Brazil-specific rule
- deterministic percentage rollout by `userId`

That is enough to model a moderately realistic product rollout without turning the language into a full remote flag-service design.

## Why `createdAt` Exists

Feature flags create cleanup debt.

Sigil now requires `createdAt` on every `featureFlag` declaration. That does not try to predict the future with an `expires` date. It records when the debt started.

Lifecycle queries belong in the CLI:

```text
sigil featureFlag audit
sigil featureFlag audit --older-than 180d
```

The compiler still enforces the hard invariants:

- `createdAt` is required
- `default` is required
- `default` must match the declared type
- the canonical project/package home is `src/flags.lib.sigil`

So the language owns validity, while the CLI owns lifecycle queries.

## What This Does Not Try to Solve Yet

This first pass is intentionally narrow.

It does not yet add:

- remote flag services as a first-class language surface
- owner metadata
- expiry metadata
- unused-flag analysis
- a separate entitlement system

Those may matter later, but they are not required for the core design:

- declarations are typed
- values are environment-selected
- rollout stays deterministic
- packages can distribute a shared flag contract

That is enough to make flags feel like part of the language instead of stringly typed infrastructure bolted on at the edges.
