---
id: C-205
title: "The hand-maintained-data guard matches English prose, so the web gate is red on main"
pillar: Surfaces
status: in-progress
priority: 1
design:
epic:
areas: [web]
note: "found by the C-191 implementor 2026-07-31 and verified by the coordinator: `npm run build && npm test` is 27/28 on a clean main. Postmark's `server` service collides with the word `server` in a comment about the dev server — and 12 more service names are ordinary English words"
---

# The hand-maintained-data guard matches English prose, so the web gate is red on main

## Goal

Make `nothing about the catalogue is hand-maintained in the explorer sources` fail only on actual
hand-maintained catalogue data, so the web gate is green and stays green as connectors are added.

## What is broken, measured

On a clean checkout of `main` at v0.6.0:

```console
$ cd web && npm ci && npm run build && npm test
# tests 28
# pass 27
# fail 1
not ok 24 - nothing about the catalogue is hand-maintained in the explorer sources
  error: 'data/catalog.data.mts names `server` — catalogue data hand-written into
          the site is the failure this project exists to correct'
  location: 'web/test/explorer.test.mjs:1071:1'
```

The guard collects every **service name** in the catalogue into a forbidden-substring set and greps
the explorer sources with `String.includes`. Postmark declares a service called `server`. And
`web/data/catalog.data.mts:20` contains the ordinary English word, in a comment about the VitePress
dev server:

```ts
  // Resolved against this file's directory, and watched: editing the document reloads the dev
  // server. `cargo run -p connector-cli -- build` is what writes it.
```

The comment predates the collision — it dates from the original explorer build, and Postmark shipped
later. **Nothing is hand-maintained. The guard is matching prose.**

## Why this is worth a story rather than a one-line comment edit

Rewording that comment turns the gate green and leaves the defect. **Thirteen of the catalogue's
service names are ordinary English words**, measured from `web/public/catalog.json`:

```
account · admin · calendar · default · delivery · drive · files · gmail
machines · mail · management · models · server
```

`default`, `files`, `mail`, `models`, `account` and `management` are words that cannot be kept out of
English prose in a documentation site by discipline. Every one is a latent gate failure waiting for
the next comment, and the next author will have no idea why the build broke. The test's own comments
already concede this class for *event* names, and `C-158`'s notes record it for `drives` and
`notion` — but a service name is treated as unconditionally forbidden with no escape hatch.

The next connector with a one-word service is not a hypothetical: this is the second occurrence
already.

## Acceptance

- [x] **Failing-first test:** a case proving the guard tolerates a catalogue service name appearing
      as an English word in a comment, and still catches genuinely hand-written catalogue data.
      Both halves, or the fix is just a weakening.
- [x] `cd web && npm ci && npm run build && npm test` is **28/28** on a clean tree.
- [x] The matching is narrowed on a stated principle rather than by an exception list — token
      matching rather than substring, or a restricted set of sources the guard reads, or checking
      structure rather than text. An allowlist naming `server` is not a fix; it is this bug filed
      once per connector.
- [x] The guard still fails if someone genuinely hand-writes catalogue data into the explorer, proved
      by a test that does exactly that. **Do not weaken this into uselessness** — it is the guard that
      keeps the site honest about where its data comes from, and C-191's Acceptance depends on it.
- [x] The comment at `web/data/catalog.data.mts:20` is left as English, because the point is that
      prose is allowed.

## Notes

- **This blocks [C-191](C-191-publish-the-explorer-components.md)**, whose Acceptance item 3 requires
  this guard to pass, and it blocks any other web story whose gate is `npm test`.
- Adjacent, found in the same read: the guard matches comments as well as code, because it reads raw
  source text. That is the mechanism behind both this and the `drives`/`notion` cases.
- The dispatch order that follows from this: **C-205 → [C-158](C-158-typescript-catalogue-types-drift.md)
  → C-191.** All three write `web/test/explorer.test.mjs`, so they cannot share a wave slot.

## Progress

Done. The whole change is `web/test/explorer.test.mjs`; no site source was edited, and
`web/data/catalog.data.mts:20` still says "reloads the dev server".

**Two narrowings, both on stated principles, no allowlist.**

1. *A comment renders nothing, so a comment is not data.* `renderedSource(file, source)` returns
   what a source contributes to the built site — its code, markup and rendered text, with comments
   removed. The script scanner is string-aware rather than a `//.*$` sweep, because a hard-coded
   `https://api.postmark.com` is precisely what the guard exists to catch and a line sweep would cut
   it at its own `//`. Vue is read as three languages: `<script>`, `<style>`, template.
2. *A value is a word, not a fragment.* `namesValue(text, value)` requires word boundaries. This is
   what let the first narrowing land at all: `data/catalog.mts` declares the field `delivery_id`,
   which contains the service name `delivery`, and that is structure rather than data. It also
   settles the `gmail`/`mail` and `drives`/`drive` misreads the story records. The trailing boundary
   treats a capital as the end of a word, so a hand-coded `zendeskTicket` is still caught.

One source-set narrowing follows from principle 1: Markdown under `.vitepress/` is dropped, because
VitePress routes pages from the site root and builds no page from it — `theme/components/README.md`
is notes for whoever edits the components, the same category as a comment. Page Markdown
(`explorer.md`, `operations/[operation].md`) is still read in full.

Between them these clear all ten latent false positives in the tree, of which `server` was only the
first to fire. Nothing was added to the forbidden set's exceptions.

Proved both ways. `the guard still catches catalogue data hand-written into the explorer` plants a
real catalogue value in each language the sources are written in — a literal beside a comment, a
base URL with its `//`, template text, a bound attribute, a script literal, a style selector, page
prose — and requires each back. Verified end to end as well: appending
`export const FIRST_CONNECTOR = 'zendesk'` to `web/data/catalog.data.mts` turns the guard red with
``data/catalog.data.mts names `zendesk` ``.

Gate: `npm ci && npm run build && npm test` is 30/30 (28 before, plus the story's two new cases). No
Rust touched, so no Cargo gate was run.
