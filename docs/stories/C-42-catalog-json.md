---
id: C-42
title: Emit catalog.json for the public site
pillar: Codegen
status: ready
priority: 4
design: docs/designs/public-docs.md
epic: public-docs
areas: [connector-cli]
note: the site's data must be generated, never hand-maintained
---

# Emit catalog.json for the public site

## Goal
Make the catalogue consumable by a static website: one generated JSON file carrying every provider
and operation with the metadata a browser needs, produced by the same build as every other artifact.

## Acceptance
- [ ] `flux-connectors build` writes `site/catalog.json` (path is this story's to choose, but it must
      sit where the site build can read it).
- [ ] Per provider: id, vendor, description, base URL, auth scheme, operation count.
- [ ] Per operation: id, provider, description, risk, idempotency, method, path, typed parameters
      with their JSON Schema, the credentials required, the hosts reached, and **the generated Flux
      source verbatim**.
- [ ] **A `status` field per operation** saying whether it currently works, and if not, why — this is
      not decoration, see Notes.
- [ ] **No credential value anywhere.** Env var names only; a test asserts it.
- [ ] Deterministic: rebuilding from unchanged inputs is byte-identical, and the file is a checked
      artifact like every other generated output.
- [ ] The JSON shape is documented, because a website will be written against it.

## Progress
- (not started)

## Notes
- **This is the fourth emitter over one IR** — after the Flux module, the manifest, and the
  `connector-catalog` crate. `crates/connector-cli/src/catalog.rs` already builds the Rust catalogue
  from the IR; this is the same walk with a different backend, and it should reuse that code rather
  than re-deriving.
- **The site must never hand-maintain catalogue data.** That is the action-proxy failure this whole
  project exists to correct, re-enacted in JavaScript. Generating this file is what prevents it.
- **The `status` field carries the honesty.** `zendesk-ticket-search` does not work (query values are
  not percent-encoded), and every Freshdesk operation is unauthenticated (its Basic form puts the
  secret in the username position, which the IR cannot yet mark as secret). Publishing those without
  a machine-readable caveat would be worse than not publishing at all — the explorer filters on this.
- Once C-37 lands, each entry gains its `oip` address; design the shape so that is additive.
