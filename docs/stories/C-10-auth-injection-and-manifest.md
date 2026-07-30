---
id: C-10
title: Emit the $auth marker and the connector manifest
pillar: Codegen
status: ready
priority: 7
design: docs/designs/auth-seam.md
epic: connectors-v1
areas: [connector-flux, flux-bridge]
note: pairs with C-16 · the second generated artifact
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
- (not started)

## Notes
- Blocked on nothing here — the *generated text* can be produced and golden-tested before flux
  understands `$auth`. Only the live run (`C-15`) needs the seam released.
- Object keys quote losslessly in Flux text (`fmt_obj_key`,
  `../flux/crates/flux-lang/src/format.rs:479`), so `{"$auth": …}` is expressible.
- Manifest shape mirrors `EndpointSpec`/`AuthMethod`/`Caps`
  (`../flux/crates/flux-plugin-protocol/src/lib.rs:422`).
