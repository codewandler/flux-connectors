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

## Progress
- (not started)

## Notes
- Determinism matters beyond tidiness: `connectors.lock` hashes the IR (`C-7`), so nondeterministic
  serialization would produce phantom drift on every build.
- The IR must be expressive enough that a hand-authored TOML can define an operation with **no** spec
  at all — see `C-3`.
