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
- [ ] `providers/babelforce.toml` — auth is **SSO-issued Bearer** (`Authorization: Bearer <token>`).
      The legacy `X-Auth-Access-Id` / `X-Auth-Access-Token` header pair is **deprecated and must not
      be modelled or emitted**, even though it still appears in the vendored spec's
      `securitySchemes`; the provider config has to exclude it explicitly.
- [ ] The connector manifest schema accommodates a **JWT** scheme without reshaping — babelforce
      plans to add it, and discovering the schema cannot express it after three providers ship is
      the expensive way to find out.
- [ ] `providers/zendesk.toml` — Basic auth in Zendesk's `<email>/token` form; operation set covers
      what `../flux/plugins/zendesk/src/main.rs` exposes (ticket search/show/update, comment
      list/add, tag add, test).
- [ ] `providers/freshdesk.toml` — Basic auth (`api_key` as user, `X` as password); a curated ticket
      operation set.
- [ ] `flux-connectors build` emits `zendesk.flux`, `freshdesk.flux`, `babelforce.flux` plus their
      `.connector.toml` manifests.
- [ ] All three generated modules pass the C-11 parse-and-analyze gate.
- [ ] Operation selection is curated, not exhaustive — babelforce's spec has **163 operations** and
      must yield a usable handful, not 163 tools.

## Progress
- (not started)

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
