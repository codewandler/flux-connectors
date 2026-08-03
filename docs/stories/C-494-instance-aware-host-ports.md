---
id: C-494
title: "Make the connector host ports instance-aware and migratable"
area: Bridge
status: in-progress
priority: 1
areas: [bridge, credentials, configuration, release]
design: docs/designs/instance-aware-host-ports.md
note: "release-order bridge for flux-exchange X-14: enumerate addresses and atomically migrate the sole connection before a second instance is admitted"
---

# Make the connector host ports instance-aware and migratable

## Goal

Give a host enough capability to select one of a tenant's several connections without inventing a
second credential address or risking a half-migrated secret set.

## Acceptance

- [x] **Failing first:** contract tests demonstrate that the current store cannot enumerate a
      tenant/authority scope or atomically move its addresses, and that the pack cannot bind an
      instance for credential and configuration lookup.
- [x] `CredentialScope` validates the tenant and authority once; `SecretStore::references` returns
      addresses only, never values, and unsupported backends refuse explicitly.
- [x] `SecretBatch` validates every address against one scope and built-in memory/file stores apply
      checked moves, puts and deletes atomically; Vault refuses the unsupported guarantee.
- [x] `Credentials::for_instance`, `Configuration::for_instance`, and
      `ConfigStore::get_for_instance` preserve the existing constructors and the sole-connection
      lookup while selecting the existing C-406 UUID address for an instance.
- [ ] Public API documentation, focused tests, the full workspace gate and publish dry-runs are
      green; the connector crate set is prepared as one v0.18.0 release and publication remains
      CI-only.

## Progress

- 2026-08-03: filed from flux-exchange X-14 after confirming C-406 supplies the pure UUID address,
  while the current host ports still expose only point reads/writes and tenant-only bindings.
- 2026-08-03: Implemented the v0.18 host ports, atomic memory/file migration and explicit Vault
  refusal; regenerated the complete catalogue and passed the Rust, public-site and host-page gates.
  The CI-equivalent publish dry-run remains open because Cargo correctly refuses this uncommitted
  implementation worktree and repository policy forbids bypassing that check with `--allow-dirty`.
