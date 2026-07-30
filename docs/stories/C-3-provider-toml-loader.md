---
id: C-3
title: Load and validate provider TOML
pillar: Spec
status: ready
priority: 4
design: docs/designs/connector-pipeline.md
epic: connectors-v1
areas: [connector-spec]
---

# Load and validate provider TOML

## Goal
Parse `providers/<name>.toml` into the IR with errors good enough to author against, covering both
roles the file plays: a pointer at a vendor spec, and a complete hand-authored connector definition.

## Acceptance
- [x] A TOML that declares operations inline — with no vendor spec present at all — produces a
      complete, valid `Connector`. This is the "two front-ends, one IR" requirement.
- [x] A TOML that only points at a spec source plus patches parses into the patch set for `C-6`.
- [x] Validation rejects: unknown keys, an operation with no method or path, an auth credential with no
      scheme, and a `basic` scheme missing `user_env`.
- [x] Golden-file error snapshots for each rejection above — failing-first, since error text is the
      authoring interface.
- [x] A documented JSON Schema for the provider TOML, kept in sync by a test.
- [x] **`deny_unknown_fields` (or a strict pre-pass) rejects typo'd keys.** C-2's review proved the
      hole and its direction: a mistyped `authh` on an operation deserializes to `auth: None`, so the
      operation silently **inherits the connector default credentials** rather than failing — the
      failure direction is credential-*sending*, not fail-closed. Likewise `envv` yields `env: []`
      with no error. Note the attribute must go on the IR types themselves; it cannot be added purely
      "in the loader".
- [x] An **empty mechanism** (`{"credentials": []}`) inside a non-empty alternatives list is
      rejected — it is a degenerate second spelling of "no auth" and must not have two encodings.

## Progress
- **Done.** `crates/connector-spec/src/provider.rs` is the front-end; `provider::load(name, source)`
  takes bytes and returns a validated `LoadedProvider { connector, spec, patch }`. Still no network
  and no filesystem in this crate.
- **The typo hole is closed on the IR types, as C-2's review said it had to be.** The loader
  deserializes `providers/*.toml` *straight into* `Operation`/`AuthMethod`/`Param`/…, so
  `#[serde(deny_unknown_fields)]` now sits on every struct in `src/ir.rs` and `src/auth.rs`.
  `tests/strict_fields.rs` uses only the IR types and `toml` — no loader — and fails at the merge
  base for the right reason: `authh` deserialized to `auth: None`, i.e. inherit the connector
  default. Two side effects worth knowing:
  - `AuthMethod::scheme` **lost its `#[serde(default)]`**. Omitting it used to mean "send it as a
    bearer", a safety decision by silence, in the same category `Risk` and `Idempotency` already have
    no `Default` for. `AuthScheme::default()` is untouched, so the flux mirror still holds.
  - the "the IR is permissive, the loader is strict" paragraph at the top of `src/ir.rs` was the
    premise the review disproved; it is rewritten to record why.
- **Empty mechanisms are rejected anywhere they appear** — on an operation, in `default_auth`, or in
  a patch. `Some(vec![])` (explicitly no auth) and `None` (inherit) both stay legal and are both
  covered by a test.
- Semantic validation reports **every** problem in one pass; see `tests/golden/several-problems.*`.
  13 golden fixtures, regenerated with `UPDATE_GOLDEN=1`.
- **`AuthMethod` gained `user_suffix: Option<String>`** so zendesk's `<email>/token` user half is
  expressible without telling an operator to pre-compose a credential in an env var — the shape
  `auth-seam.md` §7.5 says is required. Additive; only the authoring side. It deliberately does
  **not** touch the sibling gap (freshdesk's secret-in-the-user-position), which
  `provider-operation-inventory.md` §6.2 reserves for C-16.
- **`schema/provider-toml.schema.json`** is the documented format, published as
  `PROVIDER_TOML_JSON_SCHEMA`. `tests/provider_schema.rs` keeps it honest by asking *serde* which
  keys each type accepts (it feeds the type an impossible key and reads the field list out of
  `deny_unknown_fields`' own error) and diffing that against the schema's `properties`.
- **Not done here, deliberately:** no OpenAPI ingest (C-4), no overlay application (C-6) — the patch
  set is parsed and validated but nothing consumes it yet — and no `providers/*.toml` files for the
  three launch providers (C-17).

## Notes
- `deny_unknown_fields` everywhere: a silently ignored typo in a provider file is exactly how
  action-proxy's YAML drifted.
- No network in this crate — the loader takes bytes.
