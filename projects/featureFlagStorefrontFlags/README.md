# featureFlagStorefrontFlags

Publishable internal package that defines shared storefront feature flags and
their typed value surface.

## Files

- `src/types.lib.sigil` defines the shared context and variant enums
- `src/flags.lib.sigil` defines the canonical `featureFlag` declarations
- `src/package.lib.sigil` exposes small context builders for consumers

## Validate

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- package validate projects/featureFlagStorefrontFlags
```
