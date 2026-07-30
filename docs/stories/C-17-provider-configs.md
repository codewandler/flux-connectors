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
- [ ] `providers/babelforce.toml` — auth is two raw apiKey headers (`X-Auth-Access-Id`,
      `X-Auth-Access-Token`), so it needs **no** `$auth` seam and must be executable against flux as
      it stands today.
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
- **Auth reality check.** Only babelforce is executable with today's flux: its credentials are raw
  header values, which the existing `{"$secret": "ENV"}` marker already handles. Zendesk and
  Freshdesk are both Basic and therefore blocked on the [`$auth` seam](../designs/auth-seam.md)
  (C-16) before a live call can succeed — they can still be *generated* and gate-verified.
- Vendor the babelforce spec into `specs/babelforce/` so builds stay hermetic.
- action-proxy needed a bespoke `{{base64:encode (append (append context.user "/token:") context.api_token)}}`
  template function for Zendesk Basic auth — a good illustration of why credential assembly belongs
  in the host, not in the config layer.
