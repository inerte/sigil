# Homebrew Packaging

This Sigil project generates the canonical Homebrew formula for Sigil from GitHub Release metadata.

It does not talk to Homebrew APIs. It reads a release version plus `SHA256SUMS` and renders `Formula/sigil.rb` deterministically.

The generated formula is kept in this repo under `packaging/homebrew/Formula/sigil.rb` for reviewability, and the release workflow can publish the same file to `inerte/homebrew-tap`.
