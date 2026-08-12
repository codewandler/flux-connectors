---
id: C-558
title: "Expose the engine-free channel-handshake producer"
pillar: Connector
status: done
priority: 0
design: docs/designs/catalog-artifact.md
epic: catalog-artifact
areas: [connector-resolve, connector-pack]
note: "The LAST engine-free gap for Exchange X-156. C-557 relocated the operation-plan producers; channel-handshake resolution (connector_pack::channel_plan, Configuration::channel_snapshot, Credentials::resolve_channel) is still pub(crate) on connector-pack, so Exchange's X-101 supervised-channel path pins connector-pack and the flux engine, keeping ENGINE_LINE alive. connector-pack's channel.rs links NO flux — this is a relocation, not new enforcement."
---

# Expose the engine-free channel-handshake producer

## Goal

A host (flux-exchange, X-156) resolves a connector's channel handshake **engine-free** — the last
connector-pack usage that has no engine-free path. C-557 relocated the operation-plan producers
(endpoint resolution + credential assembly) into `connector-resolve`; this does the same for the
**channel** handshake, so Exchange can drop `connector-pack` from its graph entirely and retire its
`ENGINE_LINE` lockstep. Verified by X-156's feasibility scan: of seven exchange usages of
connector-pack, six now have engine-free paths in 0.25; `channel_plan` is the only one that does not.

## The gap (from X-156's scan, against the 0.25 sources)

`connector_pack::channel_plan(connector, binding, credentials: Credentials, settings: Configuration)
-> PreparedChannelPlan` (connector-pack/src/channel.rs) resolves a `catalog::Channel`'s `connect`
handshake: it substitutes the channel service base URL from the tenant's endpoint config
(`Configuration::channel_snapshot`, pub(crate)), validates the composed authority against the
declared one (`connector_resolve::validate_templated_authority` — already engine-free and public),
and resolves the channel's `connect.auth` credentials (`Credentials::resolve_channel`, pub(crate))
placing them into the URL/headers. None of `channel_snapshot`, `resolve_channel`, or any channel
producer is public or present in connector-resolve 0.25. connector-pack's `channel.rs` links no
`codewandler-flux-*` today, so this is a relocation of existing enforcement, not a new one.

## Acceptance

- [x] **An engine-free channel producer** in `connector-resolve` — mirroring C-557's shape — takes a
      `ConfigPort` (the channel endpoint config) and the `connector-secrets` secret port and returns
      `PreparedChannelPlan`, applying the SAME base-URL substitution, the same
      `validate_templated_authority` check, and the same `connect.auth` placement the current
      `channel_plan`/`resolve_channel` apply. Relocate the logic; do not reimplement it. →
      `connector_resolve::channel_plan` (`crates/connector-resolve/src/channel.rs`) with the
      relocated `Configuration::channel_snapshot` logic as `ChannelSettings` there and
      `Credentials::resolve_channel` relocated to `crates/connector-resolve/src/credentials.rs`.
- [x] `PreparedChannelPlan` (and any type the producer returns) lives engine-free — it already
      carries only `SensitiveText`; keep its redaction posture, and it must not gain a flux type. →
      moved to `crates/connector-resolve/src/channel.rs`; `connector-pack` re-exports it; the engine
      fence `engine_free_core::the_plan_deriving_core_links_no_engine_crate` still holds.
- [x] `connector-pack`'s `channel_plan` becomes a thin wrapper delegating to the engine-free
      producer (adapting its flux-side inputs into the ports), so connector-pack's channel behaviour
      is unchanged and its channel tests pass unmodified. → `crates/connector-pack/src/channel.rs`;
      the four original `channel_plan.rs` tests pass unmodified.
- [x] A differential/parity check proves the engine-free channel producer's plan is identical to the
      flux-fed `channel_plan`'s for the shipped channel bindings (the catalogue's 5 channel bindings)
      — failing-first against a seeded divergence, the C-557 pattern applied to channels. →
      `channel_plan::the_engine_free_channel_producer_matches_the_flux_fed_channel_plan` (asserts
      exactly 5 bindings) + control `..._a_seeded_divergence_in_the_engine_free_channel_plan_is_caught`.
- [x] `connector-resolve` still links no `codewandler-flux-*` (the dependency-fence test holds);
      the offline compiler fence (`no_network.rs`, `dependency_fence.rs`) still holds across any new
      edge. → all pass; no new dependency edge (the `connector-secrets` edge is C-557's), `Cargo.lock`
      untouched.

## Progress

- 2026-08-13: Filed from Exchange X-156's round-3 feasibility scan — the single remaining gap. Its
  precise ask is this story. Once it ships in connectors 0.26, X-156 resumes end to end: drop
  connector-pack entirely, own the Tool projection, retire ENGINE_LINE.
- 2026-08-13: Implemented on `impl/C-558`. `connector_resolve::channel_plan(provider, channel,
  tenant, instance, secrets, config)` is the engine-free producer; `Configuration::channel_snapshot`
  moved in as the `ChannelSettings` defaulting view and `Credentials::resolve_channel` moved into
  `connector_resolve::credentials::resolve_channel`. `ConfigField` gained a `ChannelQuery` variant
  and `Error` gained `NotSocketChannel`/`VendorSocketChannel` (twinned + mapped in connector-pack).
  `PreparedChannelPlan` moved to connector-resolve; connector-pack re-exports it and its `channel_plan`
  is now a thin wrapper resolving provider+binding and adapting `Configuration`/`Credentials` into the
  bound ports. Full gate green: `cargo nextest run --workspace --no-fail-fast` 1942 passed;
  clippy/fmt/doc clean; `connector-cli -- diff` = `1173 artifacts up to date (55 providers checked)`;
  engine + offline fences hold; `Cargo.lock` untouched (no new deps).

## Notes

- Write set: `connector-resolve` (the channel producer) and `connector-pack` (delegating its
  `channel_plan`). Runs solo. The same invariant as C-557: enforcement relocated, never duplicated;
  the parity check is the proof.
- Ships in connectors 0.26; X-156's engine-free migration consumes it and completes.
