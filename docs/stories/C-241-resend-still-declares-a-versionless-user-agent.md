---
id: C-241
title: "Four Resend operations still send a bare versionless `User-Agent`, overriding the versioned identity C-223 gave every other connector"
pillar: Spec
status: ready
priority: 3
design: docs/designs/host-identity.md
epic:
areas: [providers, bridge]
note: "left behind by C-223 deliberately and flagged by its review: removing the declaration needs `build --provider resend` plus whole-catalogue regeneration, which is coordinator-owned, and C-223's gate permitted zero red tests"
---

# Resend still declares a versionless `User-Agent`

## Goal

Let Resend inherit the versioned identity every other connector now sends, and remove the workaround
that predates it.

## What was measured

[C-223](C-223-the-host-sends-no-user-agent.md) made `connector-pack` send

```
User-Agent: flux-connectors/0.7.0 (+https://github.com/codewandler/flux-connectors)
```

on every request, with a connector's own declaration winning where it has one. Its review dumped all
299 shipped operations before and after: **295 gained exactly one header; 4 were unchanged.** Those
four are Resend's, because `providers/resend.toml:139` declares

```toml
const_headers = { "User-Agent" = "flux-connectors" }
```

That declaration was correct when it was written — it was the only way to satisfy a vendor that
refuses a request without one. It is now the *worse* of the two values available:

- it carries **no version**, so a vendor cannot tell 0.7.0 from a release two years from now;
- it is the bare product word C-223's own acceptance rules out — *"a `User-Agent` that lies is worse
  than one that is absent"* applies to one that says nothing, too;
- it is a per-connector workaround for a gap the host has since closed centrally, which is the shape
  C-214 is an instance of: one rule spelled in two places.

## Why C-223 did not remove it

Recorded rather than left to be rediscovered: removing the declaration changes a provider file, which
requires `cargo run -p connector-cli -- build --provider resend` and then leaves whole-catalogue
artifacts stale. Those are **coordinator-owned** (`AGENTS.md`, "Whole-catalogue artifacts are
coordinator-owned"), and C-223's gate permitted **zero** red tests. Doing it there would have meant
either a red gate or an implementor regenerating an artifact that is not theirs to write.

So this is a small story that must ride a wave which owns the catalogue regeneration — not a defect
in C-223.

## Acceptance

- [ ] **Failing-first test:** a Resend operation carries the versioned identity. It carries the bare
      word today. Name it.
- [ ] `const_headers` is removed from `providers/resend.toml`, and the header comment explaining why
      Resend needed one is rewritten to say the host now supplies it — not deleted, because the
      *vendor's* requirement is still a real fact worth recording next to the connector it affects.
- [ ] The **exactly-one-`User-Agent`** property still holds for Resend — the catalogue-wide check
      C-223 added must stay green, and it is what proves the removal did not leave the operation with
      none.
- [ ] `build --provider resend` and `diff --provider resend` are clean, and the whole-catalogue
      staleness failures this leaves are reported and resolved by the coordinator's full build rather
      than silenced.

## Notes

- **Check whether any other connector has since declared one.** At the time C-223 landed, Resend was
  the sole shipped `User-Agent` declaration — and `providers/github.toml` declares only `Accept`,
  correcting a claim in C-223's own story text. If a second has appeared, it belongs in this story.
- The vendor fact is the durable half: Resend rejects a request with no `User-Agent` and answers
  `403`, which is why this connector was the one that surfaced the gap at all. Losing that note would
  cost more than the workaround does.
- Cheap to do, and it should ride the next wave that already regenerates the catalogue rather than
  earning a regeneration of its own.
