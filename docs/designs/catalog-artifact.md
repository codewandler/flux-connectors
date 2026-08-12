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

**Decided by C-537 (2026-08-12; the working choice below is rejected in §2.3).** The canonical
documents compile into `crates/catalog-reader/catalog.pack`: one **uncompressed, offset-indexed
UTF-8 container over the committed document bytes**, written by `connector-cli`'s `pack` module on
every full build and embedded by the reader crate. The fixed container properties all hold — one
file, versioned schema, embedded digest, deterministic bytes, mmap-friendly reads (the embedded
bytes are served from the binary's own mapping and records are sliced in place), no network and no
filesystem walk at query time.

#### 2.1 The container format (version 1)

```text
flux-connectors-catalog-pack 1                    ← magic + container format version
digest sha256 <64 lowercase hex>                  ← over every byte after this line
schema <n>                                        ← the documents' schema_version
providers <n>
operations <m>
p <id> <start> <len>                              ← one per provider, ordered by id
o <id> <provider> <service> <start> <len>         ← one per operation, ordered by id
payload <len>
<the canonical documents, concatenated in provider-id order>
```

Offsets are decimal byte offsets into the payload. A provider row's span is exactly its committed
`catalog/<id>.catalog.json` bytes; an operation row's span slices that operation's own JSON object
**out of its owning document** — the record a consumer receives is a substring of the reviewed
artifact, never a re-serialization that could disagree with it. The writer computes operation
spans with a string-aware structural scan and refuses to emit unless every span reparses
value-equal to the element `serde_json` finds at the same position
(`crates/connector-cli/src/pack.rs`); the reader (`crates/catalog-reader`) refuses, in order and
each by name, a wrong container version, a digest mismatch, a schema version it does not serve,
and any structural disagreement — all before serving a single record. Forward compatibility is
split the way the story requires: an **additive** change (an unknown header line, a new index-row
kind) is ignored by an older reader, because the digest already vouches for the bytes; anything a
reader must not ignore is a container-version bump, which an old reader refuses by name.
`connectors.lock` records the pack in a `[pack]` section — path, `schema_version`, and the
SHA-256 of the whole file — the one whole-catalogue artifact in the lockfile, because the pack is
the one that is *distributed*.

#### 2.2 Measured (2026-08-12, commands inline)

- Pack size: **9,547,465 bytes** (`wc -c crates/catalog-reader/catalog.pack`), against
  9,517,303 bytes of canonical documents (`du -sb catalog/`) — the index costs ~30 KB, ~0.3%.
- What compression would buy at rest: gzip -9 → 697,599 bytes, zstd -3 → 561,589, zstd -19 →
  336,537 (each `<tool> -c catalog.pack | wc -c`). The layers that actually carry the bytes
  already take it: the crates.io `.crate` is a gzipped tar, and git compresses blobs — so the
  uncompressed choice costs checkout bytes and consumer binary size (~9.5 MB of `.rodata`, paged
  in on demand from the mapped binary), not registry or wire bytes. The earlier reference figures
  re-measured: `tar c crates/catalog | gzip -9 | wc -c` → 185,629; `web/public/catalog.json` →
  12,487,271 bytes.
- Read cost (release, `measure_read_costs` in `crates/catalog-reader/tests/pack.rs`):
  `Pack::from_bytes` including full SHA-256 verification of the 9.5 MB file → **33.2 ms**, paid
  once per process; 20,000 indexed lookups → **1.01 ms** (~50 ns each, a binary search over the
  in-memory index). Query time parses no JSON and copies no record.
- Determinism: two builds in independent trees produce byte-identical packs
  (`sha256sum` → `7670fe86…` both times; `tests/catalog_pack.rs::the_pack_is_byte_deterministic`
  pins the property, and the committed pack is held to a fixed point of a build).

#### 2.3 The working choice, rejected in writing

The zstd-compressed canonical CBOR container this section used to propose is **rejected**, on four
grounds, any one of which suffices:

1. **It contradicts the reader's own acceptance.** The reader ships with zero non-optional
   dependencies, and the *embedded* pack must be readable by the default, no-feature build — so
   both the zstd decoder (C bindings, or a pure-Rust implementation larger than this whole crate)
   and a CBOR codec would be non-optional dependencies in every consumer's tree. Making
   compression an optional feature fails the same test from the other side: the shipped payload
   must be readable without the feature, so it must be uncompressed anyway and the feature would
   compress nothing anyone reads. Hand-vendoring a DEFLATE decoder was also considered and
   rejected: ~400 lines of bit-level decoding in the credential supply chain to buy a size
   reduction the transport layers above already provide.
2. **Compressed bytes are not a pure function of committed inputs.** zstd output depends on the
   compressor's version and parameters, so a toolchain bump would rewrite the artifact with no
   input changing — exactly the phantom-drift class `connectors.lock`'s "unchanged inputs
   reproduce the file byte for byte" rule exists to rule out. The raw document bytes have no such
   dependency.
3. **A CBOR re-encoding forfeits reviewability.** The pack would carry a second serialization of
   the reviewed artifact rather than the reviewed bytes themselves; the byte-identity now asserted
   between every record and its committed document
   (`catalog_pack.rs::a_full_build_derives_one_pack_from_the_canonical_documents`,
   `catalog-reader/tests/pack.rs::every_embedded_record_agrees_with_its_canonical_document`)
   would be unstatable.
4. **Compression defeats the mmap-friendly read the properties require.** An offset index into a
   compressed payload still costs inflating it to the heap before the first record; the
   uncompressed container serves every record as a zero-copy slice of the embedded (or loaded)
   bytes.

What the rejection costs, measured above: ~9.5 MB in a checkout and in a consumer binary's mapped
(not resident) data, in a repository whose committed `catalog/` documents and `catalog.json`
already total ~22 MB. The digest stays SHA-256 — the one hash spelling the repository records
anywhere — vendored into the reader (~120 lines, FIPS 180-4 vectors plus a `sha2` cross-check in
tests) rather than imported.

#### 2.4 The reader and the shim, as delivered by C-537

The dependency-free reader crate is `codewandler-connector-catalog-reader` (lib `catalog_reader`).
It embeds the pack and exposes `providers()`, `provider()`, `operation()`, `operations_of()` over
it — records are canonical JSON *text* plus the indexed facts (id, provider, service), because
interpreting a document is the resolver's job (C-538), not the container's. Hosts that want the
file at a path (Exchange loading a newer catalogue than it was built with) get `Pack::load`, which
verifies format version, digest and schema version before serving a single record.
`codewandler-connector-catalog` now depends on the reader and re-exports it whole as
`catalog::reader` — the shim, taken additively, so its public API is unchanged
(`crates/catalog/tests/consumer_api.rs` compiles the promised surface as a consumer;
`publish_closure.rs` holds the publishing half, with the reader joining the derived closure
through the new edge).

**The generated Rust in `crates/catalog/src/generated/` remains the storage of the legacy API,
and its reduction is deferred to C-540 — deliberately, not silently.** The legacy API promises
`Operation::flux`, the emitted Flux text, as a public field, and `connector-pack` still parses it
(`spec.rs`); the canonical documents deliberately carry no Flux — the request template replaced
it. Serving the legacy tables from the pack today would therefore require the pack to embed the
very text this design exists to retire, and the differential gate in §"The differential gate"
still needs the Flux-derived plan intact to compare the document-derived one against. So the
tables stay until the gate holds and C-540 deletes the emitter, at which point they reduce to the
embed + re-export in the same change that retires `.flux`.

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

- ~~Exact container format and index layout (C-537).~~ **Settled 2026-08-12** — §2.1 above is the
  record, §2.3 the rejection of the prior working choice.
- Whether `graphs` lower to a declarative plan in the document or stay unpublished (deferred; no
  shipped provider declares one — `grep -l '\[\[graphs\]\]' providers/*.toml` matches nothing).
- The `.connector.toml` projection's retirement date (owned by flux/D-214, not this repo).
