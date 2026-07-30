---
id: C-17
title: Author provider configs for zendesk, freshdesk and babelforce
pillar: Spec
status: ready
priority: 10
design: docs/designs/connector-pipeline.md
epic: connectors-v1
areas: [connector-spec, providers]
note: **the goal** · three configs that compile to executable .flux
---

# Author provider configs for zendesk, freshdesk and babelforce

## Goal
Ship `providers/zendesk.toml`, `providers/freshdesk.toml`, and `providers/babelforce.toml` that
compile to `.flux` modules whose ops flux can load and execute — the concrete proof that a connector
replaces a hand-written plugin.

## Acceptance
- [x] `providers/babelforce.toml` — auth is **SSO-issued Bearer** (`Authorization: Bearer <token>`).
      The legacy `X-Auth-Access-Id` / `X-Auth-Access-Token` header pair is **deprecated and must not
      be modelled or emitted**, even though it still appears in the vendored spec's
      `securitySchemes`; the provider config has to exclude it explicitly.
- [ ] The connector manifest schema accommodates a **JWT** scheme without reshaping — babelforce
      plans to add it, and discovering the schema cannot express it after three providers ship is
      the expensive way to find out.
- [x] `providers/zendesk.toml` — Basic auth in Zendesk's `<email>/token` form; operation set covers
      what `../flux/plugins/zendesk/src/main.rs` exposes (ticket search/show/update, comment
      list/add, tag add, test).
- [ ] `providers/freshdesk.toml` — Basic auth (`api_key` as user, `X` as password); a curated ticket
      operation set.
- [ ] `flux-connectors build` emits `zendesk.flux`, `freshdesk.flux`, `babelforce.flux` plus their
      `.connector.toml` manifests.
- [ ] All three generated modules pass the C-11 parse-and-analyze gate.
- [x] Operation selection is curated, not exhaustive — babelforce's spec has **163 operations** and
      must yield a usable handful, not 163 tools.

## Progress
- **Three provider definitions land, all hand-authored, all loading through
  `connector_spec::provider::load`.** `crates/connector-spec/tests/shipped_providers.rs` is the
  gate: the files load, the counts match the inventory (7 / 9 / 9), every op id is spellable as a
  Flux declaration name, and babelforce resolves to exactly one bearer.
- **Op ids are hyphen-separated, not dotted** (`zendesk-ticket-show`). flux-lang's
  `is_valid_decl_name` admits alphanumerics, `_` and `-` only, so the inventory's dotted names are
  undeclarable (C-8). C-23 settles the canonical rule; if it picks a different spelling, these three
  files and C-8's goldens change together.
- **babelforce is inline, not a `[spec]` pointer.** `specs/babelforce/manager-0.7.0.openapi.json` is
  *not* vendored — inventory §1.3 blocks it pending confirm-and-rotate of the credential-shaped
  example in the upstream document, which is not an implementing agent's decision. The 9 selected
  operations are written out; when the spec is cleared, this file becomes `[spec]` +
  `[[patch.operations]]` and the set below is the selection to reproduce.
- **freshdesk ships with no credential, deliberately.** `base64(<api_key>:X)` puts the secret in the
  *username* position, which `AuthScheme::Basic` cannot express without declaring the API key as
  non-secret `user_env` config — outside secret gating and outside the redactor (§6.2). Fail-closed
  (every request 401s) was chosen over routing a live key through a non-secret path. Blocked on
  C-16; the file carries the exact block to add once it lands.
- **Not done, and not attempted:** `flux-connectors build` still runs `connector-cli`'s placeholder
  seam (`crates/connector-cli/src/seam.rs`), which is C-27's wiring, and `connector-flux` refuses
  body parameters (`Error::OutOfSlice`), which is C-9's. So no `.flux` is emitted from these files
  yet and the C-11 gate has nothing to check. The definitions are the input those two stories need.

### Schema gaps found while transcribing — none worked around
1. **Freshdesk's inverted Basic** (§6.2) — no way to say which half of a Basic pair is the secret,
   or that the other half is a literal constant. The one blocking gap.
2. **Nested body paths** — `ParamSet.body` is a flat `Vec<Param>`; Zendesk's real body is
   `{"ticket": {"comment": {"body": …}}}` and there is no field for the JSON path a body parameter
   occupies. Recorded in each parameter's `description`, which C-9 cannot compile.
3. **Constant body fields** — Zendesk always sends `ticket.safe_update = true`; `Param` has no
   "always emitted, never in the op signature". Declared as `required` + JSON Schema `const`, which
   still leaks it into the generated signature.
4. **Free-form object bodies** — two of babelforce's nine (`setCallSessionVariables`,
   `updateSessionVariables`) have `{"type": "object"}` bodies with no properties. `ParamSet.body`
   is a list of named fields, so "the body is this one schema" is inexpressible; both ship with no
   body parameter at all.
5. **Preconditions** — Zendesk's "at least one mutable field", Freshdesk's
   `(phone AND name) OR requester_id`, babelforce's "an empty status PUT is valid and does nothing".
   `Quirks` carries pagination, rate limits and error envelopes only.
6. **Base URL is a bare string** — all three endpoints are operator config (`ZENDESK_URL`,
   `FRESHDESK_DOMAIN`, `BABELFORCE_URL`), and nothing declares the binding from an env var to a
   `base_url` template variable.
7. **Provenance needs `[spec]`** — a hand-authored connector derived from an un-vendored upstream
   document cannot record what it was derived from.
8. **One name per parameter** — a wire name that differs from the caller-facing name (Freshdesk's
   `req_id` → `requester_id`) has nowhere to live; the wire name is kept and the alias dropped.

## Notes
- **Sources for the operation sets:**
  - zendesk — `../flux/plugins/zendesk/src/main.rs` is authoritative (687 lines, ~7 ops).
  - freshdesk — `~/babelforce/projects/integrations/action-proxy/dist/collections/freshdesk/freshdesk.yml`
    (649 lines) has endpoints, params and defaults.
  - babelforce — a real OpenAPI 3.0.3 document at
    `~/babelforce/projects/babelforce-api/babelforce-api/openapi/manager.openapi.json`
    (98 paths, 163 operations, servers incl. `https://services.babelforce.com`).
- **Auth reality check — all three need the [`$auth` seam](../designs/auth-seam.md) (C-16) before a
  live call can succeed.** Babelforce is Bearer, Zendesk and Freshdesk are Basic; none is expressible
  with flux's existing `{"$secret": "ENV"}` marker, which is a whole-value replacement and cannot
  produce the `Bearer ` prefix or a base64-joined pair. All three can still be *generated* and
  verified through the C-11 parse-and-analyze gate without the seam — that is the split to report
  honestly: **generated and gate-clean now, live-callable once the seam ships**.
  (An earlier note here claimed babelforce was executable today via raw `X-Auth-*` headers. That
  path is deprecated and withdrawn — see the Acceptance above.)
- Vendor the babelforce spec into `specs/babelforce/` so builds stay hermetic.
- action-proxy needed a bespoke `{{base64:encode (append (append context.user "/token:") context.api_token)}}`
  template function for Zendesk Basic auth — a good illustration of why credential assembly belongs
  in the host, not in the config layer.
