---
id: C-190
title: Publish connector-catalog, connector-pack and connector-secrets to crates.io
pillar: Build
status: in-progress
priority: 1
design:
epic:
areas: [build]
note: "filed from ai-agent-platform 2026-07-31 when the three consumable crates were unpublished (404 verified that day), and the consuming repo's AGENTS.md forbids path and git deps. Same reasoning C-1 used for flux-lang, pointed outward. **The 404 is stale**: 0.7.0 and 0.8.0 went out 2026-07-31 and 0.9.0 on 2026-08-01, so five of six acceptance items were satisfied by the release cuts. What remained was proving consumability from outside the workspace"
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

- [x] `connector-catalog`, `connector-pack` and `connector-secrets` publish to crates.io under a
      vanity prefix, following `flux/crates/flux-sdk/PUBLISHING.md`: the **package** name carries the
      prefix, the **lib** name does not, so `use connector_pack::…` and `use catalog::…` are
      unchanged in every consumer and in this workspace.
      → `codewandler-connector-{catalog,pack,secrets}` are each live at 0.7.0/0.8.0/**0.9.0**. The
      lib-name half was the untested one and is now proved *from outside*: a crate depending on the
      three package names, with no `package =` alias of its own, compiles `use catalog::…`,
      `use connector_pack::…` and `use connector_secrets::…`.
- [x] The compiler crates — `connector-spec`, `connector-flux`, `connector-cli` — stay **unpublished**.
      A consumer needs the catalogue and the pack, not the compiler, and publishing the compiler would
      make its offline guarantee someone else's problem to keep.
      → `codewandler-connector-spec` stops at 0.8.0 and shipped nothing at 0.9.0 (C-407 moved the
      addressing to `connector-address`, which is what had dragged it in). `codewandler-connector-flux`
      and `codewandler-connector-cli` are 404. The unprefixed `connector-cli` on crates.io is an
      unrelated crate (`github.com/dickwu/tauri-connector`).
- [x] `connector-pack` publishes with a real dependency on the **published** `connector-catalog`, not
      the path entry — verified by a `cargo publish --dry-run` that resolves from the registry.
      → Stronger than a dry-run: the *published* `codewandler-connector-pack` 0.9.0 manifest declares
      `codewandler-connector-catalog ^0.9.0` and `codewandler-connector-secrets ^0.9.0` as normal
      registry dependencies, and a consumer resolved and built through them.
- [x] `connector-secrets` publishes with `reqwest` optional and off by default, as it is today, so a
      consumer that wants only the trait does not pull a Vault client.
      → Published feature map is `default = []`, `vault = ["dep:reqwest", "dep:serde_json"]`, and on
      the scratch consumer `cargo tree -i reqwest` answers *"did not match any packages"* — no
      `rustls`, `hyper`, `native-tls` or `openssl` in the graph either. Verified from the resolved
      graph, not the manifest.
- [x] A release procedure is written down — publish order (catalog → pack; secrets independent), what
      the version means relative to the workspace version, and who bumps a consumer. A repository that
      publishes without one drifts within two releases.
      → `AGENTS.md` § Release process (owner-stated 2026-08-01), with § Publishing contract beside it.
- [x] The published crates are proved consumable: a scratch crate outside this workspace adds them
      from the registry, calls `catalog::operation(…)` and `pack(&["zendesk"], …)`, and compiles.
      → Built **and run** outside the workspace. It prints 53 providers, resolves
      `zendesk-ticket-show`, installs the pack and finds `zendesk.ticket.show` in a
      `flux_runtime::ToolRegistry`. See `## Progress` for the consumer manifest this established.

## Notes

- Depends on [C-192](C-192-flux-0-41-bump.md): publishing at 0.39 would publish a crate the consumer
  cannot link.
- `connector-catalog` is documented as containing static data only — no filesystem, no runtime, no
  transitive dependencies. That property is exactly what makes it safe to publish, and the publish
  must not quietly acquire one.
- The consuming epic is `connectors-host` in ai-agent-platform
  (`docs/designs/connectors-host.md` there); its C-63/C-66 are blocked on this.

## Progress

**2026-08-01 — the consumability proof, and what it established.** Five items were already satisfied
by the 0.7.0/0.8.0/0.9.0 release cuts; each was re-verified against crates.io rather than assumed.
The sixth was the work. Findings are written into
[docs/integrating-with-flux.md](../integrating-with-flux.md) § Step 0 — a consumer-facing fact left
only in a report is a fact nobody has.

**The consumer manifest, established by compiling it outside this workspace:**

```toml
[dependencies]
codewandler-connector-catalog = "0.9.0"   # lib `catalog`
codewandler-connector-pack    = "0.9.0"   # lib `connector_pack`
codewandler-flux-runtime      = "0.46"    # the engine line, and it is not optional
```

Three dependencies is the whole minimum for Path A + Path B. `Credentials`, `Configuration`,
`MemoryConfig`, `CredentialRef`, `Secret`, `SecretStore`, `MemoryStore`, `StoreError` and
`TenantLayout` all come off `connector_pack`'s re-export.

What each probe retired:

1. **The lib-name claim holds from outside.** A consumer names the `codewandler-` packages and writes
   `use catalog::…` / `use connector_pack::…` with **no `package =` alias in its own manifest** —
   cargo binds the extern to the dependency's `[lib] name` unless the consumer renames it. This had
   never been checked outside the workspace, where the root manifest's alias could have been hiding
   it.
2. **The flux line is the single most useful fact.** `connector-pack` 0.9.0 requires `flux-core`,
   `flux-runtime` and `flux-lang` at `^0.46` (and `flux-spec ^1.3`), and `pack()` returns
   `impl FnOnce(&mut ToolRegistry) -> flux_core::Result<()>`, so those types are in a consumer's
   signature whether it names the crates or not. A `0.x` minor is semver-incompatible, so a host on
   another line gets **two engines, not a warning** — measured by pinning
   `codewandler-flux-runtime = "0.45"` beside pack 0.9.0:

   ```text
   error[E0308]: mismatched types
     |     let _ = install(&mut registry);
     |             ------- ^^^^^^^^^^^^^ expected `flux_runtime::ToolRegistry`, found `ToolRegistry`
   note: there are multiple different versions of crate `flux_runtime` in the dependency graph
   ```

   On a correct consumer, `cargo tree -d` reports **no duplicated `codewandler-*` crate at all** —
   only third-party `hashbrown` and `syn`. That is the check a host should run.
3. **No Vault client by default**, verified from the resolved graph: `cargo tree -i reqwest` →
   *"did not match any packages"*, and no `rustls`/`hyper`/`native-tls`/`openssl` either.
4. **`codewandler-connector-address` is named directly only for `Pid`/`Gid`/`Oip`.** The *credential*
   half (`CredentialRef`, `InstanceId`, `Layout`, `TenantLayout`, `TenantInstances`) reaches a
   consumer through `connector-secrets`' re-export, and the subset a host binding the ports touches
   reaches it through `connector-pack`. The *identifier* half is re-exported by neither:
   `use connector_secrets::Pid` fails with `no Pid in the root`, measured. The constants and
   validators (`TENANTS_ROOT`, `INSTANCES_SEGMENT`, `MAX_TENANT`, `validate_tenant`,
   `validate_instance`) and `VaultStore` need `connector-secrets` named directly.

The scratch crates were built in a temp directory outside the repository and are deliberately not
committed — nothing here is a workspace member and no manifest in this repository changed.

**Not done, and deliberately:** the `note:` correction above records that the story's filing-time
"verified 404" is stale, but this story never *performed* a publish — the 0.7.0–0.9.0 releases did,
under `AGENTS.md` § Release process. Nothing about the publish pipeline was changed here.
