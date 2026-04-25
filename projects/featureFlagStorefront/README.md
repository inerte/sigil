# featureFlagStorefront

Consumer app that reads feature flags from the publishable `featureFlagStorefrontFlags`
package and resolves live values from `config/<env>.lib.sigil`.

## Setup

Install local package dependencies first:

```bash
cargo run -q -p sigil-cli --no-default-features -- package install projects/featureFlagStorefront
```

Run the configured storefront:

```bash
cargo run -q -p sigil-cli --no-default-features -- run projects/featureFlagStorefront/src/main.sigil --env test
```

Run tests:

```bash
cargo run -q -p sigil-cli --no-default-features -- test projects/featureFlagStorefront/tests --env test
```
