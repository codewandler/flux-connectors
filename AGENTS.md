# AGENTS.md — operating contract for flux-connectors

This file is for coding agents and automation. Read it before changing the repository. It defines
how work is selected, which subsystem owns each concern, what must remain fail-closed, and what
evidence “done” requires. For a human introduction, use [README.md](README.md).

<!-- BEGIN track:agents -->
## Start here — mandatory workflow for every task

This repository uses the **track** framework. Every unit of work is a Markdown story in
`docs/stories/`; story frontmatter is the source of truth and the board in
[`docs/stories/README.md`](docs/stories/README.md) is generated from it.

1. **Orient.** Read the latest user request and run `git status --short --branch`. Uncommitted work
   is user-owned unless you created it during the current task; preserve unrelated changes.
2. **Select the work.** If the user named work, do that. Otherwise take the top `ready` story by
   priority (lower is higher). `/track:next` reports it; `/track:next <area>` filters by `areas`.
3. **Create a contract when needed.** New or unscoped work gets a story before implementation via
   `/track:story`. Read its `## Goal`, `## Acceptance`, and any linked `design:`. Acceptance defines
   done.
4. **Make the work visible.** Set the story to `in-progress` and run `/track:board`. For non-trivial
   design, write or update a record in `docs/designs/` before implementation.
5. **Implement and prove it.** Behavioral changes require a failing-first test. Keep generated
   output, docs, and code consistent; run the relevant checks while working.
6. **Close the work.** `/track:done <ID>` sets the story to `done`, adds the changelog entry, and
   regenerates the board. Confirm the acceptance checklist and the final diff before reporting.

Never hand-edit the generated region between `BEGIN track:board` and `END track:board`. After any
change to a story's `status`, `priority`, `title`, `epic`, or `note`, run `/track:board`. Optional
`areas: [subsystem]` tags affect queries only; they do not create board sections.
<!-- END track:agents -->

## Current project boundary

**Snapshot: v0.6.0.** `cargo run -p connector-cli -- build` compiles **45 providers**, **52 services**
and **254 curated connector operations** — plus 8 events, 2 channel bindings and 77 Flux core entries
(29 operations, 43 node kinds, 5 capabilities) with 3 core JSON Schemas — into **488 artifacts**. The
compiler, the embedded Rust catalogue, the JSON catalogue, the Tool pack and the public explorer all
work. **The repository now also ships a host**, `connectors-api` (C-200), which makes live API calls
— it is fenced away from the compile path in both directions, and the compiler itself still reaches
no network. Read
[Intentional gaps](#intentional-gaps) before changing code that appears broken.

> Every count in this paragraph and in `README.md` is **hand-typed and unchecked**. It has drifted
> repeatedly; `docs/stories/C-81-declared-counts-are-checked.md` is the fix and is still `ready`. Until
> it lands, re-measure before quoting: `ls providers/*.toml | wc -l`,
> `cargo run -p connector-cli -- diff`, and a query over `web/public/catalog.json`.

flux-connectors compiles vendor API descriptions into Flux-Lang. A provider is described in
`providers/<name>.toml`; the build emits an installable Flux module, a capability manifest,
per-operation renderings, Rust catalogue tables, and public catalogue data. Flux—not TOML—is the
execution format.

A connector is **not** a set of operations. It declares what a vendor can do in **both directions**,
and what an **operator** must supply to use it: operations and services outbound, events and channels
inbound, `auth` across both, `config` for what a human types first, `graphs` for a flow composed from
the members above, a service's `roles`, and `verify` for the read that proves the arrangement works.
Sixteen fields of `Connector` (`crates/connector-spec/src/ir.rs`) carry it, sharing one name namespace
per service. The full surface table is
[docs/designs/connector-surfaces.md](docs/designs/connector-surfaces.md).

The charter boundary comes first for any proposed provider:

- **Generated connectors belong here:** HTTP-based SaaS services such as Zendesk, Freshdesk,
  Salesforce, Intercom, OpenAI, Anthropic, and OpenRouter.
- **Hand-written technology adapters belong in `../flux/plugins`:** Docker, Kubernetes, SQL,
  Prometheus, Loki, Vault, Asterisk, and other stateful or protocol-rich systems.

If the target is a technology rather than a service, stop and place the work in flux unless an
accepted design explicitly changes this charter.

## Ownership boundaries

| Crate | Owns | Must never |
|---|---|---|
| `connector-spec` | IR, provider-TOML loading, validation, provenance, lockfile | Touch the network |
| `connector-flux` | Lowering the IR to `flux_lang` AST and formatting Flux | Emit Flux with string templates |
| `connector-cli` | Binary, orchestration, filesystem IO, and all future network IO | Reach the network during `build`, `diff`, or `check` |
| `connector-catalog` | Static provider/operation metadata and embedded Flux | Execute operations, touch the network/filesystem, or gain runtime dependencies |
| `connector-pack` | Projecting catalogue operations onto flux `ToolSpec`s, assembling auth onto a request, giving that request this software's `User-Agent` (C-223 — the host constructs no request, and a client-level header would be invisible to the dry run; see [docs/designs/host-identity.md](docs/designs/host-identity.md)), and handing the registry declarations | Open a socket, hold an HTTP client, resolve a host, or construct a runtime — egress is a constructor argument (`Egress`), and `permission_subjects`/`intents` must never be defaulted away |
| `connector-secrets` | Resolving a `CredentialRef` **address** to a **value**: the `SecretStore` port, `MemoryStore`, the `0600` `FileStore` (C-207, unix-only — a file mode is its whole security argument), and the optional Vault KV v2 client | Be reachable from `connector-cli` — it opens sockets, and that edge would end the offline guarantee; also: no expiry, refresh, rotation or revocation |
| `connectors-api` | **The host** (C-200): binding the pack's ports, holding the transport and the per-tenant credential store, serving the catalogue, and running operations | Construct a request of its own — every route ends in `connector_pack::pack`; ship a transport of its own; be depended on by anything (it is a **leaf**, and `dependency_fence.rs` holds both directions); be published (`publish = false`) |

The first four are the **compiler**. `connector-pack` and `connector-secrets` are **host libraries**,
built and tested here and excluded from the compile path. `connectors-api` is the **host** itself and
is the one crate here that opens a socket. `crates/connector-cli/tests/dependency_fence.rs` asserts
that fence over the resolved `Cargo.lock`, optional dependencies included, so adding the edge behind
a feature flag trips it too — and it now sorts every workspace member into one of those three
buckets, so a new crate that is none of them fails rather than passing unexamined. Among the
compiler and host libraries, `connector-pack` is the one that links flux's runtime types, and it
constructs none of it — see [Relationship to flux](#relationship-to-flux).

`connector-pack`'s own "must never hold an HTTP client" is asserted separately, in
`crates/connector-cli/tests/pack_links_no_http_client.rs` (C-199), and **it deliberately reads a
different graph**. The lock cannot state that claim: `connector-pack` legitimately depends on
`connector-secrets`, whose Vault client is an *optional* `reqwest`, so the lock reports
`codewandler-connector-pack -> codewandler-connector-secrets -> reqwest` for a build that never
happens. That fence therefore reads cargo's **feature-resolved** graph, and covers what it thereby
stops seeing with manifest-level assertions: the carrier is optional, `default = []`, and no
workspace member asks for `vault` — which matters because cargo unifies features, so one member
switching it on would put `reqwest` in the single `connector_secrets` rlib the pack links.

`connector-spec` ingest accepts bytes so it remains fully unit-testable. Vendor network access, when
implemented, belongs only in `connector-cli`'s `fetch` path.

## Source and generated-file boundaries

Generated artifacts are committed and reviewed, but they are not edited by hand. Change their
source or emitter, then run:

```bash
cargo run -p connector-cli -- build
cargo run -p connector-cli -- diff
```

`diff` must finish with `488 artifacts up to date (45 providers checked)` for the current catalogue.
The artifact count may legitimately change when providers or operations change; do not encode it as
a permanent invariant. It is also not currently checked against this file — see C-81 and the caveat
under [Current project boundary](#current-project-boundary).

| Generated path | Source of truth |
|---|---|
| `connectors/*.flux`, `connectors/*.connector.toml` | `providers/`, vendored `specs/`, compiler code |
| `crates/catalog/ops/<provider>/*.flux` | Emitted provider operations |
| `crates/catalog/src/generated/<provider>.rs` | Connector IR and catalogue emitter |
| `crates/catalog/src/generated.rs` | The provider set in `providers/` — **whole-catalogue** |
| `web/public/catalog.json` | Connector IR, `specs/flux/core-v1.json`, and public-catalogue emitter — **whole-catalogue** |
| `web/public/v1/**/*.json` | `specs/flux/core-v1.json` and core-catalogue publisher — **whole-catalogue** |
| `assets/readme-snippet-{light,dark}.svg` | `assets/readme-snippet.flux` and flux highlighter — **whole-catalogue** |

One nearby file is intentionally hand-maintained: `assets/readme-snippet.flux` is the checked source
for the README image, and tests keep it identical to the operation shown in the generated Zendesk
module.

### Vendored specs: the pulled bytes, never the pull configuration

A vendor document under `specs/` is committed because builds are hermetic and offline, and this
repository is **public**. So the rule for vendoring from a private source is: the pulled bytes come
here, the configuration that pulled them does not. Concretely, for babelforce (C-415), the five
OpenAPI documents are vendored and `sources.json` and `scripts/pull.sh` are not — they name an
internal GitLab host and its project ids, which is precisely the material that stays internal. For
the same reason `SpecSource::source_url` is **omitted** rather than pointed at that host; identity is
carried by the pull date in the file name and the `sha256` of the vendored bytes, recorded in
`specs/<vendor>.provenance.toml`, and drift stays detectable through `upstream_sha256` (C-25) even
though nothing here can re-fetch. Where the upstream document embeds credential-shaped example
values, what is vendored is a **declared scrub** of what was pulled: the scrub is a script
(`scripts/vendor-babelforce-specs.sh`) so a re-vendor is reproducible and reviewable as a diff, it
takes example *values* and never declarations — ingest must keep seeing a `securityScheme` for
drift-check to keep reporting on it — and it is enforced by
`crates/connector-spec/tests/vendored_specs.rs`, which fails on a hit rather than trusting that
somebody looked.

### Whole-catalogue artifacts are coordinator-owned

The four artifacts marked **whole-catalogue** above describe the catalogue *as a whole*. A scoped run
compiled a subset, so it cannot write one honestly — it would drop every provider it never looked at,
and it would do so successfully. `build` therefore emits them **only on a full run**, and
`--provider`/`--service` leave the committed files untouched rather than truncating them
([docs/designs/catalog-json.md](docs/designs/catalog-json.md) records the rule; C-104 brought the
provider index under it). `crates/connector-cli/tests/catalog_index.rs` asserts it.

They have the same status the board and `CHANGELOG.md` already have: **a story implementor does not
regenerate them, and the coordinator writes them at integration.** They are generated, so a conflict
in one is resolved by **regenerating, never by merging hunks** — a merged index is a plausible file
that describes no build.

This is what lets provider stories run in parallel. A provider story writes `providers/<id>.toml`
plus only per-provider artifacts, so two implementors' write sets are disjoint. **The gate a provider
implementor runs is scoped and does not include a full build:**

```bash
cargo run -p connector-cli -- build --provider <id>   # per-provider artifacts only
cargo run -p connector-cli -- diff  --provider <id>   # must report no drift
cargo build --workspace
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

`--no-fail-fast` is not optional here. Plain `cargo test --workspace` stops at the first failing
binary, and the expected failures below are spread across **five** of them, so a run without it
reports a number that is simply wrong — see [Validation](#validation).

**A story that adds a new provider leaves exactly eight tests red across five binaries, and that is
the design working.** They are whole-catalogue staleness checks, and every one is red precisely
because the implementor correctly did *not* write a whole-catalogue file. Measured, not predicted:
add `providers/<id>.toml` + `specs/<id>/v1.json`, run `build --provider <id>`, then
`cargo test --workspace --no-fail-fast`.

| red test | binary | what it is reporting |
|---|---|---|
| `the_provider_list_matches_the_repository` | `catalog::embedded_operations` | the committed index does not yet name the new provider |
| `the_catalog_is_not_empty` | `catalog::embedded_operations` | the provider and rendering counts disagree with `providers/` and `ops/` |
| `the_committed_tree_is_a_fixed_point_of_a_build` | `connector-cli::catalog_artifacts` | a full build would write the index and `catalog.json` |
| `a_build_plans_both_readme_images_and_they_are_current` | `connector-cli::readme_snippet` | same whole-tree fixed-point assertion, reached from the README images |
| `the_shipped_artifacts_are_byte_identical` | `connector-cli::service_units` | same again; it excludes only `catalog.json`, so the stale index surfaces here |
| `the_published_catalogue_carries_the_service` | `connector-cli::service_units` | committed `catalog.json` does not carry the new provider's service |
| `every_shipped_operation_carries_its_metadata_and_its_flux` | `connector-cli::site_catalog` | committed `catalog.json` is missing the new provider's operations |
| `the_build_writes_and_checks_site_catalog_json` | `connector-cli::site_catalog` | same, from the document-level check |

Four of the eight are the *same* whole-tree fixed-point assertion written in four places; the rest
are `catalog.json` and index staleness. Report them and stop; do **not** run a full build to silence
them. The coordinator's full build at integration resolves all eight, and it is the only build that
can, because it is the only one with every provider.

### The scoped gate does answer "can my connector make a call at all" (C-233)

It did not, and that cost a connector. C-110 shipped eight Linear operations, ran this gate green,
and was found in review to have **zero callable operations**: `connector-pack` read each pinned
GraphQL document's braces as configuration placeholders, so unconfigured every call refused and
configured the substitution rewrote the document. The implementor could not have caught it — every
`connector-pack` entry point wanted a `&'static catalog::Operation`, `catalog::Operation` is
`#[non_exhaustive]` so no synthetic one can be built outside the `catalog` crate, and the index that
carries a real one is a whole-catalogue artifact that does not name a new provider until integration.

**`cargo test --workspace --no-fail-fast` now answers it, with nothing extra to run and nothing to
write.** `crates/connector-pack/tests/request.rs::every_declared_operation_composes_a_request_from_its_declared_configuration`
enumerates `connectors/*.connector.toml` and reads each operation's Flux from
`crates/catalog/ops/<provider>/`. Both are **per-provider** artifacts that
`build --provider <id>` writes, so a connector that is not in the index yet is covered anyway. For
each operation it composes the request against the configuration the provider file **declares** —
the `[[config]]` fields' `binds` targets and `example` values, and an *empty* configuration for a
connector that declares none — and asserts the URL is absolute, brace-free and reaches the declared
host, and that the **body and headers do not move when the configuration does**.

A failure names the operation and quotes what it could not build. Two shapes to expect:

- *"its body binds the string literal `…`, whose `{…}` is neither a templated URL nor a
  configuration pin"* — the C-110 shape. A brace in a bound string literal is read as configuration
  (C-193), and only two kinds of literal qualify: a templated URL, and C-187's pin binds. Anything
  else is refused rather than filled in. Publishing the configuration surface so the pack reads
  variables instead of inferring them is [C-87](docs/stories/C-87-configuration-codegen.md).
- *"`<op>` needs `[…]`, which `providers/<id>.toml` declares no `[[config]]` field for"* — the
  connector asks a tenant for something no form will ever collect.

`connector_pack::Rehearsal` is the same capability asked one operation at a time, for a boundary
test of your own: `Rehearsal::of(id, provider, service, flux)` takes the emitted Flux and needs no
catalogue entry. `crates/connector-pack/tests/rehearsal.rs` is worked examples, including C-110's
withdrawn documents as a known positive.

**This is not one of the eight, and it must be green.** The eight are red because a whole-catalogue
artifact is deliberately stale; this one reads only per-provider artifacts, so a red here is a real
finding about the connector in front of you.

### A per-provider test asserts about its provider, never about the catalogue

The disjoint-write-set guarantee above is what lets provider stories run in parallel, and **a
catalogue-walking assertion breaks it without touching a shared file.** A test that enumerates
`providers/` and compares the result against a hand-written literal is invisible from its author's
worktree, which holds one connector; invisible to the other implementor, whose diff is entirely
disjoint; not among the eight staleness failures above, so it does not read as expected; and
unresolvable the way those eight are, because they are *regenerated* and this is a literal in a
shipped test. It surfaces for the first time at integration, attributed to whichever merge happened
to be second.

Two instances were measured on 2026-07-31. C-216's Discord test asserted a catalogue prefix census
equalled a four-element literal; Klaviyo, landing in the same wave, declares a fifth — each branch
green alone, red together. Review caught that one. The second had been on `main` since C-165:
Trello's test asserted the query-placed credential set equalled a two-element literal, and was green
only because no provider since Trello had placed a credential in the query string. The next one that
did would have turned *Trello's* test red.

**So: a file under `crates/*/tests/` named for a provider may not walk `providers/`.**
`crates/connector-cli/tests/per_provider_test_scope.rs` enforces it, deriving the per-provider file
set from `providers/` so it keeps no inventory of its own. Note it is deliberately wider than the
defect — it refuses the walk, not just the literal — because the author of a per-provider test cannot
review the population they are quantifying over, and a monotone claim written there is correct by
luck rather than by construction.

This is **not** "never measure the catalogue". A catalogue-wide claim has three homes, and choosing
one is the whole of the rule:

| the claim | where it goes | why it survives a new provider |
|---|---|---|
| a property true of **every** connector | a whole-catalogue test file — `crates/connector-flux/tests/query_placed_credentials.rs`, `shipped_modules.rs`, `input_schema_agreement.rs` | universally quantified, so a provider that satisfies it leaves the test green, and one that violates it is exactly when the test should fail |
| a premise about **specific** connectors | the per-provider file, loading them **by name** — `discord_connector.rs::the_non_bearer_prefixes_this_connector_joins_were_already_shipped` | a closed set; only those connectors changing can falsify it, which is when the evidence stops being true |
| a **measurement** of the catalogue | coordinator-owned and ratcheted — `crates/connector-spec/tests/response_schema_coverage.rs`, with `COVERED_FLOOR` in the fence above | a floor with slack, raised at integration by the only run that sees every provider |

The reshaping to copy is C-216's, and the reasoning generalises past the example. Ask what premise
the assertion is really testing. If it is about named connectors, name them. If it is about the
catalogue, write it as a property over whatever ships — Trello's census became "every connector that
places a credential in the query string puts nothing else there", which is the question C-159 §2's
hazard actually poses, and which a fifty-fourth connector cannot falsify merely by existing.

### A ninth and tenth staleness check exist, and both are coordinator-owned

`the_recorded_floor_is_the_measured_figure` (`crates/connector-spec/tests/response_schema_coverage.rs`)
is a **two-way** ratchet: coverage may run ahead of `COVERED_FLOOR` by up to a tenth of the catalogue,
and beyond that the floor must be raised in the same commit that earned it.

**It is red per *wave*, not per *story*, which is why it is not in the table above.** Measured during
the 2026-07-31 fan-out: C-166 (7 operations, all with response shapes) and C-171 (6, likewise) each
saw exactly eight red in their own worktree, because each fits inside the slack alone. Their
*accumulation* crossed it — coverage 105 of 123 against a floor of 92, slack 12.

So `COVERED_FLOOR` joins the fence: **an implementor never raises it, the coordinator raises it at
integration.** Two concurrent provider stories that each raised it would collide on one line, which
is exactly the failure C-104 exists to prevent. A provider implementor seeing this test red should
report it as a ninth and stop, not edit the constant.

**A tenth check sits beside it, with the same ownership and the same rhythm** (C-196).
`the_recorded_ceiling_is_the_measured_absence` holds `ABSENCE_CEILING` — the count of operations
shipping *without* a response shape — to the measurement in both directions, within
`ABSENCE_SLACK` (2). It replaces `RATIO_FLOOR_PERCENT`, which guarded the same regression as a
percentage, had no second direction, and had drifted to where five unschematized operations could
land unnoticed while 27 of the 53 shipped connectors are five operations or fewer.

What this changes for a provider implementor is one line: **a story landing three or more operations
whose vendors document no response body is now red on arrival**, on
`response_schema_coverage_does_not_fall_below_its_floor`. Report it as a tenth alongside the ninth
and stop — like `COVERED_FLOOR`, the constant is fenced and the coordinator moves it at integration.
A story landing zero, one or two honest absences is unaffected and stays green, which is every
provider story the catalogue has seen except babelforce (0 of 9) and fly (4). Both of those are
vendor-wide gaps, and a connector arriving with *nothing* is the arrival this check exists to make
loud rather than silent.

**A story that only changes an existing provider leaves four red**, not one — the index is still
correct, but `catalog.json` and the README images are not:
`the_committed_tree_is_a_fixed_point_of_a_build`, `a_build_plans_both_readme_images_and_they_are_current`,
`the_build_writes_and_checks_site_catalog_json` and
`every_shipped_operation_carries_its_metadata_and_its_flux`.

**This said three until 2026-08-01, and it was under-counted.** The original figure was measured by
editing a `description` in `providers/zendesk.toml` — which also alters the emitted Flux, so the
fourth check was firing then too and went unrecorded. `every_shipped_operation_carries_its_metadata_and_its_flux`
compares the Flux text `catalog.json` carries against what the emitter produces, so it goes red for
**any** change that alters an emitted module. Two implementors measured four independently (C-186,
C-241) before anyone corrected the table, which is the cost of a hand-counted figure in a document
implementors are told to trust.

The public VitePress site is a consumer surface, not a publication of repository internals. Public
pages may explain connectors, operation contracts, safety metadata, credentials, hosts, and current
availability. Do not publish or link internal designs, roadmap/story mechanics, crate architecture,
or agent instructions there. Those belong in the repository's `docs/` tree.

## Non-negotiable engineering rules

- **TOML is compiler input, never a runtime execution format.** Do not move behavior into config
  that a runtime reads directly.
- **Do not create a connector-specific DSL.** Interpolation, branching, retries, and error handling
  are Flux language concerns.
- **Emit through `flux_lang`, never string templates.** Build `flux_lang::ast` nodes and format with
  flux-lang's formatter.
- **Generated Flux must parse and analyze in CI.** This is the load-bearing compatibility gate.
- **Generation is explicit, committed, deterministic, and offline.** Never hide network access in a
  `build.rs` or in `build`, `diff`, or `check`.
- **No credential value enters provider TOML, generated Flux, a manifest, the public catalogue, or
  the lockfile.** Generated data carries credential references and environment-variable names only.
- **Refuse ambiguous or unsafe output.** A loud compile-time refusal is better than plausible but
  incorrect Flux.
- **Library errors use `thiserror`; the binary uses `anyhow`.** No `unwrap()` on fallible IO outside
  tests.

## Authentication contract

Authentication is modelled on three independent axes: **source × acquisition × placement**. Never
replace it with a flat enum of combined schemes; that becomes combinatorial and forces credential
assembly into generated code. See [docs/designs/unified-auth.md](docs/designs/unified-auth.md).

Generated Flux names a credential and nothing more. It must not add prefixes, base64-encode pairs,
refresh tokens, or perform session login. The host resolves the credential, performs effectful
acquisition such as OAuth2, applies the placement scheme, and registers values with its redactor.
Putting acquisition in Flux would expose raw tokens in model-visible symbols.

Flux's four existing `AuthScheme` variants are presets of the three-axis model. A connector using
only those presets must serialize exactly what flux already understands.

`Signing` is the **one deliberate divergence** from flux's vocabulary. Every other variant answers
"where does this secret go on the way out"; a webhook signing secret has no answer, because it never
goes out — it verifies bytes that arrived. It is declared in `[[auth]]` like any other credential so
that one namespace covers both directions and the manifest names everything a connector requires. The
two rules that keep the directions apart are enforced at the loader: a verification secret must be
`scheme = "signing"`, and no operation may authenticate with one.

## Configuration contract

A connector declares **what a human must supply** before it can run — see
[docs/designs/connector-configuration.md](docs/designs/connector-configuration.md). The boundary is:
**this repository declares; flux resolves; a UI renders.** Nothing here holds a value, a URL, or a
callback address.

- **Configuration has two levels, and `Level` is derived, never authored.** *Operator* level is set
  once per vendor by whoever runs the product (the OAuth app registration); *connection* level is set
  once per tenant by each end user (the subdomain, the token). Conflating them is a real defect both
  ways: asking an end user for a client secret hands them the product's own credential. The level is a
  consequence of what a field `binds`, so an author cannot state it wrongly.
- **A configuration value is addressed by `(tenant, provider, service, kind, name)`.** The service is
  load-bearing, not decoration: a *service* owns its `base_url`, so a `{variable}` in one belongs to
  that service, and a field's `name` is unique across the whole connector while the placeholder it
  fills is not. `contentful` declares `delivery_space_id` and `management_space_id`, both binding
  `endpoint.space_id`. Keyed without the service they were one slot, and a management write went to
  whichever space the delivery reads had been configured with — a `200` from a real server, not a
  refusal. `catalog::Operation::service` is what carries it to the runtime port (C-197); do not add a
  consumer that keys a tenant value by connector alone.
- **`secret` must agree with `binds`.** flux partitions secret from non-secret **by type** —
  `AuthMethod` versus `ConfigSpec` — and enforces it host-side. A field claiming otherwise would put a
  contradicting source of truth in front of that enforcement. The loader refuses it.
- **Do not duplicate flux's resolution.** `EndpointSpec::template` already composes a URL from
  `{placeholder}` values host-side. A `ConfigField` names the *destination*; it never re-implements the
  templating, and it never introduces a second secret model.
- **A connector asks for everything it needs and nothing it cannot use.** Every `{variable}` in a base
  URL is bound by exactly one field; every endpoint, credential and OAuth reference resolves.
- **A field must be renderable.** `label` and `help` are mandatory. Defaulting a label to the field
  name ships `zendesk.api_token` into a form as user-facing copy.
- **`format` is a closed enum, not a regex.** A renderer given a raw pattern can reject a value and
  cannot explain why. `example` is validated against `format`, because a placeholder that fails its own
  field is worse than none — a user copies it.
- **`description` is not UI copy.** Every `description` in this repository is the text a *model*
  receives as a tool contract. Presentation belongs in `label`/`help`, and overloading `description`
  is how one string comes to serve two audiences badly.
- **A `verify` operation is a read.** It is the "Test connection" button, and it runs unattended
  whenever someone opens a settings page; a `high` or `destructive` operation is refused.
- **A `webhook` binding says how it is registered** — `[channels.subscription]` or
  `[channels.setup]`. A product that knows a callback URL and nothing about what to do with it cannot
  finish an installation.

## Flow graph contract

A connector may compose its own members into a flow that lowers to **one Flux `op`** — see
[docs/designs/flow-graph.md](docs/designs/flow-graph.md).

- **No node ever carries a formula.** This is the line principle 2 actually draws: every rejection in
  this repository's history was an *expression* language (a template DSL, JSONPath, a vendor's remote
  expression evaluator); every acceptance was declarative structure. A gate's `Condition` is a port
  reference, one of seven operators and a literal — **this repository generates the Flux expression,
  the author never writes one.** `NodeKind::free_text` is the exhaustive tripwire, and there is no
  `Formula` role to classify a new field as. Needing one means stop and re-read the north star.
- **A graph is a projection of Flux, not a layer over it.** `flux_lang::ast::Node` has 43 kinds and
  this repository constructs nine; every node kind must name the existing variant it *is*. Inventing a
  node with no Flux counterpart is inventing semantics.
- **Boundary nodes declare and are emitted nowhere.** flux lifts only `op` declarations; `channel` and
  `trigger` are Program members an operator writes. So `trigger`, `schedule` and `endpoint` take no
  inputs, sit in no region, and reach no `.flux` — the same split channel bindings hold.
- **Control flow must nest; data flow need not.** Flux has no `goto`, so a cycle is refused outright.
  Data convergence is free — a statement may read any bound symbol — but a value leaves a region only
  through a port the region declares.
- **A `gate` exports nothing.** It lowers to `when`, which has no else here, so a symbol bound inside
  is *unbound* on the false path and reading it later fails at runtime. A value escaping a conditional
  needs a branch with a default. `retry`, `throttle` and `approval` always run their body or fail, so
  they may export.
- **Edges are symbols the compiler owns.** An author never sees or names one. This is what makes
  action-proxy's silent `$emit` shadowing unrepresentable, and it is worth keeping that way.
- **Node ids are author-stable**, deliberately unlike flux's positional `NodeId`. A saved graph must
  survive re-ordering.

## Credential addressing contract

A connector derives **where a tenant's credential is kept** — the address, never the value and never
the store. See [docs/designs/credential-addressing.md](docs/designs/credential-addressing.md).

```
tenants/<tenant>/<authority>[/@instances/<uuid>][/<service>]/<credential>
```

- **This repository owns the address; a host library owns the client.** The address is pure and
  derived from facts this repo already validates. Anything that opens a socket belongs in
  `connector-secrets`, which `connector-cli` must not depend on — that dependency edge is what keeps
  `crates/connector-cli/tests/no_network.rs` a true statement about the build.
- **The API version is deliberately absent.** A credential path is `pid` + service, never the `gid`,
  because a token must survive the vendor's v2 migration. Adding the version would force every tenant
  to re-provision on a change that did not affect their credential.
- **A tenant id is untrusted input.** `CredentialRef::new` returns a `Result` and no construction can
  render a traversing path. But validation is not provenance: deriving the tenant from an
  authenticated principal — never from request input — is the host's job. Do not write anything that
  implies otherwise.
- **`default` never reaches a path**, and spelling it out explicitly does not parse. Two spellings of
  one address is how a store holds the same credential twice with nothing to say which is current.
- **One tenant may hold two connections to one vendor, and the address says which** (C-406). The
  instance is a **uuid** — stable under rename, uncollidable, unspellable as a traversal — and it is
  carried **only when the tenant holds more than one connection of that kind**, so every
  single-connection address renders byte-identically to the four-component form and no stored
  credential moves. `TenantInstances` states the rule once: several connections and no uuid is a
  **refusal naming the uuids that would have worked**, never a default and never the first match —
  the alternative is a `200` from the wrong account. The human-facing "production vs sandbox" label
  is the **host's**, mapped to the uuid before an address is built; this repository never sees a
  label, and `connector-pack` composes the sole-connection form until a host threads a connection
  through.
- **The leaf drops the vendor prefix.** `zendesk.api_token` is the flat-namespace name; the path
  already carries the authority, so the leaf is `api_token`. A prefix disagreeing with the connector
  id is refused — it would render a plausible path under the wrong vendor.
- **Validate any new path segment at construction**, the same way a service name is validated at the
  loader. The cautionary case is real and close: action-proxy puts two client-supplied headers
  straight into a Vault path with no validation.

## Member contract

A service has **five member kinds**, and they share **one name namespace** —
`Connector::member_names_of` (`crates/connector-spec/src/ir.rs`) returns all five together, which is
the definition:

| kind | direction | emitted into the module? |
|---|---|---|
| `[[operations]]` | outbound — flux calls the vendor | yes, as an `op` |
| `[[events]]` | inbound — the vendor calls flux | **no** |
| `[[channels]]` | a binding composing the two | **no** |
| `[[config]]` | what a human supplies before any of it runs | **no** |
| `[[graphs]]` | a flow composing the members above | **yes**, as one `op` |

The last two joined later than the prose around them, and several doc comments still say "three
member kinds" — `crates/connector-spec/src/inbound.rs`, `src/address.rs`, `src/provider.rs`,
`crates/connector-cli/src/seam.rs`, `src/catalog.rs`, `src/site.rs`,
`schema/provider-toml.schema.json` and `docs/designs/channel-bindings.md`. They are describing the
namespace correctly and counting it wrongly; treat this table as the authority.

See [docs/designs/channel-bindings.md](docs/designs/channel-bindings.md) and, for the whole connector
surface rather than the member kinds alone,
[docs/designs/connector-surfaces.md](docs/designs/connector-surfaces.md).

- **A channel binding declares; it never installs.** It reaches the manifest and the catalogue and
  emits nothing into the module — flux lifts `op` declarations only, while `channel` and `trigger` are
  Program members an operator writes. The tempting wrong output is an event dressed up as a pollable
  op; refuse it.
- **A binding is a composition, not a primitive.** Its inbound half names declared events of its own
  service; its outbound half names a declared **operation** of the same connector. Do not grow a
  parallel reply mechanism — if a binding cannot answer with an operation the pipeline already emits,
  the binding is wrong, not the model.
- **One namespace per service.** They render into the same address (`…#name`) and into flux's
  declaration namespace, so a cross-kind collision is a loud error. A *within-kind* duplicate is
  reported by that kind's own pass, so one problem produces one line. A configuration field is not
  addressable in the same sense — nothing calls it — but it shares the namespace anyway, because it
  shares the *host's*: a config value and an operation resolving to one name would be ambiguous
  wherever a host looked either up.
- **A member name is wider than an operation id.** It admits `-`, `_` and `.`, because an event keeps
  its vendor spelling (`app_mention`, `issues.opened`). The narrower declarable-symbol rule stays with
  the emitter: this validator guards the *address*, `connector-flux` guards the *declaration*.
- **A binding holds completely or is refused.** A dangling reply, an unbound required parameter, a
  webhook that states no verification, a poll with no cursor — each would build, ship, pass every
  artifact check, and fail on an operator's first real delivery.
- **Silence is never a verification answer.** A `webhook` binding states an HMAC scheme or states
  `verification = "none"` deliberately. Never present an unverified event as trusted.
- **A `poll` binding requires a cursor.** flux's schedule channel drops ticks across a restart and
  replays none of them, so the cursor — not the interval — is what makes a poll correct. `interval` is
  advisory.

## Service contract

A provider is one vendor; a **service** is one of its API surfaces (`s3` and `bedrock-runtime` under
AWS). The service is the unit that is addressed, versioned, selected, emitted and installed. See
[docs/designs/provider-services.md](docs/designs/provider-services.md).

- **Services partition the operation set.** Every operation belongs to **exactly one** service; the
  per-service sets are pairwise disjoint and their union is every operation. This is what makes
  "install the whole `s3` service" a well-defined set, and it is asserted as a property
  (`crates/connector-spec/tests/service_partition.rs`). Do not replace it with a free-form `tags`
  field: a tag cannot partition an operation set, cannot carry a version, and cannot carry a host.
- **`default` is reserved, implicit, and elided.** An operation naming no service belongs to
  `default`; no `[[services]]` entry may declare it; and it is **never rendered** into an address
  (`com.freshdesk.api:v2`, not `com.freshdesk.api/default:v2`) or into a file name (`zendesk.flux`,
  not `zendesk-default.flux`). A provider that declares named services has no implicit `default` for
  an operation to fall into — omitting `service` there is a loud error.
- **A service owns its base URL and its API version**, with the connector's as defaults. Each emitted
  manifest carries its own service's `base_url` and its own operations, so a service's egress surface
  is never widened to the union of the provider's. C-10's `http_hosts` derives from that value.
- **No content field of a provider TOML influences an output path.** Paths derive from the discovered
  file stem, and the one content field that reaches a path — a service name, via
  `<provider>-<service>.flux` — is validated against the address grammar in the loader
  (`connector_spec::address::validate_service_name`). A name carrying `/` or `..` would let a provider
  file decide where a build writes, including outside the repository root. Validate any future field
  that reaches a path the same way, at the loader, before it reaches `Workspace`.
- **An address, once published, is not reused.** Renaming a service or an operation mints a new
  address and deprecates the old one; it never repoints an existing one. An `authority`, a service
  name and an `api_version` are checked against the grammar on load, so a rendered address always
  parses back to the value it was rendered from.

## Intentional gaps

These failures are recorded decisions. Do not “fix” one without reading its story and design.

- **~~Nothing here makes a live call, because nothing here is a *host*.~~ CLOSED 2026-07-31 — this
  gap is no longer real.** `crates/connectors-api` is the host, and both halves that were missing
  are in the graph: `codewandler-flux-web` supplies `HttpRequestTool` as the `Egress`, and the
  service binds the ports and runs the loop. The first real call to a vendor is recorded, with its
  response, in `crates/connectors-api/README.md` §"The live leg, performed and labelled". The host
  is loopback-bound and single-tenant-valued today; the charter permitting a deployed multi-tenant
  one, and the gate on widening the bind, are
  [docs/designs/connectors-api.md](docs/designs/connectors-api.md).

  **What remains open is narrower and is the module path, not the host path.** No generated provider
  can make a live call *as Flux* — `connectors/*.flux` is still unauthenticated, because `$auth` was
  taken off the critical path rather than landed. The history below is kept because it is what makes
  that distinction legible:

  The older account ("the `$auth` seam must land in flux") is **stale**. C-114 shipped the Tool
  pack's declaration half, C-115 gave each Tool its egress with the network gate mirrored, and C-116
  bound a `CredentialStore` port and moved auth assembly — the `Bearer` prefix, the basic-auth base64,
  query placement — **into Rust**. flux's whole-value `{"$secret": "ENV"}` marker therefore never has
  to grow prefix or encode support, `$auth` is off the critical path, and
  [docs/designs/auth-seam.md](docs/designs/auth-seam.md) now records a road not taken rather than a
  blocker. See [docs/designs/connector-tool-pack.md](docs/designs/connector-tool-pack.md).

  Two things used to be missing here, and **both have since landed**:

  1. ~~An `http.request` implementation in the dependency graph.~~ **Closed.**
     `codewandler-flux-web` sits on the engine line this workspace pins — 0.45 since C-403 — and
     `connectors-api` constructs its `HttpRequestTool` once and hands it to every operation as the
     `Egress`. Note what did *not* change: `connector-pack`'s own tests still pass a stub, and still
     say so — the crate must never link a client.

     **The engine line is not repeated here.** It is recorded once, in
     `crates/connector-cli/tests/flux_engine_line.rs` (`ENGINE_LINE`/`SPEC_LINE`), which requires
     every `codewandler-flux-*` requirement in `[workspace.dependencies]` to state it and the lock to
     carry one engine line rather than two. Bumping flux is a value change in that constant; a
     version quoted in prose is the hand-typed figure this file already warns about elsewhere.
  2. ~~A host to bind it.~~ **Closed.** `crates/connectors-api` constructs the registry, binds the
     secret store and the transport, and runs the loop. It is not the `crates/connectors-app` the
     older text names: the loopback narrowing that crate was designed under was superseded by
     [C-201](docs/stories/C-201-charter-multi-tenant-host.md). See
     [docs/designs/connectors-api.md](docs/designs/connectors-api.md), and
     [docs/designs/connectors-app.md](docs/designs/connectors-app.md) for the parts still current.

- **Six declarable surfaces reach no artifact at all.** This is the largest real gap in the repository.
  The IR models each one and the loader validates it, and then neither `connectors/*.connector.toml`
  — whose emitted fields are exactly `generator`, `connector`, `service`, `gid`, `vendor`,
  `description`, `runtime`, `base_url`, `api_version`, `module`, `operations`, `events`, `channels`
  — nor `web/public/catalog.json` carries it:

  | surface | declared today | where it stops |
  |---|---|---|
  | `[[config]]` | 45 fields across 28 providers | IR and loader only |
  | `verify` | 28 providers | IR and loader only |
  | `[[services]] roles` | 1 role (`anthropic` / `models`) | IR and loader only |
  | `quirks.pagination` | 6 operations across 3 providers | IR and loader only |
  | `[[graphs]]` | none — the lowering exists (`crates/connector-flux/src/graph.rs`) and nothing declares one | no consumer *and* no producer |
  | `quirks.rate_limit` | none — `providers/hubspot.toml` records a deliberate non-declaration | no consumer *and* no producer |

  The first four are the sharp ones, because the declarations already exist: a host reading a manifest
  cannot render a settings page, cannot find the "Test connection" operation, cannot ask what a service
  claims to do, and cannot page a list — for connectors that state all four in their provider TOML.
  Do not close this by widening the manifest ad hoc; the surface-to-artifact mapping is decided in
  [docs/designs/connector-surfaces.md](docs/designs/connector-surfaces.md).
- **Freshdesk declares no credential.** Its API key occupies the Basic username position, which the
  current model treats as non-secret config. Emitting it would bypass secret gating and redaction;
  the deliberate result is a fail-closed 401.
- **`zendesk-ticket-search` is non-functional.** Query values are not percent-encoded. Spaces are a
  misleading test because URL parsing can rescue them; `&`, `#`, and `+` corrupt the request, and
  `x&per_page=1` injects a parameter. **A `form` request body has the same gap** (C-144): flux exposes
  no form encoder and no percent-encoder a Flux *program* can call, so the emitter assembles the pairs
  with `fmt` and each value is interpolated verbatim. Half-encoding in emitted Flux would look correct
  and be wrong, and hand-rolling it out of `replace` chains is the connector-specific DSL this
  repository refuses — so the fix is a flux-side encoder. **The body half now exists in flux as L-101**
  (`parse($record, as: "form")`), and it reaches this repository only when flux-lang publishes it,
  because the pin is a crates.io version and must stay one. The *query* half is still open and is the
  structured-`query` handoff in
  [docs/designs/query-encoding-flux-stories.md](docs/designs/query-encoding-flux-stories.md).
- **Some operations are refused during emission.** Examples include a nested body path without a
  `wire` field, a dotted operation id, and an ambiguous free-form body — and, under
  `body_encoding = "form"`, a nested field, a free-form `body_schema`, or an encoding declared on an
  operation that sends no body. Each refusal names its owning story.
- **`check`, `fetch`, and `install` are unimplemented.** They exit explicitly and point to C-14 or
  C-15. Do not turn them into partial, best-effort behavior.

## Validation

The full repository gate is:

```bash
cargo fmt --all
cargo build --workspace
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

Read `cargo test --workspace` correctly: it stops at the first failing test binary. A count of green
summaries does not prove the remaining binaries ran. As a diagnostic, this must print nothing:

```bash
cargo test --workspace 2>&1 | grep -E "FAILED|error: test failed|panicked at"
```

The public site has a separate Node 22+ gate:

```bash
cd web
npm ci
npm run build
npm test
```

**This runs in CI, in the `web` job of [`.github/workflows/ci.yml`](.github/workflows/ci.yml)** — the
same workflow as the Rust gate, so it runs on every pull request and blocks a bad merge rather than
reporting after one. It is *not* in `pages.yml`: that workflow builds and publishes the site, and a
gate whose survival depends on the deploy path's trigger list is one narrowing away from being
decorative. The job comment records the decision in full (C-240).

**The order is not stylistic.** `npm test` reads the rendered site out of `web/.vitepress/dist`, so
running it before `npm run build` reports **19 of 32 failing** — every one a page that was never
rendered, none of them a defect. Build first, always. Until C-240 no workflow ran `npm test` at all,
while this section documented it as the gate; `web/test/ci_gate.test.mjs` now asserts that the gate
described here is the gate a workflow enforces, so the two cannot drift apart again silently.

**The host's operator page has a third gate, and it is the one to run for a change to
`crates/connectors-api/src/index.html`** (C-239):

```bash
cd crates/connectors-api/ui
npm ci
npm test
```

`node --test` + `happy-dom`, executing the served page against a stubbed `fetch`. It runs in CI as
the `host-page` job in [`.github/workflows/ci.yml`](.github/workflows/ci.yml). Deliberately a second
Node tree rather than a directory under `web/`: the public site is forbidden by C-147 to collect a
credential and this page exists to collect one, and the site's single-dependency property is not
something a harness for the host should spend.

It exists because that page was the one surface in this repository where "a behavioural change
requires a failing-first test" could not be honoured. C-234's security review ran 16 mutations and
M15 — drawing the developer sign-in unconditionally — stayed **green** because nothing could execute
the file. Four properties are pinned there, each previously held only by a comment: the `status.dev`
guard on the developer sign-in and its secondary styling; the three sign-in states; that no page
source assigns through `innerHTML`; and that `/auth/signout` and `/auth/dev` are reached by `fetch`
POST and never by a link, which is the `SameSite=Lax` property. A fifth is a **Rust** test —
`crates/connectors-api/tests/wiring_vocabulary.rs` — asserting that every `Wiring` variant's token is
one the page answers for, in both directions. It needs no Node and runs in `cargo test --workspace`
with everything else.

For a truly docs-only change, narrower checks are acceptable. State exactly what ran. Changes to
README Flux examples must run `cargo test -p connector-cli --test readme_snippet`. Changes under
`web/` must run the site build and tests. Changes to `crates/connectors-api/src/index.html` — or to
anything under `crates/connectors-api/ui/` — must run the host-page gate above. Changes to generated
public catalogue data or Rust emitters are not docs-only and require the relevant Rust tests plus
formatting and clippy.

## Publishing contract

**Publishing to crates.io is CI-only. Never run `cargo publish` by hand** — not locally, not with
`--allow-dirty`, not "just to test". A published version cannot be withdrawn or corrected: a burned
version number is burned, and a wrong `description`, `readme` or `keywords` is fixable only in the
*next* version. `--dry-run` is the only form of `cargo publish` anyone runs outside CI.

- A release is a consequence of pushing a `vX.Y.Z` tag.
  [`.github/workflows/crates-io.yml`](.github/workflows/crates-io.yml) does the rest. It needs one
  secret, `CARGO_REGISTRY_TOKEN`, checked before anything is packaged, and holds a `concurrency`
  group so two runs cannot race. `workflow_dispatch` resumes a run that died partway.
- **The publish closure is four crates, not three.** `connector-catalog`, `connector-spec`,
  `connector-secrets`, `connector-pack`. `connector-cli` and `connector-flux` are not published.
  The closure is *derived* from the manifests by
  [`scripts/publish-crates-io.sh`](scripts/publish-crates-io.sh), which lists only the consumable
  roots; the order is a topological sort, so a new edge changes it automatically.
  `crates/connector-cli/tests/publish_closure.rs` asserts the derivation, the order and the
  metadata.
- **Adding a workspace dependency can enlarge the closure.** If a published crate gains an edge to
  an unpublished workspace crate, that crate must be published too or consumers cannot resolve
  anything — the path dependency that makes it work here does not travel. The test above fails on it
  rather than letting a release discover it.
- **The publish is idempotent.** A `crate@version` already live is skipped, so a run that trips the
  crates.io new-crate rate limit can be re-run or the tag re-pushed to resume. Do not "fix" a
  partial publish by hand.
- **Metadata is checked, not assumed.** Every published crate carries `description`, `license`,
  `repository`, `readme` and `keywords`; the `package` job in
  [`.github/workflows/ci.yml`](.github/workflows/ci.yml) runs `cargo publish --dry-run` over the
  whole closure on every pull request, so a packaging error arrives as a review comment rather than
  as a release incident.
- **Crate names are not settled.** None of the four names is reserved on crates.io today, and
  `connector-cli` is already taken by an unrelated crate — evidence that bare `connector-*` names
  collide. Whether these publish as `connector-*` or `codewandler-connector-*` (matching the
  `codewandler-flux-*` family) is an open decision recorded in
  [docs/designs/crates-io-publishing.md](docs/designs/crates-io-publishing.md). **A name, once
  published, is permanent** — settle it before the first tag, not after.

See [docs/designs/crates-io-publishing.md](docs/designs/crates-io-publishing.md) for the reasoning
and [C-190](docs/stories/C-190-publish-catalog-pack-secrets.md) for *when* the first publish
happens. This contract is only about *how*.

## Relationship to flux

- flux-connectors depends on `codewandler-flux-lang` (library `flux_lang`) from crates.io, pinned in
  `[workspace.dependencies]`. Do not replace it with a git or `../flux` path dependency; those do not
  resolve in a fresh clone and couple the build to an unpublished tree.
- The **compiler** crates — `connector-spec`, `connector-flux`, `connector-cli` — depend on no part of the flux runtime, and `connector-catalog` stays dependency-free. `connector-pack` alone links `flux-runtime`/`flux-spec` among them, because a declaration handed to a host must be spelled in the host's own `ToolSpec`/`Tool` vocabulary. **The compiler still constructs no runtime: it compiles; flux executes.** What changed on 2026-07-31 is that the repository also ships a host — `connectors-api` (C-200) — which does construct a runtime, links `flux-web`'s `http.request`, and is fenced away from the compiler in both directions by `crates/connector-cli/tests/dependency_fence.rs`. The offline guarantee is a property of the compile path, not of the workspace.
- **flux's `$auth` support for `http.request` is no longer the critical path**, and had been listed
  here as though it were. C-114/C-115/C-116 assemble auth in Rust inside `connector-pack`, so the
  whole-value `{"$secret"}` marker never has to grow a prefix or an encoder.
  [docs/designs/auth-seam.md](docs/designs/auth-seam.md) is kept as the composite-path design; C-26's
  paste-ready flux drafts should not be filed as written. What still genuinely waits on a flux release
  is the **form/query encoder** (upstream `L-101`) — see the `zendesk-ticket-search` gap above.
