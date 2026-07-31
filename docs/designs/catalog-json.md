# Design: `site/catalog.json` — the catalogue as data

> Story: [C-42](../stories/C-42-catalog-json.md) · Parent design:
> [public-docs.md](public-docs.md) · Emitter: `crates/connector-cli/src/site.rs` ·
> Status derivation: `crates/connector-cli/src/status.rs`

**A website is written against this file, so its shape is a contract.** This document specifies it.
Anything not written down here is not promised.

## Why the file exists at all

The public site must never hand-maintain catalogue data. That is the action-proxy failure this whole
repository exists to correct, re-enacted in JavaScript — and it is the risk
[public-docs.md](public-docs.md) names first: *"if anyone hand-edits catalogue data into a `.vue`
file, the project has lost the argument it was founded on."*

So `catalog.json` is the **fourth backend over one IR**, alongside the three that already exist:

```
                                  ┌─► connectors/<p>.flux           (installable)
providers/*.toml ─► Connector IR ─┼─► connectors/<p>.connector.toml (manifest)
                                  ├─► crates/catalog/…              (Rust consumers)
                                  └─► site/catalog.json             (the website)
```

It is generated, committed, and drift-checked exactly like every other artifact:
`crates/connector-cli/tests/site_catalog.rs` recomputes it from `providers/*.toml` and fails when the
committed bytes differ.

## Where it lives, and when it is written

`site/catalog.json`, at the repository root.

Outside `connectors/` deliberately: that directory holds what a user *installs* into `~/.flux/flows`,
and a JSON document a browser fetches is not that. It is a **sibling** of the site's own tree rather
than a directory inside it, so the site's tooling owns its build and this pipeline owns the data —
which is what keeps a Node build from having to run before a Rust one.

`cargo run -p connector-cli -- build` writes it. **`build --provider <name>` does not**: the document
covers every provider at once and a scoped run compiles only one, so writing it from a scoped run
would silently truncate the catalogue to the provider that happened to be built. A scoped run leaves
the committed document alone and does not report it stale.

**C-104 generalised this rule.** It was stated here for one document and reached independently, from
the other direction, by `crates/catalog/src/generated.rs` — which paid for it by keeping its provider
index *by hand*, and so became the one file every provider story had to append to. C-104 made that
index generated under this same full-run rule, and named the resulting class in `AGENTS.md`:
**whole-catalogue artifacts**, written by a full build only and owned by the coordinator. That is
what lets provider stories run in parallel, because it makes two implementors' write sets disjoint.

`crates/connector-cli/tests/catalog_index.rs` asserts the rule by comparing **whole-tree snapshots**
either side of a scoped build, rather than by checking a list of paths. The distinction is the point:
an enumerated list covers the members someone remembered to add, and stops covering the class the
moment the pipeline grows a fifth artifact. A snapshot comparison covers every member — the index,
this document, `web/public/v1/**` and the README SVGs — and covers a future one for free.

## Guarantees

1. **Every key is always present.** An absent value is `null` or `[]`, never a missing key. A
   consumer types the document once and never tests for existence.
2. **Deterministic.** Rebuilding from unchanged inputs is byte-identical. Every value is a function
   of the IR, walked in the IR's own order, with no timestamp and no map iteration; the vendor
   schemas carried verbatim are `serde_json::Value`, which is `BTreeMap`-backed and therefore
   key-sorted.
3. **No credential value, ever.** `env` and `user_env` name environment *variables*. Nothing in the
   emitter reads the process environment.
   `site_catalog.rs::no_credential_value_reaches_the_document` runs a real build with a credential's
   variable set to a sentinel and asserts the sentinel is nowhere in the output.
4. **Pretty-printed with a trailing newline**, because this is a committed artifact a human reviews
   in a diff.

## Versioning

`schema_version` is `2`.

It is bumped **only** when an existing field changes meaning or disappears. **Adding a field does not
bump it**: every consumer reads by name, so a new key is invisible to one that does not know it.

That rule has already paid twice. [C-49](../stories/C-49-provider-services.md) added `services` and
`authority` to every provider and `service` to every operation without bumping it. When
[C-37](../stories/C-37-global-addressing.md) lands, a provider gains `pid` (`com.zendesk.api`) and an
operation gains `oip` (`com.zendesk.api/support:v2#show`) the same way — **new fields on the existing
objects**. Nothing reshapes. That is also why every entity here is a JSON *object* and
never a tuple or a positional array.

## The shape

### Document

| Field | Type | Notes |
|---|---|---|
| `schema_version` | number | See above. |
| `generator` | string | `flux-connectors <version>` — the same identity every other artifact's header carries. |
| `documentation` | string | Path to this file, so the document is self-describing. |
| `providers` | array\<Provider\> | Ordered by `id`. |

### Provider

| Field | Type | Notes |
|---|---|---|
| `id` | string | `zendesk`. Names `connectors/<id>.flux`. |
| `authority` | string \| null | The reverse-DNS authority the provider publishes under (`com.amazonaws`), or `null` when it declares none — every provider shipped today. |
| `vendor` | string | Display name. |
| `description` | string | One line. |
| `base_url` | string | Templating included: `https://{subdomain}.zendesk.com`. A service may override it. |
| `api_version` | string \| null | The vendor's API version, as the default for this provider's services. |
| `hosts` | array\<string\> | Hosts reached, templating intact. An array because C-10's `http_hosts` allowlist will hold more than one. |
| `services` | array\<Service\> | The provider's API surfaces (C-49). **Always at least one**, so a consumer groups by service unconditionally rather than special-casing the providers that have not been split. |
| `auth` | Auth | See below. |
| `operation_count` | number | So a provider list renders without walking `operations`. |
| `operations` | array\<Operation\> | In the order the provider declares them, which is the order `connectors/<id>.flux` carries them. |

### Service

One API surface of a provider: the unit a consumer addresses, versions and installs
([C-49](../stories/C-49-provider-services.md), `provider-services.md`). Every operation belongs to
exactly one, and the services partition the operation set — so the per-service `operation_count`s sum
to the provider's.

| Field | Type | Notes |
|---|---|---|
| `name` | string | `s3`, or `default` for a provider with a single API surface. |
| `description` | string | One line. Empty for the implicit `default` service, which has no declaration to carry one. |
| `base_url` | string | The URL calls to this service reach — its own override, else the provider's. |
| `hosts` | array\<string\> | The hosts those calls reach. A service's egress surface is its own and is never widened to the union of the provider's. |
| `api_version` | string \| null | The vendor's version for **this** service. AWS dates `s3` at `2006-03-01` and `bedrock-runtime` at `2023-09-30`. |
| `gid` | string \| null | The service's rendered address, `com.amazonaws/s3:2006-03-01`, or `null` when the provider declares no authority or version. `default` is **elided** from it, never spelled out. |
| `operation_count` | number | How many operations belong to it. |

### Auth

| Field | Type | Notes |
|---|---|---|
| `schemes` | array\<string\> | The distinct scheme kinds in play, in declaration order — the "auth scheme" a provider list shows. `[]` when the connector declares no credential. |
| `credentials` | array\<Credential\> | Every declared credential. |
| `default` | array\<array\<string\>\> | The connector-wide default requirement. See **Credentials are OR-of-AND** below. |

### Credential

| Field | Type | Notes |
|---|---|---|
| `name` | string | `zendesk.api_token` — what an operation's requirement references. |
| `scheme` | `{kind, name}` | `kind` is `bearer` \| `basic` \| `header` \| `query`; `name` is the header or query-parameter name, `null` for the two variants that carry none. |
| `description` | string | For the prompt that asks an operator to supply it. |
| `env` | array\<string\> | Environment variable **names**, tried in order. Never a value. |
| `user_env` | array\<string\> | For `basic`: variable names holding the username half. |
| `user_suffix` | string \| null | For `basic`: a literal appended to the resolved user value — Zendesk's `/token` marker, which is public API syntax and not a credential. |
| `oauth2` | boolean | Whether the host runs token grants for this credential. |

`scheme` is flattened to a fixed two-key object rather than mirroring the IR's externally tagged
encoding (`"bearer"` for one variant, `{"header": {"name": "…"}}` for another). A JSON shape that
changes with its value would force every consumer to write a discriminated union to read it.

### Operation

| Field | Type | Notes |
|---|---|---|
| `id` | string | `zendesk-ticket-search`. The Flux symbol; unique across the catalogue. |
| `provider` | string | The owning `Provider.id`. |
| `service` | string | The owning `Service.name` — exactly one, `default` for a single-surface provider. This is the grouping a consumer wants once a provider is more than one API. |
| `description` | string | The same text a model sees as the tool description. |
| `risk` | string | `low` \| `medium` \| `high` \| `destructive` — flux's own vocabulary. |
| `idempotency` | string | `idempotent` \| `non_idempotent` \| `conditional`. |
| `method` | string | Uppercase HTTP method. |
| `path` | string | Template, relative to the provider's `base_url`. |
| `parameters` | array\<Parameter\> | See below. |
| `input_schema` | object | **One JSON Schema for everything the operation receives**, composed from `parameters` and `body_schema` (C-125). Never null: an operation that takes nothing composes `{"type": "object", "properties": {}, "required": []}`, because "takes nothing" is a derived answer. Keyed by the caller-facing `name`; `required` is exactly the required parameters. |
| `body_schema` | object \| null | Set when the body **is** a schema rather than assembled from named fields (babelforce's free-form session bodies). Mutually exclusive with `in: "body"` parameters — an operation declaring both does not load. |
| `response_schema` | object \| null | The vendor's success schema, when it publishes one. |
| `credentials` | array\<array\<string\>\> | See **Credentials are OR-of-AND**. |
| `hosts` | array\<string\> | The hosts this call reaches — its **service's**, which for a multi-service provider is not the provider's. |
| `flux` | string | **The generated Flux, verbatim** — byte for byte the `op` declaration `connectors/<provider>.flux` carries for this operation. |
| `status` | Status | See below. |

### Parameter

| Field | Type | Notes |
|---|---|---|
| `name` | string | Caller-facing: what the Flux op declares and what a model passes. |
| `in` | string | `path` \| `query` \| `header` \| `body`. |
| `wire` | string \| null | The spelling the **vendor** sees when it differs — a body field's dotted JSON path (`ticket.comment.body`), or a plain alias for a path/query/header parameter (`req_id` → `requester_id`). |
| `description` | string | Surfaced to a model as part of the op's contract. |
| `required` | boolean | Whether the vendor requires it. |
| `schema` | object | **The vendor's JSON Schema, verbatim**, constraint keywords included. Flux's declaration narrows this to `Any`/`Bool`/`Number`/`String`/`List<T>`; nothing is lost here. |

Parameters are one flat list carrying their own position, not four keyed groups. The position is a
*property* of a parameter, and a flat list renders both the ordered signature and a per-position
grouping; grouped keys would force the ordered view to be reassembled. The order is the IR's own —
path, query, header, body — which is also the argument order of the Flux declaration.

### Credentials are OR-of-AND

`credentials` and `auth.default` are **alternatives (OR) of mechanisms (AND)**, and flattening them
would be wrong in both directions:

```json
[["babelforce.access_id", "babelforce.access_token"]]
```

is **one** mechanism needing **two** credentials on the same request — not two ways to authenticate.
babelforce is why the shape exists: its two api-key headers travel together and are an alternative to
OAuth2. `[]` means the operation needs no credential at all.

The names are credential *references*. Resolve one against `provider.auth.credentials[].name`.

## `status` — the field the file exists for

> An operation that does not currently work says so, prominently, wherever it appears.
> — [public-docs.md](public-docs.md), Acceptance

| Field | Type | Notes |
|---|---|---|
| `works` | boolean | Exactly `issues.length === 0`, restated so a consumer can filter on one boolean without knowing any code. A `note` does not enter into it. |
| `issues` | array\<Issue\> | Every reason it does not. |
| `notes` | array\<Note\> | Facts that are **not** reasons it fails. `[]` for every operation shipped today. |

### Issue

| Field | Type | Notes |
|---|---|---|
| `code` | string | A stable machine token. See the table below. |
| `scope` | string | `catalog` \| `provider` \| `operation`. |
| `story` | string | The story that closes it. |
| `summary` | string | One line, renderable as-is. |
| `params` | array\<string\> | The parameters implicated, by their **wire** name; `[]` when the issue is not about parameters. |

### Note

| Field | Type | Notes |
|---|---|---|
| `code` | string | A stable machine token, from the same namespace as an Issue's. |
| `scope` | string | `catalog` \| `provider` \| `operation`. |
| `summary` | string | One line, renderable as-is. |

No `story` and no `params`: a note is not something anyone is going to close, and none of them is
about parameters. Both would be permanently empty fields inviting a consumer to look for meaning in
them.

**`scope` is what keeps the field useful.** An explorer that says "0 of 25 operations work" is
accurate and useless. `scope` separates a defect the operation owns from one it merely inherits:
`zendesk-ticket-search` has a problem nothing else in the catalogue has, while every authenticated
operation everywhere waits on the same seam. A consumer filters on `scope` without having to know the
codes.

### The codes, and how each is derived

Every one is a **rule applied to the IR**, not an entry in a list. A hard-coded list of broken
operation ids is exactly the hand-maintained truth this story exists to avoid: add a fourth provider
with a free-text query parameter and it is flagged with no edit here, and closing the underlying gap
clears the flag from every operation at once.

| `code` | kind | `scope` | Story | Derived from |
|---|---|---|---|---|
| `no-credential` | issue | provider | C-17 | `Connector::effective_auth(op)` is **empty** and the operation did not declare it so — a credential exists and cannot be held safely yet, so the request would go out unauthenticated. |
| `credential-not-injected` | issue | catalog | C-10 | `Connector::effective_auth(op)` is **not** empty. The operation names its credential, but the generated Flux does not yet attach it. |
| `unencodable-query-value` | issue | operation | C-30 | Any query parameter whose JSON Schema `type` is not `integer`, `number` or `boolean`. |
| `unbound-base-url-template` | issue | provider | C-17 | `base_url` contains a `{name}` placeholder. |
| `no-credential-required` | **note** | operation | C-206 | `Operation::auth` is `Some([])` — the author **declared** that the vendor needs no credential. |

Together the four issues reproduce, per operation and from the IR alone, the four entries README.md
publishes under **Known limits**.

The first two are **complementary** over every operation that has not declared itself public: one of
them fires. That is the machine-readable form of *"no provider can make a live call yet, and
freshdesk cannot even name the credential it would need."*

### Withheld is not the same as unnecessary (C-206)

An empty effective auth carries **two opposite meanings**, and `no-credential` used to be published
for both:

- **withheld** — a credential exists and this repository cannot hold it safely yet. Freshdesk is the
  case: its API key occupies the Basic *username* position, so `providers/freshdesk.toml` declares
  none deliberately and every request 401s. That is fail-closed behaviour working as designed, and
  `no-credential` keeps exactly this sense.
- **unnecessary** — the vendor requires nothing. Nothing is withheld and the unauthenticated call is
  the correct one. This is `no-credential-required`, and it is a **note** rather than an issue,
  because there is nothing wrong.

**The difference is declared, never inferred.** `Operation::auth` already separates the two empties,
as OpenAPI's operation-level `security` does: unset inherits the connector default, and an explicit
`auth = []` is *"the operation needs no auth at all: a health or ping endpoint"*. Inferring
"public" from the absence of a credential field would reproduce the conflation with more steps, and
would infer it wrongly for the one connector already shipping in that shape.

The declaration is per **operation**. A connector whose whole vendor is public repeats `auth = []` on
each of its operations, because `Connector::default_auth` is a plain list — `default_auth = []` and an
omitted key decode identically, so the connector cannot make the statement once. Making it able to is
a `connector-spec` change and is not part of C-206.

#### A public operation reports `works: true`, deliberately

`works` is `issues.length === 0` and a note is not an issue, so a declared-public operation with a
bound base URL and no free-text query parameter has **nothing left in the way** and says so. That is
a visible change for every consumer: this catalogue has published `works: false` for every operation
since it existed, and a site or a filter that treats `works: true` as unreachable will be wrong the
first time such an operation ships. Nothing ships one today — the change is stated here before it is
discovered.

It is also the honest answer. The `credential-not-injected` condition is about attaching a credential
to a request, and there is no credential to attach; the generated Flux for a public operation builds
its URL and calls `http.request`, which is the whole of what the endpoint needs.

`unencodable-query-value` follows [query-encoding.md](query-encoding.md) §4 exactly, including its
deliberate narrowness: a `Number` or `Boolean` value cannot contain `&`, `#`, `+` or a space, so the
six zendesk operations that take only numeric ids and page bounds are **not** flagged and the
connector reads as an honest 6/7. It inherits that design's recorded limit too — a free-form
parameter mistyped as `integer` in a provider TOML is still reported as working. Path and header
parameters have the identical gap and are deliberately not reported, because C-30 scopes the
emitter's refusal to query values and the catalogue must not disagree with the emitter.

### The one fact that is not derived

`credential-not-injected` needs to know whether the *emitter* attaches a declared credential to the
request. That is a property of `connector-flux`, not of any provider, so no walk of the IR can answer
it. It is one commented `const` — `CREDENTIALS_REACH_THE_REQUEST` in `status.rs` — rather than a list
of affected operations, and C-10 flips it in one line.

## Example

One operation, elided to the fields that matter:

```json
{
  "id": "zendesk-ticket-search",
  "provider": "zendesk",
  "service": "default",
  "risk": "low",
  "idempotency": "idempotent",
  "method": "GET",
  "path": "/api/v2/search.json",
  "parameters": [
    { "name": "query", "in": "query", "wire": null, "required": true,
      "schema": { "type": "string" } }
  ],
  "credentials": [["zendesk.api_token"]],
  "hosts": ["{subdomain}.zendesk.com"],
  "flux": "op zendesk-ticket-search(query: String, …) -> Any\n…",
  "status": {
    "works": false,
    "issues": [
      { "code": "credential-not-injected", "scope": "catalog", "story": "C-10", "params": [] },
      { "code": "unencodable-query-value", "scope": "operation", "story": "C-30",
        "params": ["query"] },
      { "code": "unbound-base-url-template", "scope": "provider", "story": "C-17", "params": [] }
    ]
  }
}
```

## Open questions

- **Where the site reads it from.** C-43 owns the site tree; this story owns the data and stops at
  `site/catalog.json`. Whether C-43 imports it directly, copies it into the site's public directory,
  or points a dev server at it is C-43's call — the only requirement is that it is not *copied by
  hand*.
- **C-31/C-32's markdown pages.** [public-docs.md](public-docs.md) proposes the site render the
  generated per-provider markdown rather than re-implement it. Those pages and this document are two
  views of the same IR; nothing here forecloses either.
- **Response schemas are empty today.** No shipped provider declares `response_schema`, so the field
  is `null` throughout. The key is present so the site can be written against it before ingest
  (C-4) fills it in.
