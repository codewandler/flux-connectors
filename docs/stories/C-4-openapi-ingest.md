---
id: C-4
title: Ingest OpenAPI 3.x into the IR
pillar: Spec
status: in-progress
priority: 1
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [connector-spec]
note: "the trunk of the spec front-end — seam.rs:160 refuses every spec-backed provider until this lands, so the `[spec]` schema landed with C-3 has been unused ever since"
---

# Ingest OpenAPI 3.x into the IR

## Goal
Turn a vendored vendor OpenAPI document into IR operations — servers, paths, methods, parameters, and
schemas — so a provider TOML shrinks to a pointer plus patches.

## Acceptance
- [x] OpenAPI 3.0 and 3.1 documents parse into `Connector` operations with path, query, header, and
      body parameters, each carrying its resolved JSON Schema.
- [x] `$ref` resolution within the document works, including nested and repeated refs; a cyclic ref
      is reported as an error rather than hanging.
- [x] `servers` produce the base URL, with templated server variables preserved for tenant
      substitution (e.g. Zendesk's per-account subdomain).
- [x] Missing or malformed sections degrade to a reported diagnostic naming the offending path — a
      real vendor spec is never fully well-formed, and one bad endpoint must not fail the whole
      ingest.
- [x] Fixture-driven tests over trimmed real Zendesk and Anthropic spec excerpts committed under
      `specs/`.
- [x] **YAML as well as JSON.** Every babelforce document is YAML, and the spec cache is already
      extension-agnostic (`discover_specs` takes the version from the file stem). `serde_norway` is
      pre-added to `crates/connector-spec/Cargo.toml` by the coordinator — do not add or change any
      dependency yourself.
- [x] **`crates/connector-cli/src/seam.rs:160`'s refusal is deleted**, not worked around. A
      failing-first test builds a provider whose `[spec]` points at a fixture and asserts operations
      reach the IR; today it fails with "spec ingest (story C-4), which is not wired yet".
- [x] Ingest is a pure function from bytes to IR — `connector-spec` must not touch the network
      (`AGENTS.md`, Ownership boundaries).

## Progress
- **Done.** `crates/connector-spec/src/openapi.rs` is the ingest: bytes -> `Ingested`, no IO of any
  kind. `provider::load_with_spec` is the join, and `seam::load` hands it the document discovery
  already read. The refusal at `seam.rs:160` is gone.
- **Ingest selects nothing, and two tests say so** —
  `spec_backed_provider.rs::a_spec_backed_provider_with_no_patch_publishes_nothing` and
  `seam.rs::a_spec_backed_provider_with_no_patch_publishes_no_operations`. A pointer with no patch
  is a connector with no operations; the whole document stays reachable on
  `LoadedProvider::ingested` so C-6/C-411 have something to widen selection *over*.
- **Two grades of failure**, and the split is the design. A document that is not OpenAPI 3.x is an
  `Error::ParseSpec` and fails the provider. One bad endpoint is an `openapi::Diagnostic` naming
  method and path, and the operation is **skipped** — never ingested half-formed. There is no
  "ingest it without its body" path: a `POST` that quietly stopped sending a body is
  indistinguishable from a legitimately bodiless write. Diagnostics reach the user through
  `Plan::diagnostics`, which `build` and `diff` print.
- **The excerpts carry deliberate defects** under `/api/v2/_ingest-fixture/` — an untyped parameter,
  a `multipart/form-data` body, an operation with no `operationId` — plus a genuinely cyclic
  `OrganizationNode`. A fixture that is well-formed proves nothing about the half of this story that
  matters.
- **What C-4 deliberately did not take**, so the sequencing holds: a selector matching a set (C-411),
  a naming rule instead of a `rename` per operation (C-412), risk/idempotency by selector (C-414),
  several documents per connector (C-410), and `securitySchemes` extraction (C-5). Every selected
  operation lands in `default`; a provider declaring named services plus a `[spec]` is therefore a
  loud loader error today, and C-410 is where that is answered.
- **Three things the loader refuses rather than deciding**, each stated because the silent
  alternative is worse: a `select` naming no operation (config rot), a selection stating no `risk` or
  `idempotency` (a safety decision made by omission — the failure `Risk` has no `Default` to
  prevent), and a selection stating no `rename` (promoting a volatile `operationId` into a public op
  id). C-412 replaces the third with a rule declared once; it does not remove the decision.
- **`[spec] path` decides which document is compiled, and the loader resolves it** (review finding).
  The first landing passed `Provider::spec()` — the *last* file by stem — and used the pin only as an
  error label, so a provider pinning `specs/acme/manager-2026-07-10.json` beside
  `user-2026-06-25.json` emitted `getUser` out of the document it never named: exit 0, no
  diagnostic, `url = fmt("{base}/api/v2/user/me")`. `specs/<provider>/` is a cache of *versions of
  one document*, so this is the ordinary pin, not an exotic one, and it breaks `AGENTS.md`'s "refuse
  ambiguous or unsafe output". `ProviderInputs` now carries the **whole cache** and
  `provider::load_with_spec` resolves the pin, because which document a connector compiles from is
  the provider file's decision and choosing in the CLI is the defect itself. A pin resolving to
  nothing is refused, listing what the cache holds. Regression:
  `seam.rs::the_pinned_document_is_the_one_ingested_not_the_last_in_the_cache` and
  `spec_backed_provider.rs::the_pinned_document_is_compiled_even_when_a_later_one_sits_beside_it`.
- **`[spec] sha256` is checked against the ingested bytes**, not copied past them. It reaches
  `Provenance::spec_sha256` and from there `connectors.lock`; unchecked, the lockfile recorded a hash
  for bytes nothing hashed. Checking *upstream* drift stays C-14's — this is the local claim against
  the local bytes.
- **Three smaller diagnostics**, each restoring the module's own rule that an unrepresentable
  construct is reported rather than dropped: `options`/`trace` path-item keys, and a `cookie`
  parameter (which now skips its operation, exactly as an unrepresentable body does — publishing the
  operation without it ships a request that quietly stopped sending something the vendor declared).
  The expansion-budget diagnostic no longer asserts self-reference it cannot distinguish; it reports
  the size and quotes the measured 3,580-node maximum for scale.

## Notes
- Ingest takes bytes; fetching is `C-14`'s job.
- **Ingest makes everything available; it selects nothing.** With no patch, a spec-backed provider
  still yields no operations — selection is opt-in and is C-6/C-411's job. Prove that with a test
  rather than leaving it to inference.
- Scale to design against, measured on the babelforce documents: 398 operations, 848 component
  schemas, 527 parameters of which 47 reach their definition through a `$ref`. Nested and repeated
  refs are the common case here, not the corner.
- Do not attempt to expose every endpoint — selection is `C-6`. Ingest's job is to make everything
  *available* to patch.
- Real vendor specs are frequently wrong or incomplete; the diagnostics path is the important half of
  this story, not the happy path.
