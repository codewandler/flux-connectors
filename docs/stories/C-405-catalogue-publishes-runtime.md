---
id: C-405
title: "The catalogue publishes each connector's runtime"
pillar: Bridge
status: ready
priority: 5
note: "catalog::Provider has no runtime field, so a host cannot read how a connector executes — it has to derive it. That makes the multi-tenant refusal rule untestable against real catalogue data"
---

# The catalogue publishes each connector's runtime

## Goal

Publish how a connector executes — `http`, `socket`, `process`, `container`, `plugin`, `remote` — so a
host reads it rather than assuming it.

## Why

A host refuses a locally-executing runtime when it serves more than one tenant, because process,
container and raw-socket execution consume the host's own identity and network position. That refusal
is only mechanical if the runtime is a **declared fact** the host can read.

`catalog::Provider` carries no runtime field. Today every shipped connector is HTTP, so a consumer
derives `Http` and is right — and will keep being right until the first connector that is not, at
which point the derivation is silently wrong for exactly the case the refusal exists to catch.

Found while designing flux-exchange's invoke path, where the consequence is concrete: no shipped
connector exercises the refusal, so its test has to construct a fixture rather than use the catalogue.

## Acceptance

- [ ] The IR carries a connector's runtime, defaulting to `http` so no provider definition changes.
- [ ] It reaches **both** the manifest and `catalog.json`, and the Rust catalogue. A field that reaches
      the IR and stops there is the failure mode this repo has six of already.
- [ ] **Failing-first test** — a provider declaring a non-`http` runtime round-trips to the published
      catalogue, failing before the field exists.
- [ ] `cargo run -p connector-cli -- diff` stays clean, or every moved artifact is explained.
- [ ] The vocabulary matches flux's, and drift between the two is checked rather than promised — a
      mirrored closed set that nothing verifies stops being closed at the seam.

## Progress
- (not started)

## Notes
- Related: the `docs/designs/ecosystem.md` runtime axis in the flux repository, and flux-exchange's
  `Deployment::admits`, which is the consumer this unblocks.
