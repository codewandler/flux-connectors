# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **The Fly.io connector**, the seventeenth provider — nine Machines operations under a `machines`
  service, authority `io.fly.api`, bearer token. *(Landed concurrently by another session; this entry
  is derived from `providers/fly.toml` and the generated artifacts rather than from its author, so
  treat the wording as descriptive rather than authoritative.)*

- **A core-catalogue projection.** `flux-connectors` now validates and republishes Flux's own
  vendored core operations from `specs/flux/core-v1.json` to `web/public/v1/`, alongside three JSON
  schemas — 81 files, 77 core entries. The module states its boundary explicitly: *"Flux owns these
  records. This crate checks and republishes the inert JSON; it does not register, execute,
  reinterpret, or mirror the built-in operations in `connector-catalog`."* *(Also landed
  concurrently; same caveat.)*

- **C-102** — **a filtered view is a shareable URL.** The explorer promised "every operation has a
  stable page you can share" — true of an operation, false of a *view*. Filter state now lives in the
  query string, so "every destructive Shopify operation" is a link someone can send. The list can also
  be sorted by catalogue order, id, or declared risk, with the risk order declared rather than
  alphabetical (`destructive` would otherwise sort between `default` and `high`).
  - Changing a filter **replaces** rather than pushes, so the back button leaves the explorer instead
    of walking back through every keystroke of a search. VitePress ships its own router whose only
    navigation method pushes, so this uses `history.replaceState`, passing `history.state` through
    untouched — that object holds the scroll position the router restores.
  - An unknown or stale parameter degrades to a wider view rather than erroring, so a shared link
    outlives a renamed connector.

- **C-100** — **the explorer renders outside the prose column.** It was designed against 6 providers
  and 25 operations and now indexes 16, 18 services and 88, inside a content column VitePress caps at
  688px — so sixteen cards rendered two-across and the filter bar wrapped to two rows. The cap is a
  rule keyed on `has-aside`, so the page that must be wide is the page that must not carry an aside:
  content column 688px → 1025px, provider grid 2 → 3 columns, filter bar 2 rows → 1. Prose pages are
  deliberately untouched — the doc column is right for paragraphs.
  - Four columns is **not** delivered and moved to C-103: a fourth track needs a 248px minimum and
    the card header measures 273px min-content because it does not wrap, so it needs a card
    restructure rather than a layout change.

- **C-101** — **the explorer surfaces services.** The catalogue publishes 18 services across 16
  connectors and the site showed none of them, so Google Workspace's three read as one. A provider
  card now lists the services it publishes with their operation counts and their `api_version` where
  it differs from the connector's; the operation list gains a service filter derived from the
  catalogue; and an operation row names its service where the connector has more than one.
  - The reserved `default` service is still rendered **nowhere** — it is elided from every published
    address by design, and a UI that named it would contradict that. Fifteen single-surface cards
    therefore grow no services list.
  - `slack` gains an `Address` row carrying its published `com.slack.api:v1`. That gid *is* the
    reserved service's, with `default` already elided by the address grammar, so the name is still
    unrendered — and the strict alternative would have satisfied the requirement with zero instances
    against today's catalogue, which is the vacuous pass the suite's own discipline exists to prevent.
  - The hand-maintained-data guard was widened to forbid service names and addresses in explorer
    sources, so the filter cannot silently start naming a vendor's service.

- **C-94** — **the flow graph**: a connector can compose its own members into a flow — an event wakes
  it, an operation reads, a gate guards, an operation writes — which lowers to **one Flux `op`**. Four
  waves had already built the vocabulary without naming it: an operation is a call node, an event is a
  source node, an `oip` is a global node id, and the dotted `wire` grammar is an edge.
  `crates/connector-spec/src/graph.rs`.
  - **It is not the second language the north star forbids, and the evidence is the repository's own
    history.** Every past rejection was an *expression* language — a template DSL, JSONPath, a
    vendor's remote expression evaluator. Every acceptance was declarative structure that compiles to
    Flux. So **no node carries a formula**: a gate's condition is a port reference, one of seven
    operators and a literal, and *this repository generates the Flux expression*.
    `NodeKind::free_text` destructures every variant exhaustively, so a field added later fails to
    compile until somebody classifies it — and there is deliberately no `Formula` role.
  - **A projection, not a layer.** `flux_lang::ast::Node` has 43 kinds and this repository constructs
    nine; every node names the existing variant it is — `Throttle`, `Confirm`, `Retry`, `When`, `Jq`,
    `Fmt`, `Obj`, `Lit`.
  - **Structural rules Flux's semantics dictate, not style.** A cycle is refused because Flux has no
    `goto`. Data convergence is free — a diamond is legal, since a statement may read any bound symbol
    — but a value leaves a control region only through a port the region declares.
  - **A `gate` exports nothing.** It lowers to `when`, which has no else here, so a symbol bound
    inside is *unbound* on the false path and reading it afterwards fails at runtime, long after the
    build passed. `retry`/`throttle`/`approval` always run their body or fail, so they may export —
    the contrast that shows the rule is about semantics rather than a blanket ban.
  - **Boundary nodes** — `trigger`, `schedule`, `endpoint` — declare what wakes a flow, take no
    inputs, sit in no region, and are emitted nowhere. flux lifts only `op` declarations; the operator
    writes the two-line program, as C-63 already establishes for the poll transport.
  - **Edges are symbols the compiler owns**, so an author never sees or names one — which is what
    makes action-proxy's silent `$emit` shadowing unrepresentable here. Node ids are author-stable,
    deliberately unlike flux's positional `NodeId`, which any edit invalidates.

- **C-90** — **credential addressing**: a tenant's credential for a connector now has a stable,
  derivable address, so a secret store can be wrapped in a convention instead of every deployment
  inventing one. `crates/connector-spec/src/credential.rs`.
  ```
  tenants/9f3a…/com.slack.api/signing_secret          # `default` service elided
  tenants/9f3a…/com.zendesk.api/support/api_token
  ```
  - **A convention, not a client.** The address is pure and derived from `pid` + service; anything
    that opens a socket is a host library outside the compile path (C-91). `Layout` is the seam —
    "wrap a simple Vault store with some conventions" is a decorator, and `TenantLayout` is the
    blessed default so two deployments cannot quietly diverge.
  - **The API version is deliberately absent.** A credential path is never the `gid`, because a token
    must survive the vendor's v2 migration — putting the version in the path would force every tenant
    to re-provision on a change that did not affect their credential.
  - **The leaf drops the vendor prefix**: `zendesk.api_token` is the flat-namespace name and the path
    already carries the authority. A prefix disagreeing with the connector id is refused, since it
    would render a plausible path under the wrong vendor.
  - `Connector::credential_ref_for` keeps its three outcomes distinguishable, because they have
    different owners: a bad tenant is the caller's error, a missing authority is `Ok(None)` (the same
    answer `gid_of` gives), and a path is neither.

- **C-86** — **the connector configuration surface**: a connector now declares what a *human* must
  supply before it can run, so a product can generate a working "Connect this integration" form from
  the connector alone. Everything else in this repository models how a credential reaches the wire;
  nothing modelled how it gets there in the first place.
  - **Configuration has two levels**, and neither this repository nor flux had the distinction:
    *operator* (set once per vendor — the OAuth app registration) and *connection* (set once per
    tenant — the subdomain, the token). Conflating them leaks the product's own credential to every
    customer, or serves exactly one of them. `Level` is **derived** from what a field binds, never
    authored, so an author cannot get it wrong.
  - A `[[config]]` field carries `label`, `help`, `example`, `format`, `required`, `secret` and
    `docs_url`, and `binds` says where the answer goes — `endpoint.<var>`, `credential.<name>`,
    `username.<name>`, `oauth.client_id`, `oauth.client_secret`.
  - **`format` is a closed enum rather than a regex**, so a renderer knows the rule, the message *and*
    the example. `example` is validated against it: a placeholder that would fail its own field is
    worse than none, because a user copies it.
  - A `verify` operation is declarable — the "Test connection" button. The convention already existed
    invisibly in three providers (`freshdesk-test`, `zendesk-test`, `babelforce-agent-list`) with
    nothing to make it findable.
  - **Webhooks became a full exposure**: `[channels.subscription]` links a binding to the operations
    that register it and names which parameter takes the product's callback URL; `[channels.setup]`
    carries the manual steps for vendors with no registration API. A `webhook` binding must declare
    one of the two. `[[events]]` gained `default` and `group`, so Slack's `message` firehose warning
    is finally machine-readable instead of prose only a model reads.

- **The auth archetype matrix** (`tests/auth_archetypes.rs`) — C-22, asked from the configuration
  side: *what form does each kind of authentication generate?* Prefixed header, basic join with a
  vendor marker and without one, raw-value header, no credential at all, AND/OR requirement sets, and
  the signing secret — each drawn from a real shipped provider.

- **C-82** — **channel bindings**: a connector can now describe a flux ingress surface instead of flux
  hand-writing one per vendor. A service gains two more member kinds, so the model is
  `provider → service → (operation | event | channel)`:
  - An **event** is the inbound direction, keeping the vendor's own name (`app_mention`,
    `issues.opened`).
  - A **channel binding is a composition, not a primitive**: it names declared events for inbound and
    a declared **operation** of the same connector for the reply. flux's Slack adapter ends by
    hand-building a `chat.postMessage` whose three fields are the three body params of
    `slack-chat-post-message` — an operation this repository already compiles. That is the 218 lines
    a binding is meant to retire.
  - Three transports — `webhook`, `socket`, `poll` — which is what makes inbound an abstraction over
    transports rather than a synonym for webhook. `providers/slack.toml` ships **both** Socket Mode
    and the Events API over one event set, one payload map and one reply.
  - `Reply::result` names the parameter carrying the **journey's own output**, which no path into the
    triggering event can reach. `HmacSpec::timestamp` says *where* a signed `{timestamp}` is read
    from — a template can say the value is signed but not where it comes from, and a host left to
    guess would fall back to its own clock.
  - Payload maps reuse the existing `wire` dotted-path grammar (`event.thread_ts`), not JSONPath. One
    path language in the repository, not two.
  - The three kinds share **one name namespace per service** and one `#name` address fragment, which
    settles C-66's open question. A cross-kind collision is refused; a within-kind duplicate is
    reported by its own pass, so one problem yields one line.

- **`AuthScheme::Signing`** — a credential never placed in a request, only used to verify an inbound
  one. The one deliberate divergence from `flux_plugin_protocol::AuthScheme`, so that a webhook secret
  is an ordinary `[[auth]]` entry and the manifest keeps naming every credential a connector requires.

### Changed

- **The four templated providers declare their tenant field** and lost the `SCHEMA GAP:` comment they
  had carried since C-17 — `zendesk` `{subdomain}`, `jira` `{site}`, `shopify` `{shop}`, `freshdesk`
  `{domain}`. The loader now **refuses** a template variable no `[[config]]` field binds, so the gap
  cannot reopen: a connector that cannot learn its own host is one nobody can configure.
  This closes [C-68](docs/stories/C-68-endpoint-binding.md)'s central acceptance, though in a
  different shape than it assumed — a hosted product has no environment variables per tenant, so what
  a connector declares is the *question to ask*, not the env var to read.

- Zendesk is the fullest form the fleet has: subdomain, agent email and API token, spanning three
  binding forms. Its help text tells a user not to type the `/token` marker Zendesk appends itself.

- No generated artifact changed except `catalog.json`'s `params` field (see **Fixed**): the configuration
  surface is in the IR and in the hash domain and reaches no artifact until
  [C-87](docs/stories/C-87-configuration-codegen.md).

- `providers/slack.toml` declares `authority = "com.slack.api"` and `api_version = "v1"`, which it had
  none of, so its binding's reply can render as an oip. The emitted `slack.flux` is **byte-identical**
  — the proof that a binding declares and never reaches the module. Only the slack manifest and
  `catalog.json` moved.

- `connectors.lock` entries are unaffected for the 15 providers that declare no inbound members: the
  new `Connector` fields carry `skip_serializing_if` inside the hash domain, so nothing churns for a
  provider nobody edited.

- A test that asserted "no shipped provider declares an authority yet, so `gid` is always null" now
  derives the expectation from the connector instead of hardcoding it.

### Fixed

- **A horizontal page overflow the narrow column had been hiding.** The provider card renders one
  `<code>` per host with **no whitespace between them**, so adjacent hostnames form a single
  unbreakable inline box. A 609px single-column card absorbed it; the two-column layout did not, and
  it escaped the page — measured 0 → 26px at 1280 and 0 → 4px at 1366, back to 0 with the fix. The
  hosts cell now wraps, which also gives the values the visual separation the markup never had.
  A separate, **pre-existing** overflow at phone widths (178px, identical before and after, caused by
  the operation list's grid) is untouched and belongs to C-103.

- **`first_template_variable` reported one variable of however many.** A base URL like
  `https://{region}.{tenant}.example` published an `unbound-base-url-template` issue naming `{region}`
  and left `{tenant}` invisible to every consumer of `catalog.json`. Replaced with
  `config::template_variables`, and the issue now names all of them in `params`, which was empty.

### Security

- **A tenant id is treated as untrusted input.** No construction can render a traversing path —
  empty, `.`, `..` anywhere, a leading or trailing `.`, any `/`, whitespace, control characters and
  anything over 128 characters are refused, and `validate_tenant` is public so a host can check before
  it builds. The precedent is close to home: action-proxy puts `x-babelforce-customer-id` and
  `x-babelforce-integration-id` — both client headers — straight into a Vault path with no validation.
  **Validation is not provenance**, and the design says so: deriving the tenant from an authenticated
  principal remains the host's job.

- **An explicitly-spelled `default` service no longer parses.** Found while testing: it would have
  been a second spelling of one address, and two paths for one credential is how a store ends up
  holding it twice with nothing to say which is current. `Gid::parse` refuses it for the same reason.

- **`secret` must agree with `binds`**, enforced at the loader in both directions. flux partitions
  secret from non-secret **by type** (`AuthMethod` versus `ConfigSpec`) and enforces it host-side, so
  a field claiming otherwise would put a contradicting source of truth in front of that enforcement.
  A credential field declared non-secret would be logged and echoed back; a subdomain declared secret
  would be hidden from an operator who needs to read it.

- **A `verify` operation cannot be a write.** A connection test runs unattended whenever someone opens
  a settings page, so a `high` or `destructive` operation is refused.

- **A `webhook` binding cannot stay silent about verification.** It must declare an HMAC scheme or
  state `verification = "none"` deliberately; an unset one is refused. Silence on an open endpoint is
  how an unverified event gets presented to a flow as trusted.

- **Replay is bounded by construction.** A `signed` template interpolating `{timestamp}` requires both
  a `tolerance` and a selector naming where the timestamp is read from.

- **The two directions cannot share a credential.** A verification secret must be `scheme =
  "signing"`, and no operation may authenticate with one — enforced in both directions at the loader.

## [0.3.0] — 2026-07-30

Sixteen providers, and a service level beneath them. **No provider can make a live API call yet** —
see the README's *Known limits*.

### Added
- **C-49** — a provider's **services** are the middle addressing level: `provider → service →
  operations`. A `Service` owns its own base URL and API version, an operation belongs to exactly one
  service, and an unset service means the reserved `default`, which is **elided** from published
  addresses. Building can select one whole service (`--service <NAME|GID>`). All ten shipped providers
  are single-service, so every artifact except `catalog.json` is byte-identical — the service fields
  carry `skip_serializing_if` inside the hash domain so no `connectors.lock` entry churns for a
  provider nobody edited.
  - **Security:** a service name reaches the emitted file path, so the loader now enforces the address
    grammar on every service name, authority and API version. A provider declaring
    `name = "../../../../outside/pwned"` previously wrote files **outside the repository root**; it is
    now refused, with a golden pinning the error.
- **C-70** — the **Jira** connector on API **v2**: issue get and create, comment list and add,
  transitions list and transition. v2 rather than v3 because v3's comment body is Atlassian Document
  Format — an array of block nodes — and `wire` paths address nested records only, so ADF is
  inexpressible rather than merely awkward. Pinned by a test, so a bulk upgrade to v3 fails loudly.
- **C-69** — the **Google Workspace** connector: `gmail`, `calendar` and `drive` as three services
  under one provider, each with its own API version and its own host, emitting one installable
  module-and-manifest pair per service. The first genuinely multi-service connector, and the proof
  C-49's service level works on a real vendor.
  - **Fixed, and only a multi-service provider could have exposed it:** the emitter bound the
    *connector's* base URL rather than the operation's *service* base URL, so a Gmail operation would
    have requested `www.googleapis.com` while the manifest installed beside it — the value C-10's
    `http_hosts` allowlist derives from — named `gmail.googleapis.com`. The two halves of one
    installable unit disagreeing about where traffic goes. Latent because for a single-service
    provider the two expressions resolve to the same string, which is exactly what C-49's
    byte-identity requirement asserted.
  - `catalog.json` now publishes a provider's `hosts` as the union of its services' hosts; before, a
    multi-service provider omitted a host some of its operations reach.
- **C-76** — the **OpenRouter** connector: chat completion, models list, model endpoints, credits.
  A transfer of the OpenAI connector's shape, with `max_completion_tokens` required so no
  LLM-callable spend is unbounded — the vendor deprecates `max_tokens`, and a test asserts it is
  absent so the deprecated spelling cannot come back.
- **C-78** — the **Zoom** connector: meeting get, create and delete, plus user get, with the nested
  `settings` object declared through a wire path. Meeting **UUID** addressing is excluded because
  `meeting_id` is typed as an integer, which makes a base64 id carrying `/` untypeable at the op
  boundary — the first case in the fleet where path injection is closed structurally rather than by
  the vendor's charset happening to be safe.
- **C-75** — the **Airtable** connector: record get, create, update and delete, with the `fields`
  envelope declared through a wire path as one opaque cell-value object — Airtable's field keys are a
  customer's own column names, unknown at compile time. It also settles the unencoded-path-parameter
  question by argument rather than by charset luck: the caller-facing parameter is `table_id`, and
  `^tbl[A-Za-z0-9]+$` makes the table-*name* form unrepresentable.
- **C-77** — the **Sentry** connector: issue get and update, project get, latest event. Every
  operation's emitted URL is pinned character-for-character *including its trailing slash*, because
  Sentry redirects or 404s without one and that is exactly what a later tidy-up removes silently.
- **C-71** — the **Asana** connector: task get, create, update, a story (comment) and project get.
  Every request body is wrapped in `{"data": {…}}` and every response records its payload at `/data`,
  declared through `wire` paths — the first real-vendor exercise of nested bodies, and it needed no
  emitter change.
- **C-72** — the **HubSpot** connector: contact, company and deal reads plus contact create and
  update, with the `properties` envelope declared through `wire` paths. HubSpot accepts a flat body
  with a 2xx and stores nothing, so this is a silent failure mode rather than a loud one.
- **C-73** — the **Intercom** connector: contact get and create, conversation get and reply, contact
  note. Admitted where Notion and Anthropic were excluded, because Intercom *defaults* its version
  header while theirs reject a request without one.
- **C-74** — the **Shopify** connector: order, product and customer reads plus a product update over
  the 2024-10 Admin REST API. The credential is a plain `X-Shopify-Access-Token` header carrying the
  whole value, which makes this the first shipped use of `AuthScheme::Header`.

### Changed
- **C-54** — the five hand-maintained `SHIPPED` provider lists and the two hardcoded catalogue totals
  are gone: every per-provider gate now derives its set from `providers/`, matching the definition
  `connector-cli`'s own discovery uses. **Adding a provider costs one file instead of seven places in
  five files across four crates.** The proof is a throwaway provider added with no test file edited —
  at the previous baseline every per-provider gate ignored it and passed; now eleven catch it. An
  empty `providers/` directory also fails loudly instead of passing vacuously, which is what made the
  old form dangerous rather than merely repetitive.

## [0.2.0] — 2026-07-30

Three connectors double the catalogue: **OpenAI**, **GitHub** and **Slack**. **No provider can make a
live API call yet** — see the README's *Known limits*.

### Added
- **C-51** — the **OpenAI** connector: the models pair, chat completions and embeddings, JSON in and
  JSON out with no query parameter of any type. `max_completion_tokens` is required rather than
  optional, so no LLM-invocable billed call is unbounded in cost.
- **C-52** — the **GitHub** connector: repository, issue and pull-request reads plus issue creation
  and commenting, addressed entirely by path parameters. Both writes are `risk = "high"`: a created
  issue or comment is world-visible and attributed to the token owner.
- **C-53** — the **Slack** connector: post a message, read conversation history, look up a user, add
  a reaction. Every operation is a POST with a JSON body *including the reads*, which is what keeps
  opaque channel and user ids out of a query string; Slack documents `application/json` for all four.
- All three connectors are curated to a **path-and-body surface only**. Every listing and search
  operation is deliberately excluded until C-30 lands, because the emitter still emits a string
  query value unencoded — the defect that makes `zendesk-ticket-search` non-functional. A test per
  connector asserts it declares no query parameter of any type.

### Known gaps found while shipping them
- **Nothing expresses "the failure is in the body of a 200."** Slack answers `{"ok": false}` with
  HTTP 200, and `ErrorEnvelope` has no success predicate, so the quirk survives only in each
  operation's prose description. Cursor pagination is unexpressible for a POST+JSON API for the same
  reason: `Pagination::Cursor` defines its cursor as a *query* parameter.
- A **constant, non-credential request header cannot be declared** at all: there is no `headers`
  table, and `connector-flux`'s `constant()` filter applies to the body chain only, so a
  `const`-pinned header emits as a caller-overridable argument. GitHub's
  `Accept: application/vnd.github+json` is therefore undeclared rather than smuggled in.
- An **optional body field cannot be omitted**: query parameters get a `when` guard, but every body
  field is placed unconditionally, so an omitted `required = false` field travels as an explicit
  `null`. OpenAI's inference knobs are left out rather than shipped as a probable-null.

### Changed
- **C-48** — rewrote the root README for human evaluators, front-loaded the agent workflow and
  generated-file boundaries in `AGENTS.md`, and recast the public site as a branded,
  consumer-facing connector catalogue. Public catalogue schema v2 no longer publishes internal
  design or story pointers; tests enforce that boundary and keep public logo assets in sync.

## [0.1.0] — 2026-07-30

The catalogue becomes public and browsable. **No provider can make a live API call yet** — see the
README's *Known limits*.

### Added
- **C-44** — the provider and operation explorer: every provider and operation browsable and
  filterable, one deep-linkable pre-rendered page per operation, and the whole thing working without
  JavaScript. A test fails if any explorer source hard-codes a provider id, vendor, host, credential
  or issue code.
- **C-45** — the README image is rendered by **flux's own** `flux_lang::highlight` — a CST walk that
  classifies a token by its parent node's kind — replacing a regex script that duplicated grammar
  flux owns. `build --png` additionally shells out to `flux render`.
- **Brand assets** — mark, icon, banner and favicon sizes under `assets/brand/`.
- **C-42** — `site/catalog.json`, a fourth backend over the same IR carrying every provider and
  operation with its typed parameters, credentials, hosts and generated Flux. Each operation also
  carries a **derived** `status`: four rules over the IR, no hard-coded operation list.
- **C-43** — a VitePress site under `web/` and a GitHub Actions workflow deploying it to GitHub
  Pages. The landing page carries the README's *Known limits* verbatim: a docs site that oversells is
  worse than none. CI builds the site on push and PR, so a broken site cannot publish.

## [0.0.1] — 2026-07-30

### Added
- **C-38** — `connector-catalog`: every operation's Flux embedded at compile time, queryable by id
  with its risk, idempotency, required credentials and hosts. A Rust consumer gets the whole
  catalogue with `cargo add`, no filesystem lookup.
- **C-38** — `build` also writes one `.flux` rendering per operation as the catalog's source; the
  per-provider module remains the installable artifact.

First tagged release. The pipeline works end to end and three providers compile; **no provider can
make a live API call yet** — see the README's *Known limits*.

### Added
- Repository scaffolding: the track backlog framework (vision, roadmap, stories board, design
  records) and the initial `connectors-v1` epic.
- **C-1** — the three-crate Cargo workspace (`connector-spec`, `connector-flux`, `connector-cli`
  producing the `flux-connectors` binary), dual MIT/Apache-2.0 licences, `.gitignore`, a README, and
  a CI workflow running build, test, `clippy -D warnings` and `fmt --check`.
- **C-1** — a flux-lang smoke test (`crates/connector-flux/tests/flux_lang_smoke.rs`) parsing a
  trivial `.flux` source through `flux_lang::program::Module::parse_str`, proving the dependency
  resolves and its API is usable from a consumer crate.

- **C-2** — the Connector IR in `connector-spec`: `Connector`, `Operation`, `ParamSet`, `Param`,
  `Quirks`, `Provenance`, plus the auth model (`AuthMethod`, `AuthRequirement`, `AuthScheme`,
  `OAuth2Spec`). Parameters and responses carry their JSON Schema; `risk` and `idempotency` are
  mandatory, so neither can be decided by silence.
- **C-2** — multi-credential auth: an operation references requirement *sets* — all credentials in a
  set (AND), any one set among alternatives (OR) — with unset and explicitly-empty distinguishable
  **on the wire**, so an unauthenticated operation cannot silently inherit credentials.
- **C-2** — deterministic IR serialization, proven from four angles including a tripwire that fails
  if anything in the workspace ever enables `serde_json/preserve_order`.
- **C-18** — `docs/designs/provider-operation-inventory.md`: curated operation sets and auth models
  for zendesk (7 of 7), freshdesk (9 of 16) and babelforce (9 of 163), every claim carrying a
  `path:line` citation.

- **C-16** — the `$auth` seam design, verified line-by-line against flux `v0.38.0`, plus eleven
  paste-ready story drafts for flux's board (`docs/designs/auth-seam-flux-stories.md`).
- **unified-auth epic** — credentials modelled on three orthogonal axes (source × acquisition ×
  placement) so a new provider archetype costs one value on one axis, not a new variant crossing all
  of them. Stories C-19 … C-22.

- **C-13** — the `flux-connectors` CLI: `build` and `diff` over provider discovery, atomic artifact
  writing, `--provider` filtering, and a byte-identical no-op rebuild. Artifacts land in
  `connectors/`.
- **C-13** — the offline guarantee is proven three ways: an armed deny-counter asserting zero
  network crossings, a source audit asserting no network primitive exists outside `src/net.rs`, and
  the binary building under a network-less user namespace. The audit test was falsified-checked by
  temporarily injecting a `TcpStream::connect` and confirming it failed.

- **C-3** — the provider-TOML front-end: a hand-authored file with no vendor spec produces a complete
  `Connector`, a spec-pointer file produces the patch set, with 13 golden error snapshots and a
  published JSON Schema kept in sync by test.
- **C-3** — `deny_unknown_fields` on the IR types themselves, closing the hole C-2's review found: a
  typo'd `authh` no longer deserializes to `auth: None` and silently inherits the connector's default
  credentials. Proven at four nesting depths.
- **C-8** — the Flux op emitter: an IR GET with path and query parameters lowers to a formatted
  composite `op` built from real `flux_lang` AST nodes, with a test asserting the output is a fixed
  point of flux's own formatter, and four golden files.

- **C-27** — the CLI seams are wired to the real loader and emitter, so `flux-connectors build`
  produces genuine `.flux` and `.connector.toml` artifacts instead of placeholders. A `[spec]`-backed
  provider is refused rather than emitted as an empty module.
- **C-17** — `providers/{zendesk,freshdesk,babelforce}.toml`, hand-authored and curated to 7 / 9 / 9
  operations (from 163 available for babelforce), each loading through `connector_spec::provider::load`
  and pinned by `shipped_providers.rs`.

- **C-7** — `connectors.lock` with an explicitly defined hash domain. The domain covers the compiled
  meaning of a connector and excludes **all** provenance, so a re-fetch or a comment-only TOML edit
  cannot move the IR hash. `HashDomain::of` destructures `Connector` exhaustively, making a
  later-added field a compile error until someone places it.
- **C-28** — `docs/designs/query-encoding.md` and two paste-ready flux story drafts. Establishes that
  generated query values are injectable, recommends a structured `query` map on `http.request` over a
  pure `urlencode` op, and specifies the interim refusal.

- **C-9** — request bodies, headers and response handling: POST/PUT/PATCH assemble a JSON body from
  the IR, content-type and parameterized headers emit, and the response is bound and returned
  explicitly so a non-2xx is data rather than an op failure. Write operations may not claim a read's
  risk, and a JSON Schema `const` body field is sent without appearing in the op signature.

- **C-29** — `Param::wire` (a dot-separated JSON path for body fields, a plain alias for query and
  header parameters) and `ParamSet::body_schema` for free-form object bodies. Both additive; C-2's
  determinism and round-trip tests pass with no assertion changed.
- **C-29** — the emitter builds a **nested** request body from wire paths, and three new refusals
  (`BadWirePath`, `BodyPathConflict`, `AmbiguousBody`) reject shapes that would otherwise silently
  drop a declared field.
- **C-29** — **`flux-connectors build` now produces all six artifacts**: `connectors/{zendesk,
  freshdesk,babelforce}.flux` and their manifests. All 25 shipped operations parse, are fixed points
  of flux's formatter, and reload as composite ops.

### Changed
- **C-1** — flux-lang is depended on from **crates.io** (`codewandler-flux-lang = "0.37"`) rather
  than as a git or path dependency. The flux git remote uses a developer-only SSH host alias that
  cannot resolve in CI, and a `../flux` path dependency is absent from a fresh clone.
