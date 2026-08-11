---
id: C-532
title: "Internal infrastructure markers leave the public repository"
pillar: Surfaces
status: done
priority: 0
areas: [docs, connector-address, connector-secrets, tests]
note: "docs/designs/spec-front-end.md quoted the internal forge host and the repository paths that its own leak-marker regex names as strings which must never be published"
---

# Internal infrastructure markers leave the public repository

## Goal

Stop this public repository from carrying the vendor's internal forge host and internal repository
paths, which its own documentation identifies as high-confidence leak markers.

## The finding

`docs/designs/spec-front-end.md` §"Vendoring and provenance" argues — correctly — that the vendored
specs are public while the *fetch configuration* is internal, and that
`manager-sdk/scripts/leak-markers.regex` names certain strings as markers "that must never be
published". It then quoted those strings, in a public repository, in the paragraph making the
argument.

Eleven occurrences across eight files, found with
`git grep -n -E "sbf/[a-z-]+|gitlab\.stack"`: the internal forge hostname once, the internal
spec-repository path once, the internal services-repository path once, and the internal secret
store's path eight times as an architectural precedent in credential-addressing prose and tests.

## Acceptance

- [x] No internal forge hostname and no internal repository path remains in the tree. Verified with
      the same `git grep` that found them, which now returns nothing.
- [x] Every argument that cited one is preserved. The secret store remains the stated precedent for
      the `tenants/` prefix — it is described rather than named, because the argument depends on
      *what it did*, never on what it is called.
- [x] `spec-front-end.md` says why it describes rather than quotes, so the next author does not
      helpfully restore the literals to make the paragraph more concrete.
- [x] Generated artifacts unmoved: `diff` reports 1110 artifacts up to date (55 providers checked).
- [x] `credential_paths` and the affected crates are green.

## Progress

- 2026-08-12: Found while scrubbing a hostname this session had itself introduced into a test corpus
  (C-523), and closed in the same release.

## Notes

**This does not unpublish anything.** The hostname entered on 2026-08-01 in `e5cf0523` and is inside
the released `v0.20.0` tag, so it is already public and reachable from history. This stops it
spreading to every future release and to the crates.io copies of `connector-address` and
`connector-secrets`; removing it from history is a separate decision about rewriting a pushed,
tagged, published repository, and is deliberately not taken here.

There is precedent for acting rather than deferring: `66c1261a` scrubbed a named individual's address
out of the vendored documents, reasoning that "repository history makes it expensive to undo after a
push, which is why it happens now".

**What was deliberately left.** `services.babelforce.com` and `x-babelforce-customer-id` are the
vendor's *public* API host and a documented request header — the same category as `api.github.com` —
and `providers/babelforce.toml` is a shipped connector. Those are vendor facts, not infrastructure.
