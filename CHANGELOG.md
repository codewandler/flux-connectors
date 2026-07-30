# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **C-51** — the **OpenAI** connector: the models pair, chat completions and embeddings, JSON in and
  JSON out with no query parameter of any type. `max_completion_tokens` is required rather than
  optional, so no LLM-invocable billed call is unbounded in cost.
- **C-52** — the **GitHub** connector: repository, issue and pull-request reads plus issue creation
  and commenting, addressed entirely by path parameters. Both writes are `risk = "high"`: a created
  issue or comment is world-visible and attributed to the token owner.
- Both connectors are curated to a **path-and-body surface only**. Every listing and search
  operation is deliberately excluded until C-30 lands, because the emitter still emits a string
  query value unencoded — the defect that makes `zendesk-ticket-search` non-functional. A test per
  connector asserts it declares no query parameter of any type.

### Known gaps found while shipping them
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
