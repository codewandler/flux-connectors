---
id: C-2
title: Define the Connector IR
pillar: Spec
status: done
priority:
design: docs/designs/connector-pipeline.md
epic: connectors-v1
areas: [connector-spec]
note: the contract every other crate speaks
---

# Define the Connector IR

## Goal
Define the normalized intermediate representation — connector, auth methods, operations, params,
quirks, provenance — that both front-ends produce and codegen consumes. Everything downstream depends
on this shape, so it lands before the loaders.

## Acceptance
- [x] Rust types for `Connector`, `AuthMethod`, `Operation`, `ParamSet` (path/query/header/body),
      `Quirks`, and `Provenance`, with serde round-trip tests.
- [x] Each parameter and the response carry their JSON Schema — types survive the pipeline rather
      than collapsing to strings.
- [x] `Operation` carries `risk`, `idempotency`, and `description`, mapping onto the metadata a Flux
      composite op declares.
- [x] The IR serializes deterministically: identical inputs produce byte-identical output (ordered
      maps, no `HashMap` iteration order leaking). A test asserts this.
- [x] Auth scheme vocabulary matches `flux_plugin_protocol::AuthScheme` — `bearer`, `basic`,
      `header{name}`, `query{name}` — so no second vocabulary is invented.
- [x] **A connector declares many auth methods, and each operation selects among them.** The IR
      models this as OpenAPI does: a connector carries `auth: Vec<AuthMethod>` keyed by credential name, and
      an operation carries a list of *requirements*, where each requirement is a **set** of credentials
      that must all be satisfied together (AND) and the list itself is a set of **alternatives**
      (OR).
- [x] The three cardinalities all round-trip and are covered by tests:
      **zero** (an unauthenticated operation — OpenAPI's `security: []`, e.g. a health/ping
      endpoint), **one-of-several** (the operation accepts either OAuth2 or an API key), and
      **all-of-several** (two credentials sent together on one request).
- [x] An operation that declares no requirement inherits the connector-level default, exactly as
      OpenAPI's document-level `security` works; an explicit empty list means "no auth" and must be
      distinguishable from "unset".

## Progress
- **Done.** The IR lives in `crates/connector-spec/src/ir.rs` (`Connector`, `Operation`, `ParamSet`,
  `Param`, `Quirks`, `Provenance`, `Risk`, `Idempotency`, `HttpMethod`, `JsonSchema`) and
  `crates/connector-spec/src/auth.rs` (`AuthMethod`, `AuthScheme`, `AuthRequirement`, `OAuth2Spec`,
  `OAuthGrant`, `OAuthRedirect`). Tests: `tests/ir_roundtrip.rs` (9) and `tests/determinism.rs` (5).
- **Auth vocabulary.** Our identifying field is `AuthMethod.name` (a *credential* name). It resolves
  to flux's `AuthMethod.purpose` at the manifest boundary — flux's own field is not renamed, and the
  `$auth` marker still spells the key `purpose` because that is flux's wire format. An
  `AuthRequirement` is one *mechanism*; the credentials inside it are ANDed.
- **Determinism is stronger than "same input, same bytes".** `AuthRequirement` normalizes its
  credential set to sorted order on construction *and* on deserialization, so the encoding is a
  function of the set's members alone. Two connectors that compare `==` therefore always hash the
  same, which is what C-7 needs. `tests/determinism.rs::serde_json_object_keys_stay_sorted` is a
  tripwire against anything in the workspace enabling `serde_json/preserve_order`.
- **`Risk` and `Idempotency` have no `Default`**, deliberately — both front-ends must state them,
  because a defaulted `risk` is a safety decision made by silence.
- **`Provenance` deliberately omits `ir_sha256`** even though the pipeline design lists it: it is
  computed *from* the serialized IR, so storing it inside the value being hashed would make the hash
  depend on itself. C-7 should write it into `connectors.lock` instead.
- **Left for the stories that own them:** `deny_unknown_fields` and all validation (C-3 — the IR is
  permissive, the loader is strict); a `retry` quirk (C-12 needs one, and `Quirks` currently carries
  only the three the pipeline design names — it is an additive field); request-body emission (C-9).

## Notes
- Determinism matters beyond tidiness: `connectors.lock` hashes the IR (`C-7`), so nondeterministic
  serialization would produce phantom drift on every build.
- The IR must be expressive enough that a hand-authored TOML can define an operation with **no** spec
  at all — see `C-3`.
- **Babelforce is the live proof of the multi-auth requirement**, not a hypothetical: its spec
  (`~/babelforce/projects/babelforce-api/babelforce-api/openapi/manager.openapi.json`) declares
  three schemes — `oauth2` (password flow), `accessId` (`X-Auth-Access-Id`) and `accessToken`
  (`X-Auth-Access-Token`) — where the two apiKey headers must be sent **together** (AND) and are an
  **alternative** to OAuth2 (OR).
- **The `X-Auth-*` pair is deprecated and must not be emitted** (see C-17). That does *not* remove
  the need to model AND: the IR still has to represent what the spec declares in order to
  deliberately exclude it, and "this requirement set is excluded" is a different statement from "this
  scheme does not exist". A single-credential shortcut cannot express either.
- Babelforce ships **SSO-issued Bearer** today and plans **JWT**, so its own scheme list grows over
  time — the OR/alternatives case is permanent, not transitional.
