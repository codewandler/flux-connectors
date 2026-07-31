---
id: C-177
title: Ship the Contentful connector
pillar: Spec
status: ready
priority: 3
design:
epic: provider-fleet-2
areas: [providers]
note: "space AND environment are both path variables, and there are two distinct hosts (delivery vs management) with different credentials — the first provider whose SERVICES need different authorities and different secrets"
---

# Ship the Contentful connector

## Goal

Add Contentful to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**Two hosts, two credentials, one vendor.** Contentful's delivery API (`cdn.contentful.com`) and management API (`api.contentful.com`) are different authorities with different tokens. The service contract says a service owns its base URL; this is the first time it also needs to own its credential.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** Two bearer tokens, one per service: a delivery token and a management token.

**Curated operation set (a starting point, not a mandate):** delivery: get an entry, list entries, get an asset · management: create an entry, publish an entry

## Hazards specific to this one

Read the credential-addressing contract before writing: a credential path is `pid` + service, which suggests per-service credentials are *already* addressable — confirm that against `CredentialRef` rather than assuming. If it holds, this connector is the proof; if it does not, it is the finding. Do **not** pair this with an inbound surface (see [C-173](C-173-provider-typeform.md)'s note).

## Acceptance

- [ ] `providers/contentful.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents.
- [ ] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy.
- [ ] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
- [ ] A `verify` operation that is a read and runs unattended.
- [ ] `crates/connector-flux/tests/contentful_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses.
- [ ] **Failing-first test:** the contract test must fail before `providers/contentful.toml` exists.
- [ ] The scoped gate is green: `build --provider contentful`, `diff --provider contentful` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- [ ] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. They are red because you correctly did not write a coordinator-owned
      artifact. Report the eight; if the number differs, that is the finding.

## Notes

- **Charter fit.** Contentful is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
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
  `crates/catalog/src/generated/contentful.rs` is **not** in that set and is yours to commit.
