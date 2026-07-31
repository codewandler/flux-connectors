---
id: C-165
title: Ship the Trello connector
pillar: Spec
status: ready
priority: 3
design:
epic: provider-fleet-2
areas: [providers]
note: "PROBE: credential travels in the QUERY STRING (`?key=&token=`). C-159 measured ZERO Placement::Query in the shipped catalogue, and query values are not percent-encoded — the documented gap"
---

# Ship the Trello connector

## Goal

Add Trello to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**The first query-placed credential.** Trello authenticates with `?key=<key>&token=<token>`. C-159 measured the committed catalogue as 18 header placements and 2 inbound — no query placement ships today, and `AGENTS.md` records that query values are not percent-encoded.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** Two query parameters, `key` and `token`, both secret.

**Curated operation set (a starting point, not a mandate):** get a board, list a board's lists, list cards on a list, create a card, archive a card

## Hazards specific to this one

**Read [C-159](C-159-request-debug-and-query-encoding.md) §2 first.** It found that a query-placed credential does not travel as the string registered with the redactor, because `query_encode` escapes `+ / =` — so a base64-ish token can defeat redaction. That makes this connector the one that would make an unreachable bug reachable. Shipping may be the wrong answer; if so, record that, and say what C-159 has to close first.

## Acceptance

- [ ] `providers/trello.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents.
- [ ] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy.
- [ ] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
- [ ] A `verify` operation that is a read and runs unattended.
- [ ] `crates/connector-flux/tests/trello_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses.
- [ ] **Failing-first test:** the contract test must fail before `providers/trello.toml` exists.
- [ ] The scoped gate is green: `build --provider trello`, `diff --provider trello` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- [ ] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. They are red because you correctly did not write a coordinator-owned
      artifact. Report the eight; if the number differs, that is the finding.

## Notes

- **Charter fit.** Trello is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
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
  `crates/catalog/src/generated/trello.rs` is **not** in that set and is yours to commit.
