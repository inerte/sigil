# Embedded Sigil Docs

Sigil ships a local docs corpus inside the `sigil` binary.

This is the language bootstrap surface for both humans and LLMs. A freshly
installed assistant should not be assumed to know Sigil syntax, stdlib modules,
or package rules from its model weights. Web search is also the wrong default
for a new language because ranking, indexing, and version matching all lag the
binary the user just installed.

`sigil docs ...` solves that problem by exposing the current guides, language
docs, specs, grammar, and design articles directly from the installed CLI.

## Discovery Loop

Start from the normal CLI help:

```bash
sigil help
```

Then move into the local docs surface:

```bash
sigil docs context --list
sigil docs context overview
sigil docs search "syntax reference"
sigil docs show docs/syntax-reference --start-line 1 --end-line 40
```

The intended loop is:

1. use `sigil help` to discover `docs`
2. use `sigil docs context --list` to discover the curated bundles
3. use `sigil docs context <id>` when the task is broad
4. use `sigil docs search <query>` when the task is specific
5. use `sigil docs show <docId>` to read the exact source material

## Commands

### `sigil docs list`

Returns the embedded corpus inventory.

Each document summary includes:

- `docId`
- `kind`
- `title`
- `path`
- `description`
- `lineCount`

This is the broadest starting point when an assistant needs to discover what is
available locally.

### `sigil docs search <query>`

Searches the embedded corpus and returns one hit per matching line.

Each hit includes:

- document identity and kind
- the nearest section heading when available
- the exact line number
- two lines before and two lines after the match
- whether the full phrase matched exactly

This is the right command when the model already has a concrete phrase in mind
such as `feature flags`, `records exactness`, or `stdlib`.

### `sigil docs show <docId>`

Returns one document with numbered lines.

Use the line-range flags to slice a smaller chunk:

```bash
sigil docs show docs/syntax-reference --start-line 350 --end-line 390
```

That lets an assistant search first, then read only the exact span it needs.

### `sigil docs context --list`

Lists the curated context bundles. Each bundle has:

- `id`
- `title`
- `description`
- `includedDocs`

These bundles are intentionally coarse. They are the best starting point when
the task is broad and the model does not yet know which exact document to open.

### `sigil docs context <id>`

Returns one resolved bundle with document references and summaries.

This command does not inline the full document contents. The model should use
`sigil docs show` for the specific documents it decides to read.

## `context` vs `search`

Use `context` when the question is broad:

- how do packages work?
- what should I read to understand topology?
- what is the minimum material for the type system?

Use `search` when the question is specific:

- where are feature flags defined?
- what does the grammar say about rooted modules?
- which document mentions `inspect world`?

## Output Model

The `sigil docs ...` commands return JSON by default.

The normal discovery help remains plain CLI help text:

- `sigil help`
- `sigil docs --help`

That split is intentional:

- help text bootstraps command discovery
- JSON payloads are the machine-first retrieval surface

## Why This Exists Even After Models Catch Up

Even if future models learn Sigil well enough to answer common questions from
memory, the embedded local corpus still matters:

- it is version-matched to the installed binary
- it avoids unnecessary web search
- it is faster and cheaper for repeated tool use
- it gives one first-party retrieval surface instead of multiple drifting sites

## Related References

- `language/spec/cli-json.md`
- `language/spec/cli-json.schema.json`
- `language/docs/syntax-reference.md`
- `website/articles/040-sigil-ships-embedded-docs-for-llm-cold-starts.md`
