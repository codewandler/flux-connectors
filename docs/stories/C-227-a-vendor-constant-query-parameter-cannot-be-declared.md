---
id: C-227
title: "C-55 gave the pipeline constant headers and constant body fields and stopped — a vendor-constant query parameter cannot be declared, and it costs Confluence every content read"
pillar: Spec
status: ready
priority: 2
design:
epic:
areas: [connector-spec, connector-flux]
note: "found by the C-219 implementor 2026-07-31. This is NOT the C-30 percent-encoding gap: a constant needs no encoding machinery, which is why it is the smaller change and the one that actually unblocks confluence's content reads"
---

# A vendor-constant query parameter cannot be declared

## Goal

Give the pipeline the third case [C-55](C-55-constant-request-headers.md) left out, so an operation
can send a query parameter the *vendor* fixes and the *caller* never chooses.

## What was measured

C-55 gave the pipeline vendor-constant **headers** and vendor-constant **body** fields. There is no
vendor-constant **query parameter**, and Confluence needs exactly one: `body-format=storage`.

Without it, `providers/confluence.toml` **cannot read any content body at all**. C-219 shipped it
honestly — as a connector that navigates and writes rather than reads back, with every read stating
that limitation in the description a model receives — and the missing parameter drove two of its
exclusions:

- `PUT /pages/{id}`, a read-modify-write whose read half is missing. Every update would blindly
  replace the page — the same data-loss reasoning that already excludes Jira's issue update.
- the footer-comment read, because a comment is *only* its body.

## Why this is not the query-encoding gap

The distinction is the whole point of filing this separately, and it will be mis-scoped otherwise.

[C-30](C-30-refuse-unencodable-query-values.md) and `docs/designs/query-encoding-flux-stories.md`
concern **caller-supplied** query values, which need a percent-encoder and a refusal path for values
that cannot be encoded safely. **A vendor constant needs none of that.** The value is authored in
the provider file, reviewed once, and never varies — so it requires no encoder, no refusal path and
no runtime validation.

That makes it the *smaller* change and the one that restores Confluence's content reads soonest.
Attaching it to the encoding work would block a cheap fix behind an expensive one.

## The workaround that was correctly refused

C-219 considered baking `?body-format=storage` into the operation's `path` and refused it: the day a
real query parameter lands beside it, the emitter produces

```
...?body-format=storage?limit=25
```

— two `?` separators and a malformed URL. It then made the refusal permanent rather than leaving it
as a note: `no_confluence_module_assembles_a_query_string` now rejects a `?` anywhere in a path. That
guard should survive this story, not be relaxed by it.

## Acceptance

- [ ] **Failing-first test:** an operation declares a vendor-constant query parameter and it reaches
      the emitted request. It cannot be declared today. Name the test.
- [ ] A constant query parameter is **not** a caller parameter: it must not appear in the operation's
      Flux signature, and a caller supplying it is refused. This is the same rule C-187 established
      for pinned values, and it should be spelled the same way rather than a second way.
- [ ] It composes correctly with a caller-supplied query parameter — one `?`, `&` between. The
      malformed-URL case above is the test to write first, because it is the one the path workaround
      would have produced.
- [ ] `providers/confluence.toml` adopts it, its content reads are restored, and the two exclusions
      it forced are revisited — `PUT /pages/{id}` in particular, whose exclusion rested entirely on
      the missing read half.
- [ ] The `?`-in-path guard `no_confluence_module_assembles_a_query_string` still passes. The fix
      must not reintroduce the shape it refuses.
- [ ] Read C-55 first and extend its vocabulary rather than inventing a parallel one. Three cases of
      one idea should read as three cases, not as three features.

## Notes

- The vendor behaviour behind this — Confluence returning an empty `body` when `body-format` is
  omitted — is community-corroborated but **absent from Atlassian's own documentation and OpenAPI**.
  C-219 recorded it in the provider header as undocumented rather than as a vendor guarantee, which
  is the right handling; the connector is correct either way, since it cannot send the parameter
  regardless. Do not upgrade that note to a citation without a source.
- Confluence is the motivating case but almost certainly not the only one. Before implementing, grep
  the shipped fleet for operations whose descriptions apologise for a missing query parameter — the
  count decides whether this is one connector's problem or the fleet's.
