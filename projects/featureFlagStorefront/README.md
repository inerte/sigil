# featureFlagStorefront

Consumer app that reads feature flags from the publishable `featureFlagStorefrontFlags`
package and resolves live values from `config/<env>.lib.sigil`.

## Setup

Install local package dependencies first:

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- package install projects/featureFlagStorefront
```

Run the configured storefront:

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- run projects/featureFlagStorefront/src/main.sigil --env test
```

Run tests:

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- test projects/featureFlagStorefront/tests --env test
```
