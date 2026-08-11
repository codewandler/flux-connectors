# Design: the spec front-end — `[spec]` + patches, proven by retiring manager-sdk

**Status:** proposed · **Pillar:** Spec · **Stories:** C-4, C-5, C-6, C-14, C-409 … C-418
**Epic:** `spec-front-end`

## Why now

[connector-pipeline.md](connector-pipeline.md) drew two front-ends over one IR: a hand-authored
provider TOML, and a `[spec]` pointer whose operations ingest pre-fills and a patch set corrects.
**Only the first was ever built.** All 53 providers are hand-authored; `crates/connector-cli/src/seam.rs:160`
refuses a spec-backed provider outright:

> `` `{}` points at the vendored spec `{}`, and compiling a spec-backed provider needs spec ingest
> (story C-4), which is not wired yet.

The *schema* landed with C-3 and has sat unused since: `SpecSource`, `Patch`, `OperationPatch` and
`ParamPatch` are complete, validated and round-tripped (`crates/connector-spec/src/provider.rs:73-176`),
with golden errors for `patch-without-spec` and `nothing-to-generate`. What is missing is the ingest
that fills the skeleton, the overlay that applies over it, and — this is the new part — **a way to
say something about 397 operations without writing 397 blocks.**

The forcing function is babelforce. `~/babelforce/projects/manager/manager-sdk` is a three-language
SDK over the same APIs `providers/babelforce.toml` describes nine of. The decision recorded with this
design is to **retire it**: consumers stop importing a per-language client and reach babelforce
through the generated connector — `connectors-api` over HTTP, or flux ops. That makes this repo the
single place the babelforce API surface is described, and it makes the `[spec]` front-end
load-bearing rather than aspirational, because nobody is hand-authoring 397 operations.

## What is actually there — measured, not assumed

Five vendored OpenAPI 3.0.3 documents in `manager-sdk/specs/`, one host between them
(`https://services.babelforce.com`):

| Document | Ops | Prefix | Security | 2xx schema | Component schemas |
|---|---|---|---|---|---|
| `manager.openapi.yaml` | 356 | `/api/v2` | root `oauth2`, **0 operation overrides** | 352/356 | 613 |
| `task-automation.openapi.yaml` | 31 | `/api/v3` | per-operation, `bearerAuth`+`oauth2` | 4/31 | 120 |
| `task-schedule.openapi.yaml` | 4 | `/api/v3` | per-operation | 0/4 | 86 |
| `user.openapi.yaml` | 4 | `/api/v2` | root `oauth2` | 4/4 | 25 |
| `auth.openapi.yaml` | 3 | `/oauth` | per-operation | 3/3 | 4 |
| **Total** | **398** | | | **363/398** | **848** |

`manager-sdk/COVERAGE.md` calls 397 of these canonical (one webhook receiver is excluded) and reports
397/397 wrapped in each of TypeScript, Go and Rust.

Further measurements that decide the design:

- **527 declared parameters** — 248 path, 232 query, 47 reaching their definition through a `$ref`.
- **128 operations declare a request body.** Of the media types declared inline: 97 `application/json`,
  5 `multipart/form-data`, 3 `application/x-www-form-urlencoded`.
- **214 operations mutate** — 94 POST, 65 PUT, 54 DELETE, 1 PATCH.
- **23 operations carry no `summary` and no `description`**, so 23 tools would reach a model nameless.
- **Every operation carries an `operationId`** (398/398).
- **Tags are useless as a namespace.** `Manager` covers 309 of the manager document's 356 operations.
  The SDK's 36 hand-written resource modules are not derived from tags — and **47 distinct
  three-segment path prefixes** (`/api/v2/agents`, `/api/v3/tasks`, …) reproduce that grouping almost
  exactly. **Services derive from the path, never from tags.**

## The mapping

**One connector, five services.** `providers/babelforce.toml` keeps its identity — `authority =
"com.babelforce.api"`, one `base_url` — and gains a service per document. This is the member model
C-66 already landed (provider → service → members, one shared name namespace per service); the two
API versions (`/api/v2`, `/api/v3`) live in the path, exactly as the nine current operations already
carry `/api/v2/` in theirs.

That immediately breaks an assumption in the CLI: `Provider::spec()` returns **the last spec by
version order** (`crates/connector-cli/src/discovery.rs:39`), and `SpecSource.path` is a single
string. One document per provider was never stated as a limit — it was assumed. C-410 lifts it.

**Specs are YAML.** The cache is extension-agnostic already (`discover_specs` takes the file stem as
the version), but ingest must parse YAML as well as JSON. `serde_norway` 0.9.42 is already in
`Cargo.lock` through the flux crates, so this costs no new lockfile entry.

## The manifest changes — where the boilerplate actually is

`OperationPatch` selects **one** operation by `operationId` and renames it. For 397 operations that
is 397 `[[patch.operations]]` blocks, each with a `select`, a `rename`, a `risk` and an
`idempotency` — before any real correction. Hand-authoring the same thing inline is worse: today's
nine operations cost 533 lines of TOML, and 527 parameters at five lines each puts the honest
estimate for 397 north of 6,000 lines restating what the specs already say.

So the overlay grows four declarations, each of which is one statement about many operations:

### 1 · A selector that matches a set (C-411)

```toml
[[patch.select]]
service = "manager"
path_prefix = "/api/v2/agents"
methods = ["GET"]
```

Selection stays **opt-in** — that is why `Patch` has no `hide` and must not grow one. A selector
widens what one statement can select; it does not make anything default-selected. A selector that
matches nothing is a loud error, for the same reason `OperationPatch::select` naming an absent
operation already is: that is how config rots.

### 2 · A naming rule instead of 397 renames (C-412)

Op ids are a public contract, which is why `rename` exists and why `operationId` must not silently
become one. The answer is not to abandon the rule but to **declare it once**:

```toml
[patch.naming]
rule = "kebab"                     # listReportingCalls -> babelforce-list-reporting-calls
prefix = "babelforce"
[patch.naming.pin]                 # the escape hatch, and the only per-op naming cost
listAgents = "babelforce-agent-list"
```

Two requirements make this safe: **collisions refuse** (two operationIds deriving one op id is an
error, not a last-write-wins), and a test asserts the derived set is **stable across regeneration** —
a spec that renames an `operationId` upstream must move the op id loudly, not quietly.

### 3 · Callable without being a tool (C-413)

`expose: true` is hard-coded in the emitter — `crates/connector-flux/src/op.rs:791` and
`graph.rs:1182`. Every emitted op is an LLM tool. 397 tools is not a catalogue, it is a denial of
service against the model's context, and it is the single reason
`docs/designs/provider-operation-inventory.md` §5.2 curated 9 out of 163 in the first place.

Curation was the right answer while the connector was the *only* surface. It is the wrong answer for
a connector that must serve every caller manager-sdk served. So the two claims separate:

- **catalogued and callable** — the operation exists, `connectors-api` will run it, it appears in
  `catalog.json` and the manifest;
- **exposed** — it additionally reaches a model as a tool.

`expose` defaults to `true`, so no shipped artifact moves. Babelforce declares the inverse: the
curated set stays exposed, the other ~388 are callable and unexposed. This is also the distinction
C-235 needs and cannot currently express.

### 4 · Risk and idempotency by selector, with silence refusing (C-414)

Specs publish neither, so every authored write needs both stated. Stating them
per-operation is 214 blocks; deriving them from the HTTP method is the failure mode this repo has
already legislated against twice (`Risk` has no `Default`; C-186 made `Conditional` state its
condition or not build).

The resolution keeps the grain: a selector may state `risk` and `idempotency` for the set it matches,
and **silence on an authored write refuses the build** rather than defaulting to `low`. A default that
flatters is worse than no default; a default that must be overridden to *lower* risk is safe. So an
unstated DELETE does not compile, and a selector saying `risk = "destructive"` over 54 DELETEs is one
reviewable line instead of 54.

### 5 · A helper that writes the statements (C-419)

The four declarations above reduce how much has to be *said*. They do not reduce who has to say it,
and at 397 operations that is still the binding constraint — **owner-stated 2026-08-01: the point is
to adjust the helpers and the manifest layout so referencing a document is easy, and then rebuild the
suite from there.**

So `connector-cli scaffold <provider>` reads the vendored document and emits the provider TOML that
references it — `[spec]` block, selectors, naming pins, and per-operation blocks for what a selector
cannot cover. To **stdout**, never over a file in place: the author diffs and pastes, so a bad run
costs nothing and the reviewed artifact is still a human's.

Two rules keep it honest, and they are the same rule twice:

- **What the document cannot state comes out as a hole, not a guess.** `risk` and `idempotency` emit
  as an explicit `TODO` that the loader refuses (§4). A scaffold that silently declares 54 DELETEs
  `low` has not saved anyone work, it has manufactured 54 unreviewed safety claims.
- **What it could not carry is reported, per operation and by count.** A dropped operation that
  produces no output reads as "the vendor does not offer that".

`--diff` is the other half and the one that matters over time: compare the document against the
connector as it stands, report what upstream added, removed or changed. That is what makes a
**re-build** a repeatable operation rather than a one-time migration.

### 6 · Quirks once per service (C-415 covers the babelforce values)

The manager document paginates uniformly (`page`/`max`, as the nine current operations already
declare). Attaching that per operation is 356 repetitions of one fact.

## Loading a provider file, once the front-end is real (C-421)

The front-end being real changes what "load a provider" *means*, and the epic did not cost that.
`provider::load` takes bytes and no spec cache, so it cannot compile a spec-backed file — and until
C-421 it returned `Ok` with a **skeleton** anyway: id, base URL, credentials, provenance, zero
operations. Ninety-one files call it, eighty-six of them tests, and C-416 measured the consequence of
the first shipped provider converting: **53 tests across 18 binaries in 4 crates**, every one of them
green beforehand over a connector it believed it had checked.

**The decision: the pure entry point stays pure and refuses.** `load` on a file that pins a `[spec]`
is an `InvalidProvider` naming the pinned documents and naming `load_with_spec`. The alternative —
folding the cache into `load` as a parameter, so "load" has one meaning everywhere — was rejected on
what it does to the callers who have no cache, which is most of them and every unit test that authors
its own TOML. The only argument they can pass is an empty slice, and an empty slice against a pinned
`[spec]` already refuses one layer down in `ingest_specs` ("names no vendored document"). So the
parameter buys one *signature*, not one meaning; the second meaning is just spelled `&[]`, and it
lands as a vestigial argument on roughly forty golden-error tests that will never own a document.

**The second half is what actually makes conversion cheap, and it is the part C-417 and C-420
depend on.** The test suite had no shared way to load a shipped provider — eighteen binaries, each
with its own loader — so the convention "read `providers/x.toml`, call `provider::load`" was
replicated everywhere and was wrong everywhere at once. There is now one:
`crates/connector-spec/tests/support/shipped_provider.rs`, `#[path]`-included by the three crates
that need it, which reads the definition **and every document under `specs/<name>/`** and calls
`load_with_spec`. The rule it states is one sentence — *bytes read from `providers/` go through the
helper; TOML a test wrote itself goes through `provider::load`* — and the consequence is that a
provider converting to `[spec]` needs **no test change at all**.

Measured on C-416's branch: the 53 failures fall to **2**, and both survivors are C-126's
coordinator-fenced ratchet constants moving because babelforce goes 0/9 → 9/9 on response schemas.

## Vendoring and provenance

The specs are not secret — the vendor's developer hub renders them publicly. What is internal is
the **fetch configuration**: `manager-sdk/specs/sources.json` holds the internal forge host, project
ids and the source repository path, and `manager-sdk/scripts/leak-markers.regex` names those same
strings as high-confidence markers that must never be published. flux-connectors is a public repo
(`github.com/codewandler/flux-connectors`), which is why this paragraph **describes** those strings
rather than quoting them: a document explaining which values must never be published is a poor place
to publish them, and it did exactly that until C-532.

So the split is: **the pulled bytes are vendored here; the pull configuration is not.** `SpecSource`
already accommodates it — `source_url` is `Option`, and `sha256` plus `upstream_version` carry
provenance without naming an internal host.

`manager-sdk/specs/scripts/pull.sh` already normalizes `servers:` to the public production host and
applies four generator-compatibility fixes before anything is committed; the vendored form is that
output. Two things it does **not** do, and C-415 must:

- **The credential-shaped examples survive it.** `user.openapi.yaml:293` and
  `manager.openapi.yaml:25935` carry an `accessId` UUID and a 32-hex `accessToken` for a
  `Testers Inc.` account. `leak-markers.regex` deliberately excludes `X-Auth-Access-*` because the
  *header names* are public — the example *values* are a different question, and the blocker
  `providers/babelforce.toml:7-15` records is exactly this one. They belong to the deprecated header
  pair this connector already refuses to model, so scrubbing them costs nothing here; confirm-and-rotate
  upstream is worth asking for separately and is not a gate on this work.
- **`info.version` is `0.0.0-dev`** on three of the five documents, so the spec cache cannot take its
  version from the document. C-415 names the vendored files by pull date instead, and records the
  real identity in `sha256`.

## What retiring manager-sdk actually requires

Three gaps stand between "the connector describes 397 operations" and "a manager-sdk caller has
somewhere to go". Naming them now is cheaper than discovering them at the migration:

1. **`multipart/form-data` is inexpressible.** `BodyEncoding` is `Json | Form`
   (`crates/connector-spec/src/ir.rs:221-231`). Five file-upload operations cannot be emitted at all.
2. **`application/x-www-form-urlencoded` is declarable but non-functional** — the form and query
   encoder is upstream flux work (`L-101`), which `docs/roadmap.md:26` already names as what keeps
   every `form` body non-functional. The three affected operations are the OAuth token endpoints,
   which is to say **the login flow is the thing that is blocked**. That connects to C-135/C-136:
   a credential-producing operation must return a handle, not the token.

   The encoder is not what actually blocks those three, and a reader of this list should not infer
   that landing `L-101` unblocks them. They are **withheld by rule**, and the rule is stated once, in
   [`AGENTS.md`](../../AGENTS.md) § Authentication contract: an authentication endpoint is never a
   connector operation, and — separately and more generally — an operation whose declared response
   carries a token is withheld until C-136's diversion lands. C-430 made that second test something
   the build enforces, reading an operation's own `credential_response` declaration rather than
   guessing from a field name; that section is the statement of the rule and this paragraph does not
   restate it.
3. **23 operations have no description.** A tool contract with no sentence in it is not a tool
   contract. Either the overlay supplies them or those operations stay unexposed.

None of the three blocks the ingest work; all three block the *claim* that the SDK can be archived.
C-418 holds that claim and must not tick until they are resolved or explicitly scoped out.

## Sequencing

```
C-4  ingest  ──┬── C-410 many documents ──┬── C-6 overlay ──┬── C-411 bulk select ──┐
               │                          │                 ├── C-412 naming rule ──┤
C-415 vendor ──┘   C-5 auth extraction ───┘                 └── C-414 risk/idem ────┤
C-413 exposure tier (independent of all of the above) ──────────────────────────────┤
                                                                                    ▼
                                                                    C-419 scaffold helper
                                                                              │
                                            C-416 reproduce the 9 ── C-417 widen to 397 ── C-418 retire
                                                                          C-14 drift ──────────┤
                                                                    C-420 rebuild the suite ───┘
```

C-4, C-413 and C-415 had no edge between them and were the first wave; all three landed 2026-08-01.

**The shape of the whole thing, in one line:** the manifest layout (C-410 – C-414) says a lot with
few statements, the helper (C-419) writes those statements from the document, and the suite is then
rebuilt through both (C-417 for babelforce, C-420 for everyone else). Each of the three is close to
useless without the other two — a terse manifest nobody can generate is still an authoring job, and a
generator emitting a verbose manifest produces something nobody can review.

## Alternatives considered

- **Emit typed clients here (Rust/TS/Go) as new IR backends, so manager-sdk becomes generated
  output.** Rejected by the owner on 2026-08-01: it makes this repo own three language toolchains and
  a publishing story per language, to serve callers who can call the host instead.
- **Keep manager-sdk and feed it a published IR.** Rejected with the same decision — it keeps two
  descriptions of one API alive, which is the drift this repo exists to end.
- **Curate babelforce to a few dozen operations and leave the SDK for the rest.** This is the status
  quo (9 of 163) and it is why the SDK exists. Partial coverage means every caller must know which
  half it is in.
- **Derive `risk` or direction from the HTTP method.** Rejected: unverified claims, each of which a host reads
  as a licence. See C-186 for the same argument made about `Conditional`.
- **Tags as services.** Disproved by measurement — `Manager` covers 309 of 356.
