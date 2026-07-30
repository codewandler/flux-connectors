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

**Snapshot: v0.3.0.** `cargo run -p connector-cli -- build` compiles 17 providers and 97 curated
connector operations plus 77 Flux core entries into 237 artifacts. The compiler, embedded Rust catalogue, JSON catalogue, and public
explorer work. **No generated provider can make a live API call yet.** Read
[Intentional gaps](#intentional-gaps) before changing code that appears broken.

flux-connectors compiles vendor API descriptions into Flux-Lang. A provider is described in
`providers/<name>.toml`; the build emits an installable Flux module, a capability manifest,
per-operation renderings, Rust catalogue tables, and public catalogue data. Flux—not TOML—is the
execution format.

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

`connector-spec` ingest accepts bytes so it remains fully unit-testable. Vendor network access, when
implemented, belongs only in `connector-cli`'s `fetch` path.

## Source and generated-file boundaries

Generated artifacts are committed and reviewed, but they are not edited by hand. Change their
source or emitter, then run:

```bash
cargo run -p connector-cli -- build
cargo run -p connector-cli -- diff
```

`diff` must finish with `237 artifacts up to date (17 providers checked)` for the current catalogue.
The artifact count may legitimately change when providers or operations change; do not encode it as
a permanent invariant.

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

**A story that only changes an existing provider leaves three red**, not one — the index is still
correct, but `catalog.json` and the README images are not. Measured by editing a `description` in
`providers/zendesk.toml` and running `build --provider zendesk`:
`the_committed_tree_is_a_fixed_point_of_a_build`, `a_build_plans_both_readme_images_and_they_are_current`
and `the_build_writes_and_checks_site_catalog_json`.

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
tenants/<tenant>/<authority>/<service>/<credential>
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
- **The leaf drops the vendor prefix.** `zendesk.api_token` is the flat-namespace name; the path
  already carries the authority, so the leaf is `api_token`. A prefix disagreeing with the connector
  id is refused — it would render a plausible path under the wrong vendor.
- **Validate any new path segment at construction**, the same way a service name is validated at the
  loader. The cautionary case is real and close: action-proxy puts two client-supplied headers
  straight into a Vault path with no validation.

## Member contract

A service has **three member kinds**, and they share **one name namespace**:

| kind | direction | emitted into the module? |
|---|---|---|
| `[[operations]]` | outbound — flux calls the vendor | yes, as an `op` |
| `[[events]]` | inbound — the vendor calls flux | **no** |
| `[[channels]]` | a binding composing the two | **no** |
| `[[config]]` | what a human supplies before any of it runs | **no** |
| `[[graphs]]` | a flow composing the members above | **yes**, as one `op` |

See [docs/designs/channel-bindings.md](docs/designs/channel-bindings.md).

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

- **No provider can make a live call.** `http.request` can replace a whole value with
  `{"$secret": "ENV"}`, but it cannot assemble a Bearer header or a Basic pair. The required
  `$auth` seam is designed in [docs/designs/auth-seam.md](docs/designs/auth-seam.md) and must land in
  flux.
- **Freshdesk declares no credential.** Its API key occupies the Basic username position, which the
  current model treats as non-secret config. Emitting it would bypass secret gating and redaction;
  the deliberate result is a fail-closed 401.
- **`zendesk-ticket-search` is non-functional.** Query values are not percent-encoded. Spaces are a
  misleading test because URL parsing can rescue them; `&`, `#`, and `+` corrupt the request, and
  `x&per_page=1` injects a parameter.
- **Some operations are refused during emission.** Examples include a nested body path without a
  `wire` field, a dotted operation id, and an ambiguous free-form body. Each refusal names its
  owning story.
- **`check`, `fetch`, and `install` are unimplemented.** They exit explicitly and point to C-14 or
  C-15. Do not turn them into partial, best-effort behavior.

## Validation

The full repository gate is:

```bash
cargo fmt --all
cargo build --workspace
cargo test --workspace
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

For a truly docs-only change, narrower checks are acceptable. State exactly what ran. Changes to
README Flux examples must run `cargo test -p connector-cli --test readme_snippet`. Changes under
`web/` must run the site build and tests. Changes to generated public catalogue data or Rust emitters
are not docs-only and require the relevant Rust tests plus formatting and clippy.

## Relationship to flux

- flux-connectors depends on `codewandler-flux-lang` (library `flux_lang`) from crates.io, pinned in
  `[workspace.dependencies]`. Do not replace it with a git or `../flux` path dependency; those do not
  resolve in a fresh clone and couple the build to an unpublished tree.
- flux-connectors does not depend on the flux runtime. It compiles; flux executes.
- The critical runtime change is flux's `$auth` support for `http.request`. Its design is recorded
  here; implementation stories belong to flux's own board.
