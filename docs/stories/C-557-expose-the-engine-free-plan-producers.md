---
id: C-557
title: "Expose the engine-free plan producers so a host derives a RequestPlan without flux"
pillar: Connector
status: ready
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

- [ ] **An engine-free endpoint resolver**: given a document `Operation` and a bound config port
      (the tenant's/operator's declared values — a trait the consumer implements, NOT a flux type),
      it returns the resolved `BTreeMap<String, String>` `resolve` expects, applying the SAME logic
      the live path applies today — declared defaults, `Approval::Operator`, `HttpsOrigin`
      normalisation, slot checks, declared-authority validation. Same code, relocated/exposed; not a
      second implementation.
- [ ] **An engine-free credential assembler**: given a document `Operation` and a bound secret port
      (resolve a `CredentialRef` address to a value — a trait, NOT a flux `ToolContext`), it returns
      `Vec<Assembled>` **and the redaction set as data** (the `SensitiveText` list `RequestPlan`
      already models), selecting the mechanism (source × acquisition × placement) and applying the
      prefix/base64/placement enforcement through `Assembled`/`place`/`placed_form` exactly as
      `Credentials::resolve` does today. The consumer registers the returned redactions with its own
      redactor; the library never touches a flux redactor on this path.
- [ ] Both producers live in an **engine-free** crate (`connector-resolve` or a new sibling) that
      links no `codewandler-flux-*`; a dependency-direction test pins it (the `dependency_fence.rs`
      pattern). Published metadata complete; it joins the derived publish closure.
- [ ] **The differential gate extends to the engine-free producers**: for every catalogued
      operation, the `RequestPlan` derived through the new engine-free producers is byte-identical to
      the one the flux `ToolContext` path (`build_request_plan`) produces — request, subjects,
      redaction set — failing-first against a seeded divergence. This is the evidence the enforcement
      was relocated, not reimplemented.
- [ ] `connector-pack`'s existing `ToolContext` producers become thin wrappers over the engine-free
      ones (or delegate to them), so connector-pack's behaviour is unchanged and its tests pass
      unmodified; nothing composes a request twice.
- [ ] `connector-secrets`' `SecretStore` port is a candidate for the secret port; if it is used,
      confirm the `connector-cli` offline fence (`no_network.rs`) still holds — the resolver crate
      must not become reachable from the compiler. If a new port trait is cleaner, define it in the
      engine-free crate and adapt.

## Progress

- 2026-08-12: Filed from flux-exchange X-156's Option-B finding, with its precise gap: connectors
  0.25 must expose the engine-free plan producers (endpoint resolver + credential assembler) taking
  bound ports rather than a flux `ToolContext`. X-156 half 1 (the exchange 0.24 adoption) is done on
  its branch and resumes against the release this story ships.

## Notes

- Write set spans `connector-resolve` (or a new crate) and `connector-pack` (delegating its
  producers). Collides with C-548/C-552/C-553's neighbourhood; runs solo.
- The invariant this protects, stated for the reviewer: the enforcement topology (credential
  resolution ordering, scheme placement, endpoint substitution with declared-authority validation)
  lives in the library and is relocated here, never duplicated — the differential gate over the two
  producers is the proof.
- Ships in connectors 0.25; X-156's engine-free half consumes it.
