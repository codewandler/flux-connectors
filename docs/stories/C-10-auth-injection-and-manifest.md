---
id: C-10
title: Emit the $auth marker and the connector manifest
pillar: Codegen
status: done
priority: 7
design: docs/designs/auth-seam.md
epic: connectors-v1
areas: [connector-flux, flux-bridge]
note: "CLOSED 2026-08-12 as superseded by flux-roadmap Decision 0022 (adopted by C-535), never implemented. Flux never grows a connector module loader, so a module naming a credential for flux to resolve has no consumer; auth assembly landed in Rust instead (C-114/C-115/C-116) and the manifest half ships today. Kept as honest history"
---

# Emit the $auth marker and the connector manifest

## Goal
Generate the credential reference in the Flux call and the `<provider>.connector.toml` manifest that
declares what the host may resolve — so a connector's capabilities are manifest-scoped exactly as a
plugin's are.

## Acceptance
- [ ] Auth headers emit `{"$auth": {credential: "<name>"}}` — a reference, never a value.
- [ ] `<provider>.connector.toml` is generated with `http_hosts`, the endpoint env spec, and one
      `[[auth]]` entry per method (credential name, scheme, env, user_env).
      **It is a build artifact and a declaration, not an installable capability grant** — C-16 proved
      flux has no file-based capability manifest, so credentials reach flux through operator config.
- [ ] **An operation requiring several credentials together emits one marker each** — the emitter
      must handle an AND-set, not just a single credential.
      **Use a synthetic fixture, not babelforce.** Babelforce's `X-Auth-Access-Id` +
      `X-Auth-Access-Token` pair was this story's motivating example and is now deprecated (C-17), so
      no in-scope provider exercises the AND case. The capability is still required — OpenAPI models
      it and providers use it — but it needs a fixture of its own.
- [ ] When an operation offers **alternative** requirement sets, codegen picks one deterministically
      (documented rule — e.g. the first satisfiable set in declared order) and records the choice, so
      regeneration is stable and a reader can see why that scheme was chosen.
- [ ] An operation requiring **no** auth emits no marker at all and no credential header.
- [ ] A test asserts **no credential value** appears in any generated artifact or in the lockfile.
- [ ] `http_hosts` is derived from the connector's base URL and is never widened to `*`.
- [ ] Generated ops declare the `network` effect.

## Progress
- **2026-08-12 — closed as superseded by Decision 0022
  (`../flux-roadmap/decisions/0022-connectors-compile-to-a-catalog-artifact.md`), adopted by
  [C-535](C-535-adopt-decision-0022.md). Nothing above was implemented, and the
  acceptance boxes stay unticked deliberately** — that is the honest history (the C-496 pattern).
  The `$auth` marker assumed flux would load `connectors/<name>.flux` as a module and resolve the
  credential in-language; that world was never built, Decision 0022 rule 5 states Flux never grows
  a connector module loader, and the compiled form of a connector becomes a catalog artifact
  ([C-534](C-534-catalog-artifact-epic.md)). What this story wanted arrived by other routes: auth
  assembly moved into Rust in `connector-pack` (C-114/C-115/C-116 — the prefix, the base64 pair,
  query placement, redactor registration), and the manifest half ships today —
  `connectors/*.connector.toml` carries `[[auth]]` entries and derived hosts (67 manifests on
  2026-08-12: `ls connectors/ | grep -c '.connector.toml'` → `67`). The no-credential-value
  guarantee it asked a test for is vision principle 4, enforced across artifacts and lockfile.

## Notes
- **Superseded without implementation** by Decision 0022 via [C-535](C-535-adopt-decision-0022.md);
  see Progress. The notes below are kept as written — they describe the module-loading world this
  story was filed for, and the `done` status records the close, not delivery.
- Blocked on nothing here — the *generated text* can be produced and golden-tested before flux
  understands `$auth`. Only the live run (`C-15`) needs the seam released.
- Object keys quote losslessly in Flux text (`fmt_obj_key`,
  `../flux/crates/flux-lang/src/format.rs:479`), so `{"$auth": …}` is expressible.
- Manifest shape mirrors `EndpointSpec`/`AuthMethod`/`Caps`
  (`../flux/crates/flux-plugin-protocol/src/lib.rs:422`).
