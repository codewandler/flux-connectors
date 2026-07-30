---
id: C-42
title: Emit catalog.json for the public site
pillar: Codegen
status: done
priority:
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
- [x] `flux-connectors build` writes `site/catalog.json` (path is this story's to choose, but it must
      sit where the site build can read it).
      → `crates/connector-cli/src/workspace.rs::site_catalog_path`, planned in
      `pipeline.rs::plan`; pinned by `tests/site_catalog.rs::the_build_writes_and_checks_site_catalog_json`.
- [x] Per provider: id, vendor, description, base URL, auth scheme, operation count.
      → `site.rs::ProviderEntry` / `ProviderAuth::schemes` / `operation_count`.
- [x] Per operation: id, provider, description, risk, idempotency, method, path, typed parameters
      with their JSON Schema, the credentials required, the hosts reached, and **the generated Flux
      source verbatim**.
      → `site.rs::OperationEntry`; pinned by
      `tests/site_catalog.rs::every_shipped_operation_carries_its_metadata_and_its_flux`, which
      compares the embedded Flux against `connector_flux::emit_operation` for all 25 operations.
- [x] **A `status` field per operation** saying whether it currently works, and if not, why — this is
      not decoration, see Notes.
      → `crates/connector-cli/src/status.rs`, four rules over the IR; pinned by
      `tests/site_catalog.rs::the_status_of_every_operation_is_derived_from_the_ir`.
- [x] **No credential value anywhere.** Env var names only; a test asserts it.
      → `tests/site_catalog.rs::no_credential_value_reaches_the_document` runs the real binary with a
      credential's variable set to a sentinel and asserts the sentinel is absent and the *name*
      present.
- [x] Deterministic: rebuilding from unchanged inputs is byte-identical, and the file is a checked
      artifact like every other generated output.
      → it travels through `pipeline::plan` like every other artifact;
      `tests/site_catalog.rs::rebuilding_the_document_writes_nothing` and
      `site.rs::serialization_is_deterministic`.
- [x] The JSON shape is documented, because a website will be written against it.
      → [catalog-json.md](../designs/catalog-json.md).

## Progress
- **Done.** `site/catalog.json` is written by `flux-connectors build` as a fourth backend over the
  same IR, through `pipeline::plan` like every other artifact. Emitter:
  `crates/connector-cli/src/site.rs`. Shape: [catalog-json.md](../designs/catalog-json.md).
- **Reuse, not re-derivation.** `catalog.rs` gained `credential_mechanisms` and made `host_of`
  crate-visible; both backends now share one walk, so `catalog.json` and `crates/catalog` cannot
  disagree about what authenticates an operation or which host it reaches. The Flux is emitted
  **once** per operation (`seam::emit`) and handed to both.
- **`status` is derived, not listed** (`crates/connector-cli/src/status.rs`). Four rules over the IR,
  which between them reproduce README.md's four "Known limits" per operation:
  `no-credential` (effective auth empty) · `credential-not-injected` (effective auth non-empty) ·
  `unencodable-query-value` (a query param whose schema is not numeric/boolean, C-30's rule verbatim)
  · `unbound-base-url-template` (`base_url` holds a `{name}`). The first two are complementary, so
  every operation carries exactly one credential verdict. Result today: `zendesk-ticket-search` is
  the **only** zendesk operation flagged for encoding — the honest 6/7 the query-encoding design
  records — and all nine freshdesk operations report `no-credential`.
- **One fact could not be derived** and is a single commented `const`,
  `status.rs::CREDENTIALS_REACH_THE_REQUEST`: whether the *emitter* attaches a declared credential.
  That is a property of `connector-flux`, not of any provider, so no walk of the IR can see it. C-10
  flips it in one line. Everything else is a rule, not an inventory.
- **Scope on the issue, not the operation.** Since nothing can make a live call yet, `works` is false
  for all 25 operations; `scope` (`catalog`/`provider`/`operation`) is what lets the explorer
  separate a defect an operation owns from one it inherits, instead of rendering a useless "0 of 25".
- **`--provider <name>` deliberately does not write the document.** It covers every provider at once,
  so a scoped run would truncate it; the committed file is left alone and not reported stale.
- **Additive for C-37 by construction:** every entity is a JSON object with named fields and
  `schema_version` is not bumped for added keys, so `oip`/`pid` land as new fields.
- Not done here: nothing under `web/` or `.github/` — C-43 wires the site to `site/catalog.json`.

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
