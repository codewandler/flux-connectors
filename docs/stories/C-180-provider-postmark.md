---
id: C-180
title: Ship the Postmark connector
pillar: Spec
status: ready
priority: 3
design:
epic: provider-fleet-2
areas: [providers]
note: "`X-Postmark-Server-Token`, and the server token is per-SERVER while the account token is a different header for a different endpoint set — two credentials scoped to different operation subsets"
---

# Ship the Postmark connector

## Goal

Add Postmark to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**Two credentials, partitioned by operation rather than sent together.** Postmark uses `X-Postmark-Server-Token` for sending and `X-Postmark-Account-Token` for account management. Unlike Datadog they are never sent together — they partition the surface, which is what a *service* is for.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** `X-Postmark-Server-Token` (default service) and `X-Postmark-Account-Token` (account service).

**Curated operation set (a starting point, not a mandate):** send an email, get delivery stats, list bounces, get a bounce · account: list servers, get a server

## Hazards specific to this one

This is the clean version of what [C-177](C-177-provider-contentful.md) probes: per-service credentials where the *services* already justify the split. If the credential model is per-service, this connector should be straightforward and is worth landing before Contentful. **Author no recipient addresses.**

## Acceptance

- [ ] `providers/postmark.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents.
- [ ] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy.
- [ ] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
- [ ] A `verify` operation that is a read and runs unattended.
- [ ] `crates/connector-flux/tests/postmark_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses.
- [ ] **Failing-first test:** the contract test must fail before `providers/postmark.toml` exists.
- [ ] The scoped gate is green: `build --provider postmark`, `diff --provider postmark` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- [ ] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. They are red because you correctly did not write a coordinator-owned
      artifact. Report the eight; if the number differs, that is the finding.

## Notes

- **Charter fit.** Postmark is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
  non-goals exclude technology adapters and the inference path. If authoring reveals it is really a
  technology rather than a service, stop and say so rather than shipping it.
- **Provenance is hand-authored and drift is undetectable by machine**, the caveat zendesk, freshdesk,
  github and notion already carry. Record it in the TOML's header comment.
- **Do not invent an endpoint.** A confident set of four operations beats a guessed set of ten — the
  repository's rule is that a loud refusal beats plausible-but-incorrect output, and a hallucinated
  path ships as a `404` a contract test cannot catch. Where you are unsure of a path, shape or required
  field, leave the operation out and name it in `## Progress` as unverified.
- **No credential value, and no realistic-looking `example` on a secret field.** A placeholder shaped
  like a real token has already tripped GitHub's push protection and blocked a release.
- Whole-catalogue artifacts are coordinator-owned: `crates/catalog/src/generated.rs`,
  `web/public/catalog.json`, `web/public/v1/**`, `assets/readme-snippet-*.svg`. The per-provider
  `crates/catalog/src/generated/postmark.rs` is **not** in that set and is yours to commit.
