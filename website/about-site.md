---
title: About This Site
---

# About This Site

This website is built from markdown that already lives in the repo. Docs stay in `language/docs`, the spec stays in `language/spec`, articles stay in `website/articles`, and curated project pages render from `projects/*/README.md`; the site generator renders those files in place instead of keeping a mirrored copy.

The static site generator is <a href="https://github.com/inerte/sigil/tree/main/projects/ssg">built in Sigil itself</a>, demonstrating the language's capabilities for real-world applications.

## Sections

- <a href="/articles/">Articles</a> - Design articles documenting language evolution and decisions
- <a href="/docs/">Docs</a> - Reference docs rendered from `language/docs`
- <a href="/projects/">Projects</a> - Curated real programs built with Sigil, including browser-runnable demos when available
- <a href="/spec/">Spec</a> - Normative language and stdlib specs rendered from `language/spec`

## Build

```bash
cargo run -q -p sigil-cli --no-default-features -- run projects/ssg/src/main.sigil
```

Generated output goes to `website/.local/site/`.

## Homepage

The built homepage includes generated project, article, docs, and spec indexes so the rendered site stays in sync with repo content automatically.
