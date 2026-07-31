---
id: C-190
title: Publish connector-catalog, connector-pack and connector-secrets to crates.io
pillar: Build
status: ready
priority: 1
design:
epic:
areas: [build]
note: "filed from ai-agent-platform 2026-07-31 — the three consumable crates are unpublished (verified 404), and the consuming repo's AGENTS.md forbids path and git deps. Same reasoning C-1 used for flux-lang, now pointing outward"
---

# Publish `connector-catalog`, `connector-pack` and `connector-secrets` to crates.io

## Goal

Make the three consumable crates resolvable from outside this workspace, so a host can depend on the
catalogue and the pack the same reviewed way this repository depends on flux.

## Why now

`~/babelforce/projects/ai-agent-platform` is building a `ConnectorRuntime` over
`connector_catalog::providers()` and `connector_pack::pack(…)`, with `connector-secrets` behind it.
None of the three is on crates.io (verified 2026-07-31: `does not exist`), and that repository's
`AGENTS.md` forbids path and sibling dependencies for the same reason C-1 gave here — a git or path
dep couples a shipped image to an unreviewed working tree.

This is C-1's argument pointed the other way. C-1 chose a registry pin for `flux-lang` because "the
registry pin is the only form that resolves everywhere". A consumer of *this* repository needs the
same thing from us.

## Acceptance

- [ ] `connector-catalog`, `connector-pack` and `connector-secrets` publish to crates.io under a
      vanity prefix, following `flux/crates/flux-sdk/PUBLISHING.md`: the **package** name carries the
      prefix, the **lib** name does not, so `use connector_pack::…` and `use catalog::…` are
      unchanged in every consumer and in this workspace.
- [ ] The compiler crates — `connector-spec`, `connector-flux`, `connector-cli` — stay **unpublished**.
      A consumer needs the catalogue and the pack, not the compiler, and publishing the compiler would
      make its offline guarantee someone else's problem to keep.
- [ ] `connector-pack` publishes with a real dependency on the **published** `connector-catalog`, not
      the path entry — verified by a `cargo publish --dry-run` that resolves from the registry.
- [ ] `connector-secrets` publishes with `reqwest` optional and off by default, as it is today, so a
      consumer that wants only the trait does not pull a Vault client.
- [ ] A release procedure is written down — publish order (catalog → pack; secrets independent), what
      the version means relative to the workspace version, and who bumps a consumer. A repository that
      publishes without one drifts within two releases.
- [ ] The published crates are proved consumable: a scratch crate outside this workspace adds them
      from the registry, calls `catalog::operation(…)` and `pack(&["zendesk"], …)`, and compiles.

## Notes

- Depends on [C-192](C-192-flux-0-41-bump.md): publishing at 0.39 would publish a crate the consumer
  cannot link.
- `connector-catalog` is documented as containing static data only — no filesystem, no runtime, no
  transitive dependencies. That property is exactly what makes it safe to publish, and the publish
  must not quietly acquire one.
- The consuming epic is `connectors-host` in ai-agent-platform
  (`docs/designs/connectors-host.md` there); its C-63/C-66 are blocked on this.
