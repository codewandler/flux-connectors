---
id: C-557
title: "Expose the engine-free plan producers so a host derives a RequestPlan without flux"
pillar: Connector
status: in-progress
priority: 0
design: docs/designs/catalog-artifact.md
epic: catalog-artifact
areas: [connector-resolve, connector-pack]
note: "Exchange X-156 (engine-free) is blocked: connector_resolve::resolve consumes an already-resolved endpoints map and already-Assembled credentials, but nothing engine-free PRODUCES them — endpoint resolution is pub(crate)/private in the flux-coupled connector-pack, credential assembly (Credentials::resolve) is pub(crate) and takes a flux ToolContext, and build_request_plan takes a ToolContext too. C-538/C-553 published the plan CONSUMER; this publishes the PRODUCERS."
---

# Expose the engine-free plan producers so a host derives a RequestPlan without flux

## Goal

A consumer (flux-exchange, X-156) can derive a complete `RequestPlan` for a catalogued operation
**engine-free** — without depending on `connector-pack` and without a flux `ToolContext` — so it can
drop connector-pack from its invoke path, escape cargo's flux-version unification, and retire its
`ENGINE_LINE` lockstep. C-538 moved the plan *derivation* to the engine-free `connector-resolve`
(`resolve(operation, base_url, params, endpoints, credentials)`); C-553 published a full-plan seam
on connector-pack. Neither exposed the **producers** of `resolve`'s two data inputs, so the
engine-free path dead-ends: `connector-resolve` has no config port, no secret port and no mechanism
selection, and the resolution that fills those inputs is `pub(crate)`/private in the flux-coupled
connector-pack (`Configuration::snapshot`, `Operation::endpoints`/`endpoint`, `Credentials::resolve`
taking a `&ToolContext`). This story closes that, keeping the enforcement topology **in the library**
— the consumer must never reimplement it.

## Acceptance

- [x] **An engine-free endpoint resolver**: `connector_resolve::resolve_endpoints`/`resolve_endpoint`
      over the new `ConfigPort` trait return the resolved `BTreeMap<String, String>` `resolve` expects.
      The declared-default, `Approval::Operator`, `HttpsOrigin` normalisation and declared-default
      detection moved from `connector-pack`'s `Configuration::snapshot`/`Operation::endpoint` into
      `crates/connector-resolve/src/endpoints.rs`; `connector-pack`'s `endpoint()` delegates. Slot
      checks and declared-authority validation stay in `resolve`/`build_request` where they always ran.
- [x] **An engine-free credential assembler**: `connector_resolve::assemble_credentials` over
      `connector_secrets::SecretStore` (the secret port) + the `ConfigPort` (Basic user half) returns
      `Assembly { credentials: Vec<Assembled>, redactions: Vec<Redaction> }`. The mechanism selection,
      acquisition axis and C-159 redaction-form computation moved from `Credentials::resolve_mechanism`
      into `crates/connector-resolve/src/credentials.rs`; it touches no redactor. `connector-pack`'s
      `Credentials::resolve` delegates and registers the returned redactions with the flux redactor.
- [x] Both producers live in `connector-resolve`, which still links no `codewandler-flux-*`
      (`engine_free_core::the_plan_deriving_core_links_no_engine_crate` green). It is already in the
      derived publish closure; `publish_closure.rs` recomputed the order (secrets now before resolve)
      and passes.
- [x] **The differential gate extends to the engine-free producers**: `catalogue_differential.rs`
      gained a fourth arm and `engine_free_compared == byte_compared`; the plan the bare
      `ConfigPort`/`SecretStore` producers derive is byte-identical (request, subjects, redaction set)
      to the flux-derived one for every operation. Control `a_seeded_divergence_in_the_engine_free_producers_is_caught`.
- [x] `connector-pack`'s producers are thin wrappers now: all 124 integration + 88 lib tests pass
      unmodified except the mapped-refusals pin (extended for the relocated variants) and the
      differential gate (the new arm). Nothing composes a request twice — the one derivation is
      `resolve`, which both `connector-pack` and a bare consumer feed.
- [x] Used `connector-secrets`' `SecretStore` as the secret port. Offline fence holds:
      `dependency_fence::connector_cli_does_not_depend_on_connector_secrets` and `no_network::*` green —
      `connector-cli` reaches neither `connector-resolve` nor, through it, `connector-secrets`.

## Progress

- 2026-08-12: Filed from flux-exchange X-156's Option-B finding, with its precise gap: connectors
  0.25 must expose the engine-free plan producers (endpoint resolver + credential assembler) taking
  bound ports rather than a flux `ToolContext`. X-156 half 1 (the exchange 0.24 adoption) is done on
  its branch and resumes against the release this story ships.
- 2026-08-13: Implemented on `impl/C-557`. Producers host: `connector-resolve` (extended). Two ports:
  the new `connector_resolve::ConfigPort` trait (config) and `connector_secrets::SecretStore`
  (secret). New public items: `ConfigField`, `ConfigValue`, `ConfigPort`, `resolve_endpoint`,
  `resolve_endpoints`, `assemble_credentials`, `Assembly`, `Redaction`, plus ten relocated
  `connector_resolve::Error` variants (`MissingConfig`, `UnapprovedConfig`, `UnsafeOrigin`,
  `MissingCredential`, `CredentialStore`, `NoCredentialAddress`, `CredentialAddress`,
  `UndeclaredCredential`, `MissingCredentialConfig`, `EmptyMechanism`) — none carries a credential
  value, and the credential-derived redaction forms are guarded behind `SensitiveText`. New edge
  `connector-resolve -> connector-secrets`; the derived publish order becomes
  `connector-address, catalog-reader, connector-catalog, connector-secrets, connector-resolve,
  connector-pack` (AGENTS.md's publishing-contract prose still lists the old order — coordinator-owned,
  left untouched). Base proof: the extended gate did not compile at `4d371101` because the producers
  were absent; the whole catalogue then agrees byte-for-byte through them.

## Notes

- Write set spans `connector-resolve` (or a new crate) and `connector-pack` (delegating its
  producers). Collides with C-548/C-552/C-553's neighbourhood; runs solo.
- The invariant this protects, stated for the reviewer: the enforcement topology (credential
  resolution ordering, scheme placement, endpoint substitution with declared-authority validation)
  lives in the library and is relocated here, never duplicated — the differential gate over the two
  producers is the proof.
- Ships in connectors 0.25; X-156's engine-free half consumes it.
