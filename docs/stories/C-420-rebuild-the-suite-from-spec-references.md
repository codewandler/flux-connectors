---
id: C-420
title: "Rebuild the catalogue from spec references wherever a document exists"
pillar: Spec
status: backlog
priority: 3
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [providers, connector-cli]
note: "twenty-odd providers open with a comment explaining WHY THIS IS NOT A `[spec]` POINTER, each naming ingest as the blocker. Ingest landed 2026-08-01, so those comments are now claims about a gap that closed"
---

# Rebuild the catalogue from spec references wherever a document exists

## Goal
Turn the hand-authored suite into spec-referencing connectors wherever the vendor publishes a usable
document, so the catalogue is derived from upstream and drifts loudly instead of silently.

## Acceptance
- [ ] Every provider whose file opens with a `WHY THIS IS NOT A [spec] POINTER` comment is
      re-assessed, and each ends in one of exactly two states: converted to `[spec]` + patches, or
      carrying a **rewritten** comment that says why *now* — the old text names ingest (C-4) as the
      blocker and ingest has landed, so leaving it is a false statement about the code.
- [ ] Conversion is per provider and each is independently revertible; a provider that converts
      badly stays hand-authored with the reason recorded, and that is a success, not a failure.
- [ ] **Byte-identity is the default expectation and every departure is stated.** A conversion that
      changes `connectors/<name>.flux` records what changed and why in the provider file, the way
      C-416 does for babelforce.
- [ ] The declared-counts check (C-81) covers how many providers are spec-backed versus
      hand-authored, so the ratio is a measured fact rather than a claim in prose.
- [ ] Vendoring policy is stated once for the whole suite, not re-argued per provider: what may be
      committed, what a scrub must remove, and what stays out. C-415 wrote the babelforce paragraph;
      this generalizes it.

## Progress
- (not started)

## Notes
- **Sequenced behind [C-419](C-419-scaffold-a-provider-from-a-spec.md), deliberately.** Converting 53
  providers by hand is the same authoring cost this epic exists to remove; with the scaffold it is a
  run and a diff per provider.
- Providers that name a real published document in their opening comment — a partial list to work
  from, not the whole set: `box`, `cloudflare`, `datadog`, `asana`, `bitbucket`, `intercom`,
  `confluence`. Others (`contentful`, `airtable`, `figma`) record that **no** machine-readable
  document exists, and those stay hand-authored — the second front-end is not a failure mode, it is
  half the design (`connector-pipeline.md`, "Two front-ends, one IR").
- **The prize is drift, not line count.** A hand-authored connector cannot notice that a vendor
  changed a type; a spec-backed one can (C-14). That is vision principle 1 and it is unenforceable
  across a hand-authored suite.
