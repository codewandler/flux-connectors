---
id: C-553
title: "Publish the plan seam Exchange's adoption actually needs"
pillar: Connector
status: done
priority: 1
design: docs/designs/catalog-artifact.md
epic: catalog-artifact
areas: [connector-pack, connector-resolve]
note: "Exchange's X-156 parked BLOCKED on this, with the gap read out of the vendored 0.23.0 sources: RequestPlan never crosses connector-pack's public boundary — Credentials::resolve and Configuration::snapshot are pub(crate), Egress::send is pub(crate), and build_authenticated_request drops permission_subjects and redactions. C-541's wrapper retirement is un-gated by this, not by exhortation"
---

# Publish the plan seam Exchange's adoption actually needs

## Goal

Let a consumer obtain a complete `RequestPlan` — request, `permission_subjects`, `redactions` —
for a live invocation, and hand a derived request to its bound transport, without reimplementing
credential resolution or endpoint substitution. Exchange's X-156 established by reading the
vendored 0.23.0 sources that this is impossible today: every input to
`connector_resolve::resolve` is produced only by `pub(crate)` paths in `connector-pack`
(`Credentials::resolve` at `src/credentials.rs:268`, `Configuration::snapshot` at
`src/config.rs:417` with the live per-variable resolution private in `tool.rs`), dispatch is
`pub(crate)` (`Egress::send`; the public `Egress::tool()` is exactly what consumers refuse), and
the one public plan-deriving function, `Operation::build_authenticated_request`, returns
`plan.request` and drops the subjects and redactions.

## Acceptance

- [x] A public `connector-pack` path yields the complete `RequestPlan` for a catalogued operation
      with the SAME enforcement topology the Tool path applies — credential resolution ordering,
      checked redactor registration, scheme placement, endpoint substitution with
      declared-authority validation, the live per-variable resolution (declared defaults,
      operator approval, origin normalisation). Not a parallel derivation: the same code, its
      result published instead of swallowed.
- [x] A public seam accepts a plan-derived request for dispatch through a bound `Egress` without
      exposing the transport (`Egress::tool()` stays refusable) — or the design records, with
      Exchange's agreement, that dispatch stays behind the Tool projection and only the plan is
      published; either way the decision is written, not defaulted.
- [x] The differential gate covers the new public path: the plan it yields is byte-identical to
      the wrapper-Tool derivation for every operation, subjects and redaction set included — the
      C-538 gate extended to the published seam, failing-first against a seeded divergence.
- [x] No secret-bearing value gains a printable path: `SensitiveText`/redacted-`Debug` discipline
      holds on everything newly public, pinned by test.
- [x] The consumer contract is documented on the crate (this is what Exchange's X-156 and
      upstream C-541's wrapper retirement both key on; name both in the doc).

## Progress

- 2026-08-12: Filed by the cross-repo coordinator from Exchange X-156's blocked findings.

- 2026-08-12: Implemented on `impl/C-553` (`0c4fa683` + review follow-up `7ccb8d09`), merged
  `635a652c`. `Operation::build_request_plan` publishes the complete `RequestPlan` through the
  same body `build_authenticated_request` already had (now one line over it), and `Egress::send`
  is public — the reviewer confirmed it is a strict subset of the already-public
  `tool().execute()`, and materially safer than the pre-existing public `build_authenticated_request`
  which dropped subjects and redactions. The differential gate's third arm holds the published
  plan byte-identical to the Flux derivation for all 835 operations with live per-field seeded
  controls. Independent review PASS, zero blocking; the C-136 diversion boundary is documented as
  a plan-path limit. Un-gates Exchange X-156 and upstream C-541.

## Notes

- Write set: `crates/connector-pack` (public surface), possibly `connector-resolve` re-exports,
  the differential gate. Collides with C-548 and C-552; do not share a wave with either.
- Ships in the next connectors release; Exchange's X-156 resumes against that release.
