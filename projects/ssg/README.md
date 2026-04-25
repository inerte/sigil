# Sigil Static Site Generator

`projects/ssg` is the canonical site builder for the repo. It renders the public Sigil website from markdown that already lives in the repo instead of copying docs into a second content tree.

## Source Of Truth

- `website/README.md` for the homepage
- `website/articles/*.md` for articles
- `website/projects/README.md` for the curated projects index
- `language/docs/*.md` for docs
- `language/spec/*.md` for spec pages
- `projects/*/README.md` for curated project detail pages and showcase content

## Build

```bash
pnpm sigil run projects/ssg/src/main.sigil
```

This produces HTML pages plus:

- `website/.local/site/feed.xml` for article feed consumers
- `website/.local/site/search-index.json` for client-side search consumers
- `website/.local/site/site.json` with build metadata
- `website/.local/site/sitemap.xml` for crawlers

Generated output goes to `website/.local/site/`.

## Validate

```bash
pnpm sigil test projects/ssg/tests
pnpm sigil run projects/repoAudit/src/main.sigil -- --check repo-compile
```
