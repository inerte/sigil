---
title: About This Site
---

# About This Site

This website is built from markdown that already lives in the repo. Docs stay in `language/docs`, the spec stays in `language/spec`, and articles stay in `website/articles`; the site generator renders those files in place instead of keeping a mirrored copy.

## Sections

- [articles/](./articles/) - Design articles documenting language evolution and decisions
- [language/docs/](../language/docs/) - Reference docs rendered into the site
- [language/spec/](../language/spec/) - Normative language and stdlib specs rendered into the site

## Build

```bash
cargo run -q -p sigil-cli --manifest-path language/compiler/Cargo.toml -- run projects/ssg/src/main.sigil
```

Generated output goes to `website/.local/site/`.

## Homepage

The built homepage also includes generated article, docs, and spec indexes so the rendered site stays in sync with repo content automatically.
