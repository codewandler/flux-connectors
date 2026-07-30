# Design: a provider's services as the middle addressing level

**Status:** approved · **Pillar:** Spec · **Stories:** [C-49](../stories/C-49-provider-services.md)

## Why

A provider is not a usable tool catalogue, and the repository has already paid for that twice.

- **AWS is one provider and dozens of APIs.** `s3` and `bedrock-runtime` share an authority and
  nothing else: different hosts, different documentation, and — the fact that settles it — different
  API versions. `s3` is dated `2006-03-01`, `bedrock-runtime` `2023-09-30`. A single
  connector-level `api_version` cannot describe that provider at all.
- **A 163-operation provider cannot be installed whole.** C-18 curated babelforce down to a handful
  by hand. That cut was *editorial*: nothing in the IR recorded which slice of the vendor's API an
  operation belonged to, so the only way to install less than everything was to select operations one
  by one.
- **C-37's middle level had no owner.** Its `gid` — `com.zendesk.api/support/tickets:v2` — is a
  "versioned resource group" made of anonymous `Operation.path` segments. It is addressable but not
  declarable: nothing carries its description, its base URL or its version, and nothing says which
  operations constitute it. So "install the whole `support` group" was not a well-defined set.

This design promotes that middle level to a named thing with an owner: a **service**.

```
provider  ──►  service  ──►  operations
aws            s3            object-get, object-put, …
aws            bedrock-runtime  model-invoke, …
zendesk        default       ticket-show, ticket-comment-add, …
```

## Approach

### `Service` is an IR level, not a tag on the operation

`Connector` gains `services: Vec<Service>`; `Service` carries `name`, `description`, an optional
`base_url` override and an optional `api_version`. `Operation` gains `service: String`.

A **free-form `tags` field was considered and is rejected.** The three things this level has to do
are the three things a tag cannot do:

1. **Partition.** "Install the whole `s3` service" must denote a set. Tags overlap by construction,
   so tag membership answers "is this operation s3-ish", not "which operations are s3".
2. **Version.** `s3:2006-03-01` is a fact about the service. A tag is a label on an operation and has
   nowhere to put it, so the version would have to be repeated on every operation and could disagree
   with itself.
3. **Host.** A service may have its own `base_url`, and therefore its own egress allowlist. A tag has
   no fields at all.

A tag is metadata for search. This is structure for addressing, selection and safety.

### Exactly one service per operation

`Operation.service` is a single `String`, not a set. Three consequences, and each is the reason:

- **The gid stays unambiguous.** A set leaves "which segment renders?" undecided.
- **Selection partitions.** With one service per operation, "the s3 service" is set membership;
  with a set it is a filter, and the complement is no longer a service.
- **A genuinely shared operation is duplicated deliberately**, with two ids, which is visible in a
  diff — rather than resolved by an invisible rule.

The invariant the code holds, and `crates/connector-spec/tests/service_partition.rs` asserts: the
per-service operation sets are **pairwise disjoint** and their **union is every operation**.

### `service` unset means `default`, and `default` is reserved

`Operation.service` is a `String` with a serde default of `"default"`, not an
`Option<String>`. That is deliberate and it is this repository's standing rule about encodings: an
`Option` would give `None` and `Some("default")` as two spellings of one meaning, which is the same
objection `AuthRequirement` records against an empty mechanism inside a non-empty alternatives list.
The IR always carries a concrete service name; "unset" exists only in the *file*.

Two rules make the reserved name safe:

- **No `[[services]]` entry may be named `default`.** It is the name of the implicit service, so a
  declaration of it would be a second, contradictable definition.
- **A provider that declares any `[[services]]` must place every operation in one of them.** An
  operation that omits `service` in such a file lands in `default`, which no entry declares, and that
  is a loud error listing the services that *do* exist — following C-3's treatment of a duplicate op
  id. The alternative, an implicit `default` service sitting beside `s3`, would emit an
  `aws-default.flux` nobody asked for and would make the elision rule below incoherent.

So the service set of a connector is: the declared names, or exactly `["default"]` when it declares
none.

### The service is the first path segment of C-37's gid, and `default` is elided

```
pid   com.amazonaws                                   the provider
gid   com.amazonaws/s3:2006-03-01                     one service of it, versioned
oip   com.amazonaws/s3:2006-03-01#object-get          one operation
```

```
pid  := <authority>
gid  := <authority> [ "/" <service> ] ":" <api-version>
oip  := <gid> "#" <operation>
```

The bracket is the elision: a `default`-service gid renders as `com.freshdesk.api:v2`, never as
`com.freshdesk.api/default:v2`. `default` is an internal name for "this provider has one API
surface", and it must never reach a published address — an address is a promise, and that one would
have to be broken the day the provider grows a second service.

`parse(render(x)) == x` holds exactly, in both directions, for every `Pid`, `Gid` and `Oip`
(`crates/connector-spec/tests/service_partition.rs`).

### `api_version` belongs to the service, with the connector as its default

`Connector.api_version` is the fallback; `Service.api_version` overrides it. Resolution is
`Connector::api_version_of(service)`. A connector with one API surface states its version once; AWS
states one per service. Nothing in the IR requires a version at all — it is `Option` — and a gid is
renderable only for a connector that declares both an authority and a version, which is stated at
the accessor rather than papered over with a placeholder.

`Service.base_url` follows the same shape: an override, resolved by
`Connector::base_url_of(service)`, defaulting to the connector's.

### The emitted unit is the service

| Provider shape | Emits |
|---|---|
| `default`-only (all six shipped today) | `<provider>.flux`, `<provider>.connector.toml` |
| named services | `<provider>-<service>.flux`, `<provider>-<service>.connector.toml`, per service |

A `default`-only provider emits **exactly** what it emitted before this story, byte for byte. That is
not politeness; it is the regression proof that the whole reshape is meaning-preserving for the
existing catalogue, and it is asserted directly
(`crates/connector-cli/tests/service_units.rs::the_shipped_artifacts_are_byte_identical`).

Each manifest carries only its own service's operations and its own service's `base_url`, so a
service's egress surface is its own and is never widened to the union. The manifest's real
`http_hosts` allowlist is C-10's and does not exist yet; when it lands it derives from
`base_url_of(service)`, which is the per-service value this story introduces.

The catalog crate's unit stays the **provider** (`crates/catalog/ops/<provider>/`,
`generated/<provider>.rs`). Splitting it per service is a second reshape with its own churn and no
acceptance behind it; the service travels in `catalog.json` instead, where C-42's consumers can group
by it.

### Selecting a service

`build`/`diff` take `--service <NAME|GID>`, alongside `--provider`. A name selects the service of that
name; a gid selects by rendered address, which is what lets an external reference name a slice
without knowing our local provider spelling. An unknown service is a loud error naming the available
ones, exactly as an unknown provider already is.

### Service fields are inside `HashDomain::of`

`services`, `authority`, `api_version` and `Operation.service` are part of a connector's compiled
meaning — change any of them and a generated module moves — so they hash, unlike C-7's provenance.

They carry `skip_serializing_if` **inside the hash domain**, which is the one place this crate
otherwise avoids it. The reason is exactly the reason provenance is excluded: a connector that
declares no service, no authority and no version must hash to what it hashed before this story, or
every `connectors.lock` entry in the repository churns for a connector nobody edited — phantom drift,
which is the failure `connectors.lock` exists to rule out.

## Alternatives considered

- **A `tags: Vec<String>` on the operation.** Rejected above: cannot partition, version or host.
- **`Operation.service: Option<String>`.** Two encodings of one meaning. Rejected.
- **A service per *file* (`providers/aws/s3.toml`).** Tempting, and it would make the partition a
  filesystem fact. But then the authority, the credentials and the vendor description are repeated
  per service and can disagree, and a provider stops being one reviewable unit. The directory layout
  is C-41's bundle concern, downstream of this.
- **Keeping C-37's anonymous `path: Vec<String>` and calling the first segment "the service".** No
  owner, so no version, no base URL, no description, and no way to enumerate a service's operations
  without scanning every operation. That is the state this design replaces.
- **Emitting one module per provider with services as a comment.** Then "install the s3 service" is
  still not installable, which was the point.

## Risks & open questions

- **Elision plus variable depth is not context-free.** This story's gid grammar has exactly one
  optional middle segment, so parsing is unambiguous. When C-37 adds its remaining path segments,
  `com.freshdesk.api/tickets:v2` becomes ambiguous — `tickets` could be the service or a
  tail segment under an elided `default`. C-37 must resolve it explicitly, and the two candidate
  resolutions are: parse against the declaring connector's known service set, or forbid a tail on the
  `default` service. **Do not let C-37 land without choosing one**; the round-trip law depends on it.
- **`default` is a name a vendor could plausibly use.** A provider whose real API surface is called
  "default" cannot declare it. Accepted: the collision is loud (the reserved-name error) rather than
  silent, and the workaround is a different spelling.
- **Choosing service granularity commits us**, and C-37's stability contract binds the address once
  published. Is Zendesk's `support` a service, or is `tickets`? Getting it wrong is cheap now and
  expensive later — the same open question C-37 records for its scope segments, inherited here with a
  named owner.
- **Nothing yet reports a changed gid as a breaking change.** `flux-connectors diff` growing that
  check belongs with C-23's rename detection, as C-37 already records.

## Acceptance / done

See [C-49](../stories/C-49-provider-services.md). In short: `Service` is an IR level; services
partition the operation set; `default` is reserved, implicit and elided from every rendered address;
`api_version` resolves service-then-connector; the emitted unit is the service; a whole service is
selectable; the fields hash; and the six shipped `default`-only providers emit byte-identical output.
