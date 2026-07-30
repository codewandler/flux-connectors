---
id: C-2
title: Define the Connector IR
pillar: Spec
status: ready
priority: 3
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
- [ ] Rust types for `Connector`, `AuthMethod`, `Operation`, `ParamSet` (path/query/header/body),
      `Quirks`, and `Provenance`, with serde round-trip tests.
- [ ] Each parameter and the response carry their JSON Schema — types survive the pipeline rather
      than collapsing to strings.
- [ ] `Operation` carries `risk`, `idempotency`, and `description`, mapping onto the metadata a Flux
      composite op declares.
- [ ] The IR serializes deterministically: identical inputs produce byte-identical output (ordered
      maps, no `HashMap` iteration order leaking). A test asserts this.
- [ ] Auth scheme vocabulary matches `flux_plugin_protocol::AuthScheme` — `bearer`, `basic`,
      `header{name}`, `query{name}` — so no second vocabulary is invented.
- [ ] **A connector declares many auth methods, and each operation selects among them.** The IR
      models this as OpenAPI does: a connector carries `auth: Vec<AuthMethod>` keyed by purpose, and
      an operation carries a list of *requirements*, where each requirement is a **set** of purposes
      that must all be satisfied together (AND) and the list itself is a set of **alternatives**
      (OR).
- [ ] The three cardinalities all round-trip and are covered by tests:
      **zero** (an unauthenticated operation — OpenAPI's `security: []`, e.g. a health/ping
      endpoint), **one-of-several** (the operation accepts either OAuth2 or an API key), and
      **all-of-several** (two credentials sent together on one request).
- [ ] An operation that declares no requirement inherits the connector-level default, exactly as
      OpenAPI's document-level `security` works; an explicit empty list means "no auth" and must be
      distinguishable from "unset".

## Progress
- (not started)

## Notes
- Determinism matters beyond tidiness: `connectors.lock` hashes the IR (`C-7`), so nondeterministic
  serialization would produce phantom drift on every build.
- The IR must be expressive enough that a hand-authored TOML can define an operation with **no** spec
  at all — see `C-3`.
- **Babelforce is the live proof of the multi-auth requirement**, not a hypothetical: its spec
  (`~/babelforce/projects/babelforce-api/babelforce-api/openapi/manager.openapi.json`) declares
  three schemes — `oauth2` (password flow), `accessId` (`X-Auth-Access-Id`) and `accessToken`
  (`X-Auth-Access-Token`) — where the two apiKey headers must be sent **together** (AND) and are an
  **alternative** to OAuth2 (OR). Model it correctly here or C-10 cannot emit babelforce at all.
- Do not collapse a requirement set to a single purpose "for now". The AND case is the first one we
  ship, so a single-purpose shortcut would be wrong immediately.
