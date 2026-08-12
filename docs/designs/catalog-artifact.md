# Design: the catalog artifact

**Status:** accepted direction (Decision 0022, 2026-08-12) · **Scope:** compiler output, distribution, resolver input

The cross-family source of truth is
`../flux-roadmap/decisions/0022-connectors-compile-to-a-catalog-artifact.md`. This record applies
that decision to this repository: the build stops emitting Flux source and instead lowers the IR to
one canonical document per provider, compiles the documents into a compressed pack, and hands the
resolver (today's `connector-pack` assembly path) document data instead of Flux text to re-parse.
The epic is [C-534](../stories/C-534-catalog-artifact-epic.md).

## The defect this closes

Measured 2026-08-12, commands inline:

- `catalog::Operation` publishes no method and no path — its fields are `id`, `provider`,
  `service`, `direction`, `description`, `risk`, `idempotency`, `semantic_effects`, `credentials`,
  `credential_requirement`, `hosts`, `flux` (`grep 'pub [a-z_]*:' crates/catalog/src/lib.rs`, lines
  211–288). The emitted Flux text is the sole carrier of the verb and URL, recovered by
  `flux_lang::program::Module::parse_str` at `crates/connector-pack/src/spec.rs:250` and a closed
  seven-node AST walk in `crates/connector-pack/src/request.rs`.
- Exchange repeats the parse at runtime: `connector_pack::Rehearsal::of(.., entry.flux)` has four
  call sites in `../flux-exchange/crates/exchange-host/src/settings.rs` (416, 471, 1457, 3449),
  including connection verification.
- The round trip costs a full emitter (`crates/connector-flux`: 34,077 lines,
  `find crates/connector-flux -name '*.rs' | xargs wc -l`), 835 per-operation renderings
  (`ls crates/catalog/ops/*/ | grep -c '\.flux$'`), and 16,792 lines of generated Rust — while four
  declared surfaces (`roles`, `quirks.pagination`, `quirks.rate_limit`, `graphs`) reach no artifact
  at all because the op grammar cannot say them.
- Nothing executes the emitted Flux as Flux: `connectors/*.flux` has no non-test consumer in this
  repository or either sibling (see `docs/integrating-with-flux.md`, Path C).

## The artifact model

Three forms, one source of bytes:

```
providers/<name>.toml ──load/validate──▶ IR ──lower──▶ catalog/<name>.catalog.json   (canonical, committed)
                                                          │
                                                          ├──compile──▶ catalog.pack           (one file, embedded/distributed)
                                                          └──project──▶ catalog.json, *.connector.toml, explorer   (unchanged consumers)
```

### 1. The canonical document — `catalog/<name>.catalog.json`

One deterministic, byte-stable JSON document per provider. This is the reviewed artifact: committed,
diffed in PRs, hashed per provider in `connectors.lock` exactly as the `.flux` files are today. It
carries the complete published surface — the IR minus nothing:

- **Provider / services**: id, vendor, authority, runtime, per-service base URL (template preserved),
  api version, roles, tags, legacy markers.
- **Operations**, each with an explicit **request template** replacing the Flux body:

```json
{
  "id": "zendesk-ticket-update",
  "service": "default",
  "direction": "write",
  "risk": "medium",
  "idempotency": "non_idempotent",
  "effects": ["write", "network"],
  "expose": true,
  "auth": [["zendesk.api_token"]],
  "request": {
    "method": "PUT",
    "url": "{base}/api/v2/tickets/{ticket_id}",
    "headers": { "content-type": "application/json" },
    "query": [],
    "body": { "encoding": "json", "template": { "ticket": { "$param": "ticket" } } }
  },
  "params": [ { "name": "ticket_id", "position": "path", "required": true, "schema": { "type": "number" } } ],
  "response_schema": {},
  "quirks": { "pagination": null, "rate_limit": null, "error_envelope": null }
}
```

  The template language is **closed and total**: literal values, `{var}` interpolation over params
  and endpoint slots, `{"$param": name}` splices, nothing else. It is the data equivalent of what
  `request.rs` already accepts from the AST — anything the seven-node interpreter refuses today has
  no spelling in the template either.
- **Auth**: every credential with scheme, acquisition, placement, subject, hazard, user-half
  binding, and the **complete** `OAuth2Spec` (grants, paths, redirect, scopes) plus token-endpoint
  quirks — ending the `oauth2: bool` collapse. **Registration identity is not vendor truth**:
  `client_id`, `client_secret` and the redirect URI are per-deployment — both shipped OAuth2
  connectors carry `client_id: ""` in the generated catalogue
  (`grep 'client_id' crates/catalog/src/generated/{gitlab,babelforce}.rs`) while gitlab already
  declares the operator-level `oauth_client_id` config field binding `oauth.client_id`. The
  document publishes the registration **requirement** through that existing `binds` grammar and
  never a value; the vestigial `client_id` value field does not survive into the document.
- **Config**: every field with label/help/format/choices/level/approval and its `binds` targets.
- **`verify`**, **events**, **channel bindings** (transport, verification matrix, payload maps,
  reply, subscription/setup), **runtime bindings** (C-497 vocabulary as it lands).
- **Provenance**: the generator version and input hashes, as the manifests carry today.

The document schema is itself a versioned, published JSON Schema owned by this repository and
consumed by Exchange and Flux as a release (Decision 0008 contract rule). Additive evolution bumps
the minor; anything a consumer must act on bumps the major and is refused by older readers.

### 2. The pack — one file, read anywhere

The canonical documents compile into a single compressed, indexed, digest-named file (working
choice: zstd-compressed canonical CBOR with a provider/operation offset index; the format is C-537's
decision to finalise, the container properties are not: one file, versioned schema, embedded digest,
deterministic bytes, mmap-friendly reads, no network and no filesystem walk at query time). Size
budget is trivial — today's entire catalogue crate compresses to ~186 KB
(`tar c crates/catalog | gzip -9 | wc -c` → 185,704) and `web/public/catalog.json` is 12.5 MB of
mostly schemas that compress the same way.

A dependency-free **reader** crate exposes the pack behind the existing `catalog` API surface
(`providers()`, `provider()`, `operation()`, `operations_of()`), so `codewandler-connector-catalog`
becomes a shim embedding the pack and re-exporting the reader without breaking its public API.
Hosts that want the file at a path (Exchange loading a newer catalogue than it was built with) get
a `load` constructor that verifies schema version and digest before serving a single record.

### 3. The resolver — the pack minus the parse

`connector-pack` keeps its enforcement topology and its entire assembly path — credential
resolution ordering, checked redactor registration, scheme placement, endpoint substitution with
declared-authority validation, channel plans, and every fail-closed `Error` variant by name. What it
loses is `spec.rs`'s parse and `request.rs`'s AST walk: `build_request` reads the request template
off the document. `Rehearsal` is replaced by a document-backed equivalent with the same observable
semantics so Exchange's settings/verify paths migrate mechanically.

The semantics keep their contract; the **signatures shed the engine**. Today
`connector_pack::resolve` returns `flux_core::Result<Arc<dyn Tool>>`
(`crates/connector-pack/src/lib.rs:956`), which couples every consumer to one `codewandler-flux-*`
line even when no catalogue content changed. The plan-deriving core moves to an **engine-free**
crate — its `resolve` returns the request plan as data, the same unit the differential gate
compares, with secret-bearing fields on the `SensitiveText`/redacted-`Debug` pattern the channel
plan already uses. Dispatch and the `Tool`/`ToolSpec` projection are the consumer's: Exchange
depends on the engine directly for its own workflows and wraps the plan there.
`connector-pack`'s existing `resolve`/`project`/`pack` surface survives the migration as a thin
wrapper over the core so no consumer breaks mid-flight; the wrapper retires when Exchange adopts
the plan API (X-151 in `../flux-exchange`), and that deletion — owned by C-541, deliberately split
from C-540 so the `.flux` retirement cannot stall behind Exchange's merge queue — takes the
engine-line machinery (`crates/connector-cli/tests/flux_engine_line.rs`) with it in the same
change. A dependency-direction test pins the core's engine freedom the way `dependency_fence.rs`
pins the compiler's offline guarantee. One boundary the plan API must hold, named on the Exchange
side and worth stating here too: **projecting a plan into a Tool is not composing a request** — the
consumer wraps and dispatches the plan it was handed; a consumer that edits one has become the
second request path this family already rejected.

## Compatibility projections

- **`connectors/<name>.connector.toml`** continues to be emitted, generated from the canonical
  document, for as long as `flux-channels`' connector arm reads it (until flux/D-214 repoints
  inbound per Decision 0009). It becomes a projection, not a source.
- **`web/public/catalog.json`** and the explorer are fed from the canonical documents; the site's
  per-operation Flux snippet is replaced by the request template rendering.
- **`codewandler-connector-catalog`** stays published as the shim described above until Exchange
  and autodev consume the reader directly.

## The differential gate (migration rule)

Old and new derivations run side by side: for every operation in the catalogue (835 today,
`ls crates/catalog/ops/*/ | grep -c '\.flux$'`), the document-derived request plan must be
byte-identical to the Flux-derived one — method, URL, headers, query, body, `permission_subjects`,
and the registered redaction set. The gate covers the **configuration surface** too: the
document-backed `Rehearsal` equivalent must agree with the Flux-derived one — endpoint variables,
slots, caller path parameters — across the whole catalogue, because Exchange's settings and
connection-verification paths consume that surface and the request-plan comparison alone would not
prove it. (Exchange independently characterizes its current `Rehearsal`-derived behaviour before
the swap — X-152 in `../flux-exchange` — which needs nothing from this repository and is the
evidence "same semantics" is checked rather than trusted.) The gate is a workspace test, not a
promise. Only after it holds,
and Exchange consumes the reader/resolver release, does C-540 delete the emitter and the `.flux`
artifacts — deletion in the same release train as proven adoption, per Decision 0022.

## What is deleted at the end

`crates/connector-flux` (34,077 lines), `connectors/*.flux`, `crates/catalog/ops/**` (835 files),
the generated Rust in `crates/catalog/src/generated/` (replaced by the embedded pack), and the
parse-back halves of `connector-pack` (`spec.rs`, the AST walk in `request.rs`). The provider TOML
format, `connector-spec`, `connector-address`, `connector-secrets`, the lockfile, and the hermetic
committed-build discipline are unchanged.

## Invariants that survive unchanged

- No credential value in any input, artifact, lockfile or error (vision principle 4).
- Unchanged inputs reproduce every artifact byte for byte; the lock names which input moved.
- The build is hermetic and offline; `specs/` refresh stays in `scripts/`.
- Fail-closed everywhere the pack fails closed today, with the same error names.
- One name namespace per service; stable public op ids.

## Open questions (owned by child stories)

- Exact container format and index layout (C-537).
- Whether `graphs` lower to a declarative plan in the document or stay unpublished (deferred; no
  shipped provider declares one — `grep -l '\[\[graphs\]\]' providers/*.toml` matches nothing).
- The `.connector.toml` projection's retirement date (owned by flux/D-214, not this repo).
