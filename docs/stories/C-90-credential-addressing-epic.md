---
id: C-90
title: "Credential addressing and the secret-store seam (epic)"
pillar: Spec
status: backlog
design: docs/designs/credential-addressing.md
epic: credential-addressing
areas: [connector-spec, bridge]
note: "EPIC — the address, store seam and provider authorities landed; the Flux adapter remains an explicit parked story"
---

# Credential addressing and the secret-store seam (epic)

## Goal
Give a tenant's credential for a connector a stable, derivable, tenant-scoped address — so a secret
store can be wrapped in a convention rather than each deployment inventing one.

## Acceptance
- [x] **The boundary is recorded and respected.** This repository owns the address (pure, no IO); the
      store client is a host library outside the compile path, so `tests/no_network.rs` keeps meaning
      what it means.
- [x] `CredentialRef` + a `Layout` trait + `TenantLayout` as the blessed default, rendering
      `tenants/<tenant>/<authority>/<service>/<credential>` with `default` elided.
- [x] **The API version is deliberately absent** — a token must survive the vendor's v2 migration, so
      the path is `pid` + service, never the `gid`.
- [x] **A tenant id is untrusted input**: no construction can render a traversing path, and the
      validator is public so a host can check before it builds.
- [x] `parse(render(r)) == r` as a property over a hostile corpus, with the validator as the gate.
- [x] Derivation from a `Connector`, with the three outcomes distinguishable — a bad tenant is the
      caller's error, a missing authority is `Ok(None)`, a path is neither.
- [x] A `SecretStore` trait and a Vault implementation — [C-91](C-91-connector-secrets-crate.md).
- [x] Every provider declares an authority, so every provider has a path —
      [C-92](C-92-authorities-for-every-provider.md).
- [ ] The flux adapter — [C-93](C-93-flux-credential-store-adapter.md).

## Progress
- 2026-08-03 — Returned the umbrella to backlog after the address vocabulary, secret-store seam and
  provider authorities landed. C-93 owns the remaining Flux adapter and can resume when that seam is
  scheduled; C-494 independently extends the delivered host seam for multiple instances.
- 2026-07-30 — **The pure layer landed.** `crates/connector-spec/src/credential.rs`;
  `Connector::credential_ref_for` and `local_credential_name` beside `oip_of_member`. 9 tests in
  `tests/credential_paths.rs`. No artifact changed — the derivation reaches nothing generated.
- 2026-07-30 — Found while testing: an explicitly-spelled `default` service parsed, which would have
  been a **second spelling of one address** and a store holding the same credential twice with nothing
  to say which is current. Refused now, as `Gid::parse` already does.
- 2026-07-30 — **The remembered path `tenants/{uuid}/cloud/google/gemini` does not exist.** It
  conflates action-proxy's `customer/<uuid>/integrations/<uuid>`, the Go credentials-store's
  un-tenanted `cloud/<provider>/<service>`, and the vendor's internal secret store's
  `tenants/<id>/credentials/<id>`. The
  design records all three.

## Notes
- **flux already has a store trait**, `flux_credentials::CredentialStore`, with a complete
  `VaultCredentialStore` that has **zero callers** — it exists for a host app to inject. Its key is
  `plugin:<name>:<purpose>` with no tenant, and D-83's own acceptance said `plugin+purpose[+account]`
  before shipping without the `[+account]`.
- The reason for anything new here is **not the storing — it is the addressing.** flux has no
  connector addresses, so it cannot derive this path.
