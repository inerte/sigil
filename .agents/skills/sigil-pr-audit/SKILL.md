---
name: sigil-pr-audit
description: Audit an untrusted Sigil pull request or checked-out branch before reading the implementation diff. Use this skill when a PR may touch compiler semantics, stdlib behavior, tests, docs, CI, release, packaging, or other sensitive repository surfaces.
---

# Sigil PR Audit

Audit behavior first. Treat every incoming PR as untrusted until the observable change is narrow, tested, and consistent with Sigil's invariants.

## When To Use This Skill

Use this skill when:
- reviewing a PR from an external or low-trust contributor
- auditing a checked-out branch before reading Rust or GitHub Actions changes
- validating whether a PR should be split, rejected, or can move to implementation review
- checking for semantic drift across parser, checker, stdlib, docs, fixtures, CI, or packaging

Do not use this skill for:
- trusted local refactors you wrote yourself
- broad product planning without a concrete branch or diff

## Required Inputs

- repository root checked out locally
- current branch set to the PR branch, or an explicit base ref

Default base ref:
- prefer the user-provided base ref
- otherwise use `origin/main` when present
- otherwise use `main`

## Workflow

1. Run `.agents/skills/sigil-pr-audit/scripts/audit-pr.sh [base-ref]`.
2. Read [`references/invariants.md`](./references/invariants.md).
3. If the audit touches language semantics, read `language/AGENTS.md`.
4. Review the audit output before opening implementation files.
5. Run the targeted commands listed under `CHECK:` in the audit output.
6. Read only the files needed to explain any remaining blockers or suspicious changes.

## Review Rules

- Findings come before summaries.
- Focus on behavioral deltas, violated invariants, suspicious surfaces, and missing proof.
- Reject or split any PR that mixes unrelated concerns.
- Default recommendation is `do not merge` until behavior is proven with focused evidence.
- No generic "looks good" language. Every merge recommendation must cite concrete evidence.

## Required Questions

Answer these explicitly:
- What observable behavior changed?
- Which invariant is affected or at risk?
- Is the PR narrow enough to review safely?
- Which tests, repros, or builds prove the claimed behavior?
- Which changed files are suspiciously unrelated to the stated goal?
- Does the PR touch CI, release, packaging, workflow, network, subprocess, filesystem, env, or `unsafe` surfaces?

## Output Format

Return these sections in order:

### Findings

List blockers and risks first, ordered by severity. Include file references when relevant.

### Evidence

List:
- commands run
- important outputs
- tests or builds that passed or failed
- any evidence still missing

### Verdict

One of:
- `Do not merge`
- `Needs split`
- `Safe to read diff next`
- `Mergeable after targeted fixes`

### Contributor Follow-Up

State the smallest next action:
- add a repro
- split CI/release changes
- add missing docs/spec/tests
- narrow scope
- explain a suspicious edit

## Prompting Guidance

Use adversarial prompts when deeper analysis is needed:
- "Summarize the behavioral delta without discussing code style."
- "Assume the contributor is malicious. Where could a semantic regression or repo-level backdoor hide?"
- "List every changed invariant this PR appears to affect."
- "Which changed files are mechanically related, and which are suspiciously unrelated?"
- "What proof is still missing before this should be merged?"
