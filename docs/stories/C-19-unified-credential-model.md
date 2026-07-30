---
id: C-19
title: Model credentials as source x acquisition x placement
pillar: Bridge
status: ready
priority: 12
design: docs/designs/unified-auth.md
epic: unified-auth
areas: [connector-spec]
note: extends C-2's AuthMethod · the axis split that keeps auth from going combinatorial
---

# Model credentials as source x acquisition x placement

## Goal
Replace the flat auth-scheme enum with the three orthogonal axes, so a new provider archetype costs
one value on one axis rather than a new variant crossing all of them.

## Acceptance
- [ ] `AuthMethod` carries `name`, `source`, `acquire`, and `place` as separate fields. The name
      identifies a **credential** an operation references. Deliberately *not* called a "purpose":
      the name says *what the thing is* (`zendesk.api_token`), never what it is for, and the AND
      case settles it — babelforce sends two headers together, which is **two credentials, one
      mechanism**, not two purposes.
- [ ] `Source` covers env-var names (tried in order), the flux token store, and a key file. **No
      variant can hold a literal credential value** — proven by a test that no serialization of any
      `AuthMethod` contains a secret.
- [ ] `Acquisition` has at least `static`, `basic_join { user_source }`, and `oauth2 { grant,
      token_url, scopes }`; `jwt`, `session` and `hmac` are **accepted by the schema** even if
      unimplemented, so adding one later does not reshape the model.
- [ ] `Placement` has `header { name, prefix }`, `query { name }`, and `cookie { name }`. The
      `prefix` field is what makes `Bearer `/`Basic `/`Token `/empty one code path.
- [ ] Each acquisition declares whether it is **effectful** (needs network/cache/refresh), and a test
      asserts `oauth2` and `session` are effectful while `static`, `basic_join` and `jwt` are not.
- [ ] **Superset proof**: the four flux `AuthScheme` presets (`Bearer`, `Basic`, `Header{name}`,
      `Query{name}`) each round-trip to and from the unified model exactly. This is the test that
      keeps the seam acceptable to flux.
- [ ] A method can be marked **deliberately excluded** with a reason — "known and excluded" must be
      distinguishable from "absent" (babelforce's deprecated `X-Auth-Access-*` pair).

## Progress
- (not started)

## Notes
- **Ordering edge: C-2 lands first.** This story extends the `AuthMethod` that C-2 defines; do not
  start until C-2 is merged, or the two will collide on the same types.
- The three in-scope providers exercise only `static` and `basic_join` plus the header placement
  prefix. Do not implement `hmac` — the design flags it as the archetype most likely not to fit.
- flux's vocabulary to stay compatible with:
  `../flux/crates/flux-plugin-protocol/src/lib.rs:344`.
