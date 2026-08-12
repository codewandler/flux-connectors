# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.25.0] — 2026-08-13

## [0.24.0] — 2026-08-12

### Added

- **Channel handshakes resolve engine-free too** (C-558, completing the engine-free producer set).
  The sibling of C-557 for the inbound side: `connector-resolve` gains `channel_plan` over the same
  `ConfigPort` + `SecretStore` ports, relocating the channel base-URL substitution, the declared-
  authority validation and the `connect.auth` credential placement out of the flux-linked
  `connector-pack` (whose `channel_plan` now delegates down). A parity check holds the engine-free
  channel plan byte-identical to the flux-fed one across all five catalogue channel bindings. With
  this, `connector-resolve` resolves both operations and channels with no `codewandler-flux-*`
  dependency — a host can build every connector request and channel handshake as data.

- **The plan producers are engine-free, so a host derives a request plan without flux** (C-557,
  the last piece un-gating Exchange's engine-free adoption). C-538 moved the plan *derivation* to
  the engine-free `connector-resolve`; this moves the two *producers* of its inputs there too —
  `resolve_endpoints` (endpoint resolution: declared defaults, operator approval, HTTPS-origin
  normalisation) over a new `ConfigPort` trait, and `assemble_credentials` (mechanism selection,
  the acquisition axis, the redaction forms) over `connector-secrets`' `SecretStore` port, returning
  the credential set and the redaction set as **data**, touching no redactor. `connector-pack`'s
  `Credentials::resolve` and endpoint resolution now delegate down, so there is one derivation, not
  two; the whole-catalogue differential gate gains a fourth arm proving the engine-free producers
  byte-identical to the flux-`ToolContext` path for all 835 operations, request, subjects and
  redaction set. `connector-resolve` still links no `codewandler-flux-*`, and the offline compiler
  fence holds across its new edge to `connector-secrets`. A consumer can now obtain a complete
  `RequestPlan` without depending on `connector-pack` at all.

- **Anthropic declares both of its OAuth2 login flows** (C-555, completing the cross-repo login
  goal's third vendor). The **Console flow** — single-host, a public PKCE client (no client
  secret) — authorizes the connector's model catalogue and Admin API (`org:admin`). The
  **subscription flow** (the one Claude Code uses) is two-host: authorize on `claude.ai`, token on
  `platform.claude.com`, expressed through C-556's `token_endpoint` reference, PKCE S256, with
  refresh. Both are public clients, exempt from the operator-secret requirement via C-556's
  discriminator; endpoints are web-verified (Anthropic publishes none, so each is recorded with
  its source and measurement in the connector), and no registration value enters the artifact. A
  host composes either authorize URL from the declaration alone, exactly as for GitHub and GitLab.
  The catalogue is now 70 services / 1173 artifacts.

- **An OAuth2 declaration may place its token endpoint on a second service, and mark a public
  client** (C-556, for the Anthropic flows). `OAuth2Spec` gains `token_endpoint` — the name of a
  second declared service whose base URL the token path resolves against, a reference never a URL,
  so `http_hosts` and declared-authority validation keep working — and `public_client`, marking a
  PKCE client that issues no client secret. The credential-archetype gate now requires the
  operator-level secret client-secret field only of a **confidential** authorization-code client;
  a public one is exempt, and the default stays confidential so nothing escapes the requirement by
  omission. Both fields are additive and skipped when absent, so every existing document and
  manifest is byte-identical; `catalog::OAuth2` and the document schema carry them.

- **`connector-pack` publishes the request plan and the dispatch seam** (C-553, un-gating
  Exchange's engine-free adoption and upstream's Tool-wrapper retirement). `Operation::build_request_plan`
  yields the complete `RequestPlan` — request, permission subjects, redaction set — through the
  same enforcement topology the Tool path applies (it is the body `build_authenticated_request`
  already had, its result published instead of swallowed), and `Egress::send` is public so a
  consumer dispatches a plan-derived request without unwrapping the transport. The whole-catalogue
  differential gate gains a third arm holding the published plan byte-identical to the Flux
  derivation for all 835 operations; every newly public type keeps its `SensitiveText`/redacted-`Debug`
  discipline. A `produces_credential` operation's diversion is a documented plan-path boundary —
  such an operation goes through the Tool projection. No artifact bytes move.

- **GitHub declares its OAuth2 acquisition** (C-554, for the cross-repo login goal). A new
  `github-login` auth-host service (`https://github.com`, distinct from the API host) plus a
  `github.oauth_token` credential composing the authorization-code grant — endpoints verified
  against docs.github.com with the sources recorded in `providers/github.toml`. Grants are
  `authorization_code` only (the classic-OAuth-app model, chosen because it takes scopes and
  issues no refresh token; the GitHub App model is the recorded reversible alternative), scopes
  are `["repo", "read:org"]` at the vendor's documented floor, and no registration value is
  carried — `client_id`/secret/redirect stay deployment configuration. A host composes GitHub's
  authorize URL from the artifact exactly as it does GitLab's. The catalogue grows to 68 services
  and 1169 artifacts.

## [0.23.0] — 2026-08-12

### Added

- **`connector-pack` derives every request from the canonical document, behind a whole-catalogue
  byte-identity gate** (C-538, the resolver delivery of C-534's program). The plan-deriving core
  is a new published, engine-free crate — `codewandler-connector-resolve` (lib
  `connector_resolve`), whose `resolve` returns the request plan as data with secret-bearing
  fields on the redacted-`Debug` pattern and no `codewandler-flux-*` edge (pinned by a
  dependency-direction test). `build_request` reads the document's request template; the AST walk
  is unreachable from the request path; `DocumentRehearsal` ships beside `Rehearsal` with the
  same surface for Exchange's settings/verify migration (C-539). The differential gate compares
  method, URL, headers, body, `permission_subjects`, the registered redaction set AND the
  configuration surface for all 835 operations — it went red at its base on a real divergence
  class (the document publishes IR parameter names, callers address Flux symbols; 23 operations)
  and every `connector_pack::Error` variant keeps its name and wording, enforced by test. The
  recorded residue: the `ToolSpec` projection still parses emitted Flux for the extended
  description and contract `input_schema` the document does not yet carry — that closure, which
  is also what C-540's deletion actually waits on, is C-552.

### Changed

- **The 55 providers compile concurrently, folded strictly in provider order** (C-544, third
  delivery of C-546's test-cost program). `compile_all` claims providers off an atomic cursor
  across scoped std threads — no new dependency — and a single sort by provider index decides
  everything downstream, so the plan, every diagnostic, the refusal a broken provider raises, and
  every artifact byte are identical at any width (width 1 is the old loop, and permanent tests
  compare the full Plan against it; a seeded completion-order fold demonstrably reports the wrong
  provider's error and stays in the suite as a tripwire). A full `diff` drops from ~31 s to
  ~13 s (~2.6×) — a floor every whole-tree fixed-point test also pays — with babelforce alone
  10.3 s of the sequential cost (filed as C-551; the nextest interaction as C-550).

- **The workspace gate runs tests as parallel processes through pinned cargo-nextest** (C-543,
  second delivery of C-546's test-cost program). `cargo test` runs one test binary at a time and
  its serialisation is total — 792.14 s wall clock against a 790.63 s sum of its own per-target
  times (0.2 % apart); the same suite under nextest 0.9.143 runs in 365 s plus 2 s of doc-tests,
  2.16× on an uncontended machine (~1.6× under load). The verified surface is proven not to
  shrink: 1892 = 1877 nextest + 3 ignored + 12 doc-tests, reconciled by `comm` in both
  directions, with doc-tests kept explicitly (`cargo test --workspace --doc`) and
  `fail-fast = false` pinned in `.config/nextest.toml`. `ci.yml`, `scripts/cut-release.sh` (whose
  preflight now refuses to cut before touching anything if the runner is absent, naming the
  install command), AGENTS.md § Validation and README convert together. The connector-secrets CI
  matrix jobs deliberately stay on plain `cargo test` so the root-privileged ownership proofs
  cannot become silent skips. The remaining wall-clock floor — one 284–386 s conformance test —
  is filed as C-549.

- **The test suite links one binary per crate, not one per file** (C-533, first delivery of
  C-546's test-cost program). The workspace's 201 integration-test files are now `#[path]` modules
  of nine `tests/main.rs` roots — no test deleted, merged or weakened, proven by name-level
  identity of the before/after `cargo test --workspace -- --list` output (1892 = 1892, identical
  per-package multisets). Executables in `target/debug/deps` fall 792 → 38 and integration link
  targets 201 → 9; a full clean workspace test run completes in 473 s. The shared
  `shipped_provider`/`origin_corpus` support modules are declared once per consuming binary
  instead of `#[path]`-included 88 times. A single test is now addressed as
  `cargo test -p <package> --test main <module>::<test>`; AGENTS.md's expected-staleness table and
  gate advice were reconciled in the same change.

### Added

- **The catalog pack is a verifiable GitHub release asset** (C-547, operator-proposed). Every
  `vX.Y.Z` release carries `catalog.pack` and `catalog.pack.sha256`, attached mechanically by the
  new tag-triggered `.github/workflows/release-assets.yml` — a client with no Rust toolchain and
  no clone fetches the catalogue from the release and verifies it twice: `sha256sum -c` against
  the out-of-band digest (which equals the tag's `connectors.lock` `[pack]` row), then
  `Pack::load`'s embedded digest and schema-version refusals. The workflow refuses to attach a
  pack the tag's lockfile does not vouch for, handles the release-created-after-publish ordering
  without ever creating a release itself, and holds `contents: write` alone so the crates.io
  publish gains no new failure mode. The client contract is documented in the reader's README;
  `web/test/release_assets.test.mjs` pins the workflow against silent narrowing (mutation-tested
  against seven seeded narrowings). v0.22.0's assets were attached at integration with the digest
  verified against its tag (`e6c1f242…`).

## [0.22.0] — 2026-08-12

### Added

- **Every provider now compiles to a canonical catalog document** (C-536, the first delivery of
  C-534's program). `catalog/<name>.catalog.json` — 55 documents plus their published JSON Schema
  (`catalog/connector-document.schema.json`), all committed, byte-deterministic, hashed per
  provider in `connectors.lock`, and validated in-process at render time. Each document carries the
  complete published surface including an explicit request template (method, URL template,
  parameter placement, body encoding, constant headers, endpoint slots) equivalent to what
  `connector-pack` derives by parsing the emitted Flux — proven field-for-field for zendesk's 35
  operations by `crates/connector-pack/tests/document_differential.rs`, the mechanism C-538's
  whole-catalogue gate extends. A service's `roles` and `quirks.pagination` reach an artifact for
  the first time; `quirks.rate_limit` becomes representable (nothing declares one yet). The
  template vocabulary is closed: a construct the pack's evaluator would refuse is a build error,
  never a degraded document, and the schema carries no field for an OAuth2 registration value at
  all — `client_id` is published as an operator-level configuration requirement, never a value.
  Emission is additive: every previously emitted artifact is unchanged (the build now reports 1166
  artifacts across 55 providers), and nothing ships that reads the documents until the pack and
  resolver land (C-537, C-538).
- **The canonical documents compile into one pack, served by a new published reader crate**
  (C-537, the second delivery of C-534's program). A full build now derives
  `crates/catalog-reader/catalog.pack` — one uncompressed, offset-indexed, digest-carrying file
  over the committed document bytes (9,547,465 B; index overhead 52,467 B ≈ 0.55%) —
  byte-deterministically: three independent builds across two checkouts reproduced
  `sha256 7670fe86…`, and `connectors.lock` records it as the sixth whole-catalogue artifact
  under `[pack]`. The new crate `codewandler-connector-catalog-reader` (lib `catalog_reader`)
  serves the embedded pack with **zero non-optional dependencies** — the digest check is a
  vendored SHA-256, cross-checked against `sha2` across every padding boundary in tests — through
  `providers()`, `provider()`, `operation()`, `operations_of()`, and a `Pack::load(path)` that
  refuses a wrong container format, digest or schema version by name before serving any record.
  `codewandler-connector-catalog` becomes an additive shim: it re-exports the reader as
  `catalog::reader` with no breaking change to its public API (`crates/catalog/tests/
  consumer_api.rs` compiles the whole promised surface to hold that line), and the `&'static`
  tables remain the legacy API's storage until C-540 — deferral reasoning in
  `docs/designs/catalog-artifact.md` §2.4. The zstd-CBOR working choice is rejected in writing
  there (§2.1–2.3): a zero-dependency reader can carry no codec, compression ties
  byte-determinism to a compressor version, and the raw payload keeps every record byte-identical
  to its reviewed committed document. The reader joins the derived publish closure (now five
  crates: address, catalog-reader, catalog, secrets, pack), and `scripts/cut-release.sh` carries
  the documents and the pack through a release. The build now plans 1167 artifacts.

### Fixed

- **The status route no longer points a machine-readable consumer at a superseded story** (C-545,
  from C-542's adjacent findings). `connector-cli`'s status surface served an `Issue` with
  `story: "C-10"` behind `CREDENTIALS_REACH_THE_REQUEST = false`; it now names C-534's program —
  the same successor the manifest header cites — and a sweeping unit test refuses any issue naming
  a superseded story. The four stale C-10 source comments in `seam.rs`/`catalog.rs` are reconciled
  in the same change; no artifact byte moves (`Issue::story` is never serialized). The ~45
  remaining C-10 comment references across five other crates are filed as C-548.
- **The emitted manifest header no longer promises a superseded story** (C-542, from C-535's
  review finding). Every generated `.connector.toml` opened with `# Auth and the \`http_hosts\`
  allowlist land in C-10.`; C-535 closed C-10 as superseded-never-implemented, so 67 committed
  manifests pointed readers at work that is never coming. The header now states the current
  arrangement — auth is assembled by the host resolver (`connector-pack`), and the manifest
  becomes a projection of the catalog artifact under C-534's program. A unit test beside the
  emitter (`crates/connector-cli/src/seam.rs`) pins both header lines and refuses any `C-10`
  mention; all 67 manifests and the lockfile were regenerated, with every other artifact
  byte-identical. The machine-readable sibling — `status.rs` serving an `Issue` with
  `story: "C-10"` — is filed as C-545.

### Changed

- **The compile destination is a catalog artifact, not Flux source** (C-535, adopting flux-roadmap
  Decision 0022; program in C-534). The repository contract now states the destination everywhere
  it stated the old north star: the build will lower the IR to one canonical committed document per
  provider and a compressed pack the resolver reads, instead of emitting Flux that
  `connector-pack` parses back. The vision's "compiled, never interpreted" north star is superseded
  in place with the dated owner amendment quoting the original; `AGENTS.md`, `README.md` and
  `docs/integrating-with-flux.md` describe the destination while stating plainly that none of it
  has shipped — the emitter still runs and every artifact still ships until C-536…C-540 land
  behind the byte-identical differential gate. C-10 (`$auth` injection) and C-15 (the module
  installer) are closed as superseded, never implemented: Flux never grows a connector module
  loader. The `.flux`-module half of C-41/`connector-bundle.md` is annotated as superseded; the
  bundle-directory grouping idea survives. No behavior, artifact byte, or safety invariant changed.

## [0.21.0] — 2026-08-12

### Added

- **Internal infrastructure markers leave the public repository** (C-532). `docs/designs/spec-front-end.md`
  argued that the vendored specs are public while the fetch configuration is internal, cited a
  leak-marker regex naming strings "that must never be published" — and then quoted those strings, in
  a public repository, in the paragraph making the argument. Eleven occurrences across eight files:
  the internal forge hostname, two internal repository paths, and the internal secret store's path
  cited eight times as an architectural precedent. All are now described rather than named; every
  argument that cited one is preserved, because each depends on what that system *did* rather than on
  what it is called. This does not unpublish anything — the hostname is inside the released v0.20.0
  tag — it stops the strings reaching any further release or crates.io copy.

- **A hosted deployment can be asked for its redirect URI** (C-531). `oauth.redirect_uri` joins
  `oauth.client_id` and `oauth.client_secret` as an operator-level, non-secret binding — the third
  half of one application registration, issued together and supplied together. `OAuth2Spec::redirect`
  models a loopback port and path (RFC 8252 §7.3, the native-app shape), so a host reached at
  `https://exchange.internal/api/oauth/callback` previously had nowhere to declare its callback and
  would have met the mismatch on the vendor's error page. `providers/gitlab.toml` declares it, and
  `auth_archetypes.rs` now requires it of every connector declaring a grant. The loopback field is
  kept and not deprecated: a loopback redirect is a vendor fact, a registered URI is a deployment
  fact, and both can be true for one connector.

- **GitLab authenticates as the integration or on behalf of a user** (C-530), and it is the first
  shipped connector to declare `[auth.oauth2]`. `gitlab.oauth_token` sits beside `gitlab.token` and
  `default_auth` lists them as **alternatives**: a deployment provisions one static token org-wide,
  or each signed-in person completes an OAuth2 grant and acts as themselves. Both declare
  `subject = "user"`. The OAuth application is operator level, derived from `binds`, so an end user
  is never asked for the product's own client secret. `read_repository` is requested and the reason
  recorded — it is what lets the resulting token clone over HTTPS, and while cloning is not a
  connector operation the credential a git client is handed is this one. `connectors/gitlab.flux` is
  byte-identical.

- **One deployment asks its origin question once** (C-529). `ConfigField::also_services` lets one
  field fill the base-URL placeholder of several services, keeping `service` as the head and the
  address the value is stored under. A self-managed GitLab serves its REST API at `{origin}/api/v4`
  and its OAuth endpoints at `{origin}` — one server, one fact — so gitlab.com and a self-hosted
  instance both work from one operator-approved value that moves both surfaces together. Declaring
  it twice would be a security defect rather than a redundancy: two slots that must agree and are
  not forced to is how a token exchange reaches a host the API never approved.

  Sharing is stated, never inferred — Contentful's two `space_id` fields stay two values, because
  keyed as one a management write went to whichever space the delivery reads had been configured
  with. Four loader refusals; `catalog::ConfigField` publishes the field; a per-service manifest now
  carries whatever fills its placeholder.

  The alternative — an `origin` on `OAuth2Spec` — was rejected: it puts a destination in a second
  spellable place, which is the defect C-523 exists to remove, in the one place where getting it
  wrong sends the client secret somewhere else. `OAuth2Spec.endpoint` stays a reference to a declared
  endpoint, which is what keeps the exchange inside the egress allow-list by construction.

- **A credential declares whose authority it carries** (C-528). `connector_spec::Subject` and
  `catalog::Subject` — `unstated` | `app` | `user` — land on `AuthMethod` and on the published
  `catalog::Credential`. This is the "on behalf of" axis, independent of placement and acquisition:
  Slack's single OAuth v2 grant returns a workspace bot token and the signed-in person's token in one
  response, placed identically and acquired identically, differing only in who they can act as.
  `providers/slack.toml` had that fact in prose and no field to state it in; it now declares both
  credentials `app`.

  **The default is `unstated`, and it means "nobody has reviewed this" rather than `app`.** A
  consumer needing the distinction refuses on it — assuming `app` over-grants, assuming `user`
  silently fails. Requiring every connector to state it, as C-516 did for direction, is not yet
  available: 55 connectors ship credentials unreviewed and some are genuinely ambiguous, GitHub's
  single `github.token` being documented as covering both an App installation token and a personal
  access token. The unreviewed default is skipped when serialized, so only Slack's manifest moved.

  **Breaking for consumers that construct `catalog::Credential`** — it gains a field and is
  deliberately not `#[non_exhaustive]`. Every generated catalogue table was rewritten; no `.flux`
  byte moved.

- **GitHub and GitLab can answer what a token reaches** (C-527). GitHub gains `github-user-get`,
  `github-org-list`, `github-org-repo-list` and `github-user-repo-list`; GitLab gains
  `gitlab-group-list` and `gitlab-project-list`. Every forge operation previously took `{owner}`/
  `{repo}` or a numeric `{project_id}` as given, so a caller holding a valid token could enumerate
  nothing. `gitlab-project-list` returns that numeric id and `http_url_to_repo` beside it — the HTTPS
  clone address, declared because cloning is not a connector operation. GitHub also declares
  `verify = "github-user-get"`; it had none, so a host reading its manifest had no Test connection
  read. The catalogue moves 829 → 835 operations and 1102 → 1108 artifacts; the five originally
  published GitHub operations keep their Flux bytes.

  Three reviewed gates asserted that nothing percent-encodes a query value and enforced "every query
  parameter is an integer". **C-30 invalidated that premise** — a scalar now travels in the
  structured `http.request(query: …)` map with RFC 3986 semantics. The rule was corrected to the two
  properties it was a proxy for, both strictly stronger: every query parameter is a scalar, and no
  query value reaches the URL, checked on every operation rather than four exempted ids. The
  published reads keep their narrow parameter sets as a compatibility bound on shipped request
  bytes, now labelled as such.

- **The published catalogue carries a credential's OAuth2 acquisition** (C-525). `catalog::Acquisition`
  gains an `OAuth2` variant holding a `&'static catalog::OAuth2`, with `catalog::OAuthGrant` and
  `catalog::OAuthRedirect` beside it, mirroring `connector_spec::OAuth2Spec` field for field in
  `&'static` form. The crate keeps zero runtime dependencies. Until now `OAuth2Spec` reached the
  emitted manifest and `web/public/catalog.json` but had no representation in `crates/catalog` — the
  one artifact Exchange and autodev link — so an `[auth.oauth2]` declaration would have been a
  marking no host could read. The loader now also refuses a credential declaring both an
  `[auth.oauth2]` grant and an operation's `produces_credential`, naming both and carrying the
  discriminator: an authorize or token endpoint is never a connector operation, so a credential
  obtained from the vendor's OAuth endpoints declares only the grant. No provider declares the block
  yet, so every generated artifact is byte-identical and `build`/`diff` still report 1102 artifacts
  up to date.

  **Breaking for consumers that match `Acquisition` exhaustively** — the same break `Minted` made,
  and the reason the enum is deliberately not `#[non_exhaustive]`. `connector-pack` treats the new
  variant as `Static`: the stored value is an access token the host already obtained, and the pack
  opens no socket, so a grant is not something it could run.

### Added

- **The connector domain is named once, in `docs/concepts.md`** (C-522). Connector, Service,
  Operation, Event Type, Channel Binding and Graph each get one definition and the artifact that
  publishes it, and the terms a *host* adds — Connection, Channel, Event Delivery, Trigger,
  Datasource, App, Managed Agent — are named as explicitly not connector members. The page records
  that `connector_catalog::Provider`/`ProviderKey` are compatibility API names, that no standalone
  `Service` value is published (service identity travels as a `service` field on `Operation`,
  `Event`, `Channel`, `ConfigField` and `ConfigChoices`), and that no provider declares a
  `[[graphs]]` member. Recovered from an unmerged 2026-08-03 branch and re-measured against v0.20.0
  before landing; the branch's `docs/vision.md` delta was dropped because C-495 has since reversed
  the *Non-goals* wording it restored.

### Changed

- **Three stale documentation claims are corrected against measured output.** `README.md` said six
  declarable surfaces reach no artifact with `config` at "112 fields across 40 providers" — C-87
  published `config` and `verify`, and the measured figures are 82 config fields across 42 providers,
  identical in `web/public/catalog.json` and across 46 emitted manifests, with `verify` on 42. It
  also said all 53 providers are hand-authored and that a `[spec]`-backed provider is rejected; there
  are 55 providers and 8 are `[spec]`-backed. `AGENTS.md` and `docs/stories/README.md` carried
  `v0.15.0`/`v0.17.0` snapshot labels against a v0.20.0 tree; the catalogue counts beside them
  re-measured exact and are unchanged.
- **Connector operation direction is now an explicit reviewed safety fact** (C-516). Every one of
  the 829 published operations states closed `read` or `write` direction independently of its HTTP
  method. Generated Flux, manifests, embedded/public catalogues, intents and staging carry that
  fact unchanged; spec-backed identities fail closed on missing, orphaned or conflicting review,
  and Flux's canonical consequence predicate remains the sole admission authority for parallel
  gather work.

## [0.20.0] — 2026-08-04

### Added

- **Recoverable prepared credential transactions are now a public host port** (C-515).
  `connector-secrets` adds opaque generation/id/digest types and the object-safe
  `PreparedSecretStore` state machine. `MemoryStore` and `FileStore` reserve one complete invisible
  `SecretBatch`, publish it atomically on commit, persist abort-before-prepare tombstones, refuse
  cross-id ledger mutation while prepared, and bound terminal outcomes at 4096 until the owner
  advances an inclusive generation fence. Vault explicitly returns the separate payload-free
  `Unsupported` error.

### Changed

- **Published crate setup now uses the permanent registry package names.** The four packaged
  READMEs use the `codewandler-connector-*` dependencies and canonical docs.rs and crates.io links;
  release cuts advance their major/minor examples with the workspace and preserve all customer
  Markdown headings in annotated tags.

- **`FileStore` now holds a lifetime native writer/recovery lease and speaks transactional v2**
  (C-515). Clean v1 files are not eagerly migrated; first transaction use couples credentials, the
  retirement fence and canonical terminal ledger in one atomic live file and uses a fixed owner-only
  complete-image stage. Recovery, child-process crash tests and canonical parsing refuse ambiguous,
  oversized, mismatched or already-retired records. Every 0.19.1 writer must be stopped before 0.20
  opens the store because an already-open legacy process can rewrite cached v1 and erase v2 recovery
  state; a fresh legacy opener refuses v2. Five native CI rows assert their host triples and run the
  crash, lease, concurrency and platform-protection suite instead of treating cross-compilation as
  runtime evidence.

- **The repository now speaks flux-roadmap Decision 0006's datasource vocabulary** (C-510). The
  catalogue datasource (C-137…C-140) is amended before any dispatch: the compiled-in catalogue
  binds as an indexed `flux_capabilities::DatasourceBackend` — the six retrieval verbs
  search/get/list/relation/batch_get/sources, with typed refusals from the trait's mutating
  methods — instead of the two-op
  `LiveDatasource` projection whose method set could not satisfy the stories' own acceptance.
  Vendor-data Datasource Definitions are chartered here as a new `[[datasources]]` connector
  surface (the `vendor-datasources` epic, C-511…C-514, designed in
  `docs/designs/vendor-datasource-declarations.md`): a projection over the connector's declared
  operations with IR-derived entity schemas, per-verb operation bindings, manifest/catalogue reach
  from first release and never the `.flux` module, superseding and removing `quirks.pagination`.
  C-501/C-502 now carry Decision 0006 rule 11's checkable rule that no datasource-declaring plugin
  is deleted without a mapped, conformance-proven replacement. Documentation only — no runtime
  capability is claimed.

## [0.19.1] — 2026-08-04

### Fixed

- **Durable connector credentials are owner-only on every supported platform** (C-509).
  `connector-secrets::FileStore` now exposes the same bounded v1 format and atomic `SecretBatch`
  backend on Linux, macOS and Windows. Unix verifies the effective owner plus `0700`/`0600` modes;
  Windows creates and verifies process-`TokenUser` ownership with a non-null protected DACL that
  allows only that SID. Both paths refuse unsafe object kinds, links/reparse points and
  uninspectable or widened security metadata before reading or writing a value, preserve the unsafe
  evidence without repair, and direct shared-directory users to an owner-only child rather than
  suggesting that they narrow `/tmp`.

## [0.19.0] — 2026-08-04

### Added

- **GitLab connections can target an operator-approved self-managed HTTPS origin** (C-508).
  GitLab.com remains the zero-configuration default; custom origins accept only an HTTPS authority,
  leave `/api/v4` connector-owned, remain inert until operator approval, and drive transport,
  permission subjects, intents and evidence from the same connection snapshot. Proposed,
  replaced, revoked and instance-scoped values fail closed without disclosing the proposal.
- **C-505** — Establish the native-plugin migration inventory and Exchange conformance ratchet: a
  retained inventory, closed captured-observation format, derived comparator and offline
  cross-repository release check now prevent a Flux adapter from disappearing before its published
  connector replacement is proven conformant through Exchange.

### Changed — breaking for public catalogue consumers

- **Connector configuration and verification are now public consumer contracts** (C-87).
  Generated manifests, the embedded catalogue and the explorer carry complete config, setup,
  subscription and `verify` metadata. Embedded `ConfigField` approval is a closed typed policy and
  `Provider::verify` identifies the bounded Test-connection read without parsing declaration JSON.
  Public `catalog.json` moves from schema 2 to schema 3: `auth.oauth2` is now the complete OAuth
  declaration instead of a lossy boolean.

### Changed

- **Official integrations now have one execution boundary in the cross-repository plan** (C-507).
  flux-connectors owns declarations, zero-IO runtime plans and vendor-specific artifacts; Exchange
  executes them and holds vendor authority; Flux embeds only the Exchange client. The C-495…C-505
  backlog no longer requires local Flux execution or local-versus-hosted parity, and C-505 now lands
  the inventory and legacy-plugin-versus-Exchange conformance ratchet before the first ordered wave.
  This corrects the v0.18.0 direction without claiming a new runtime has shipped.

## [0.18.0] — 2026-08-03

### Added

- **Hosts can safely admit several connections to one connector** (C-494). `CredentialScope` and
  `SecretStore::references` expose a validated tenant/authority inventory containing addresses only;
  `SecretBatch` applies checked moves, puts and deletes atomically in the memory and file stores.
  Unsupported backends, including the current Vault adapter, refuse explicitly rather than falling
  back to partial point mutations.
- **The connector pack can bind one C-406 connection UUID.** `Credentials::for_instance`,
  `Configuration::for_instance`, and `ConfigStore::get_for_instance` carry the same stable instance
  into secret and non-secret lookup. Existing constructors and config stores retain the
  sole-connection behaviour, while named instances fail closed unless the store implements them;
  mismatched port instances are refused before projection or channel composition.

### Changed

- **Connector operations can now publish policy-bearing semantic effects** (C-155). The closed Flux
  vocabulary is validated independently of host-resource effects, reaches install manifests, the
  embedded and public catalogues, `connector-pack`, and the explorer. Stripe capture and refund now
  declare `money`; capture is graded `destructive`, while cancellation remains a high-risk operation
  with no money or delete claim.

- **The documented integration boundary now covers every official integration** (C-495, C-496).
  Docker, Kubernetes, SQL, observability systems, secret stores, and other rich protocols are
  connector migration targets; Flux owns generic guarded runtimes and Exchange may host the same
  connector address. The existing native plugins remain until explicit parity and cutover gates pass.

### Fixed

- **Generated scalar query parameters are encoded structurally instead of interpolated into URLs**
  (C-30). String, numeric, boolean and configured query values now use Flux 0.54's RFC 3986 query
  map; null alone is omitted, so explicit `false` and `0` survive. Collection-shaped query values
  fail closed until their vendor wire convention is declared: 12 Asterisk ARI operations are
  deferred with reasons, while Babelforce's 18 scalar-or-array parameters explicitly select their
  documented string branch. The public catalogue no longer reports the retired
  `unencodable-query-value` issue for scalar queries.

## [0.17.0] — 2026-08-03

### Changed — breaking for `connector-pack` consumers

- **The connector runtime seam moves from Flux 0.52 to Flux 0.54** (C-493). Hosts must move their
  engine pins with this release: the pack's generated channel plans and the guarded WebSocket
  executor now share the same `Tool`, `System`, and result types. All six authored requirements and
  the resolved lock move as one registry-only unit; the independent `flux-spec` line remains 1.3.

### Changed — breaking for `connector-catalog` consumers

- **Generated channel bindings now expose their complete routing contract** (C-489–C-491).
  `catalog::Channel` gains `delivery_id`; downstream struct literals must add the optional selector.
  The catalogue also publishes socket connection paths, query/header templates, auth alternatives,
  subprotocols, event wire identities and raw-payload mode, so a host no longer reparses declaration
  JSON to run a generic RFC 6455 binding. `connector_pack::channel_plan` resolves those declarations
  through tenant-bound configuration and credential ports into a redacted, zero-I/O handshake plan.
  Configured bytes cannot move the connector-declared authority, and refusals name the field without
  echoing its runtime value.

### Added

- **Asterisk ARI now includes its generated event WebSocket** (C-492). `ari-events` composes the
  declared `/events` upgrade with Basic authentication, required `app`, default-false
  `subscribeAll`, and all 45 official `Event` subtypes. PascalCase wire discriminators map exactly
  once to stable lowercase-kebab local names, with full resolved schemas and the untouched event as
  the routed payload. The source census remains explicit: 108 REST operations plus one socket
  channel.

## [0.16.0] — 2026-08-02

### Changed — breaking for `connector-pack` consumers

- **The connector runtime seam moves from Flux 0.49 to Flux 0.52** (C-488). `connector-pack`
  exchanges Flux `Tool`, `ToolSpec`, `ToolContext`, and result types with its host, so downstream
  hosts must move their engine pins with this connector release; mixing these pre-1.0 minor lines
  resolves distinct, unlinkable traits.

  The six authored engine requirements and twelve resolved Flux packages move as one registry-only
  unit, while the independent `flux-spec` line remains at 1.3. The registry-source comparison found
  byte-identical `src/` trees for `flux-lang` and `flux-credentials`; additions in core, runtime,
  system, and web compile without a connector source change. The engine-line tests and workspace
  build pass, and `connector-cli diff` reports all 1114 artifacts up to date across 55 providers, so
  no connector operation bytes change.

## [0.15.0] — 2026-08-02

### Changed — breaking for Zendesk catalogue consumers

- **Every Zendesk operation now comes from its vendored first-party API description** (C-487).
  The seven remaining hand-authored Support calls and two Messaging transcriptions have been
  replaced by selected operations, and response-only recursive schemas are represented by a bounded
  prefix while request recursion still fails closed. The three hand-authored ticket-update variants
  collapse to the one `UpdateTicket` operation the vendor document actually declares, taking the
  Zendesk surface from 37 to 35 operations. This greenfield catalogue keeps no compatibility aliases.

### Added

- **Asterisk ARI is a spec-generated REST connector with all 108 ordinary HTTP operations**
  (C-483–C-486). Eleven official Asterisk 22.10.1 Swagger documents are vendored and normalized
  deterministically; Basic credentials and the configured TLS authority compose through the normal
  connector request path. Fifteen operations are exposed to model discovery while callers can
  resolve the complete catalogue by name. The one WebSocket upgrade is deliberately absent: event
  delivery is deferred until connector channels have a settled contract.

### Fixed

- **Connector Basic authentication now accepts configured TLS authorities with literal ports.**
  This permits deployments such as ARI's HTTPS listener without weakening the rule that credentials
  are never sent over plain HTTP.

## [0.14.0] — 2026-08-02

### Changed — breaking for `connector-pack` consumers

- **Caller-owned path segments are refused before authentication or egress when they could reshape
  the URL** (C-478). `connector_pack::Error` gains `UnsafePathParameter`; downstream exhaustive
  matches must add that arm or a wildcard. String path values containing `/`, `?`, `#`, `%`, `\\`,
  whitespace/control characters, or the complete segments `.` and `..` no longer compose a
  request. Safe strings and numeric identifiers retain their bytes, and query/header/body values do
  not inherit the path rule.

### Changed — breaking for `codewandler-connector-spec` consumers

- **Selected operations now retain their exact API-document provenance** (C-481). The public
  `Provenance` struct gains an `operation_specs` map, so downstream struct literals and exhaustive
  destructuring must add that field (usually `BTreeMap::new()`) or use
  `..Provenance::default()`. The new public `OperationSpecSource` value is otherwise additive. Its
  vendor `operation_id`, ingested upstream version, measured committed-document SHA-256, and
  nullable public source URL are derived during patch application and remain outside the IR hash
  domain; provider TOML operations cannot author them. Public catalogue operations gain an
  always-present `spec_source` key, `null` for inline operations and an object for selected ones,
  without exposing local paths or fetch metadata.

- **A published default service can grow named siblings without changing its addresses** (C-458).
  `Service` gains the public `legacy` field; downstream struct literals must add `legacy: false` or
  use `..Default::default()`. Connector authors may set `legacy = true` only on an already-published
  `default` beside a named service, and every operation, spec, event, channel, config field, and graph
  must then state its service explicitly. The legacy GID/OIP, credential path, and unsuffixed module
  stay unchanged while named siblings use ordinary segmented addresses and suffixed artifacts.

- **A configuration pin can reuse a Basic username in a request path** (C-475). Public `Pin` is no
  longer `Copy`, and `Pin::variable` changes from `&str` to `Cow<'_, str>` so a qualified
  `username.<credential>` placeholder can travel without leaking or duplicating the username value.
  Downstream callers must clone/borrow pins deliberately and accept the `Cow`. `LoadedProvider` also
  gains private bookkeeping for explicit legacy-service membership, so external code can no longer
  construct or destructure that public type by fields; use the loader and its accessors.

### Added

- **Five established connectors add twenty exact first-party-spec reads** (C-467–C-474): GitHub
  adds repository issues, pull-request files, workflow runs and commits; Stripe adds country specs,
  events, exchange rates and billing meters; Microsoft Graph adds messages, master categories,
  supported time zones and supported languages; OpenAI adds stored-response, response-input, file
  and batch reads; Twilio adds recording list/get, usage records and conferences. Each provider
  vendors a pinned, reproducibly scrubbed source and selects four operation ids explicitly. String
  cursors, free-text/OData filters, recursive schemas, and currently unencodable write bodies remain
  omitted or deferred rather than silently weakened.

- **Zendesk grows from 7 operations to 37 across three surfaces** (C-462–C-464, C-466). Support adds
  five incremental/custom-object reads and eight foundation reads; Help Center adds seven category,
  section, article and translation operations; Messaging adds nine conversation, participant,
  message and user operations using a separate app id and app-scoped Basic key. Seven Messaging
  operations select directly from the pinned document, while two message operations use bounded
  transcriptions because their official response graph is recursive. The original seven Support
  operations, their addresses, and their Flux bytes remain unchanged.

- **Twilio's configured Account SID now supplies both halves of its Basic credential and the four
  new read paths** (C-475). One non-secret field binds the username and qualified path pin; the
  account id is no longer collected twice for those reads, while the five established operations
  keep their published caller signatures.

- **Zendesk Support adds eight query-free foundation reads** (C-466): recent tickets, built-in view
  tickets, one user, one organization, groups, ticket fields, ticket forms, and custom statuses.
  Their 33 optional query inputs remain explicitly omitted, response envelopes retain the pinned
  document's requiredness, and unsafe view path segments are refused. Ticket and organization
  creation remain deferred for incomplete contracts; create-or-update user remains withheld because
  both union variants expose `password` and its merge variant requires no stable identity.

- **Zendesk Support now exposes query-free ticket audit history** (C-461). The new
  `zendesk-ticket-audit-list` operation is selected by operationId from Zendesk's pinned Ticketing
  document, and its seven optional query parameters are explicitly omitted until structural query
  encoding is safe. The seven existing Zendesk operations keep their methods, paths, addresses, and
  per-operation Flux bytes.

- **Zendesk's first-party Ticketing, Help Center, and Messaging OpenAPI documents are vendored,
  scrubbed, and provenanced** (C-459). A reproducible script records upstream and committed hashes,
  removes credential/contact/system-shaped example values, preserves security declarations, and is
  absent from every offline compiler path.

### Documented

- **GitHub and Stripe record an explicit runtime/schema-version limitation** (C-477 follow-up).
  GitHub sends its media type but not `X-GitHub-Api-Version`; Stripe sends no `Stripe-Version`, so
  its response behavior follows the account-pinned version while selected schemas come from
  `2026-07-29.dahlia`. This release does not claim runtime bytes are date-pinned. Twilio message/call
  writes remain deferred for structural form encoding, and the selected Stripe exchange-rate
  surface retains the vendor's deprecated/restricted warning.

- **Zendesk's Support webhook family is explicitly withheld rather than partially published**
  (C-465). The five prose-only CRUD requests and both signing-secret endpoints are accounted for,
  but the generic Webhook response can carry the live signing credential and raw responses are not
  redacted by narrowing their schema. Response-safe update/delete calls do not ship as an orphaned
  lifecycle, and inbound events wait for C-479's lossless wire discriminator plus C-480's complete
  subscription and generated-secret provisioning.

- **The Zendesk suite has an implementation inventory instead of an endpoint wish list** (C-460).
  Support foundations, sync/custom data, Help Center, Messaging, and webhooks each have bounded
  carry/defer/withhold decisions. Ticket creation, unrestricted custom-object paths, missing-body
  operations, multipart uploads, credential responses, and lossless inbound event names remain
  deferred behind their actual model gaps.

### Fixed

- **The public explorer no longer hides a legacy primary service when a connector gains named
  siblings** (C-482). A multi-surface `default` remains the machine value used by filters and
  addresses but renders generically as `Primary` in facets, provider cards, and operation rows.
  Ordinary single-surface defaults remain elided; Zendesk Support is now selectable beside Help
  Center and Messaging.

- **Documented catalogue counts are checked against the full build plan** (C-81). Provider,
  service, operation, and artifact totals plus the exact clean `diff` sentence in `README.md` and
  `AGENTS.md` now fail when prose drifts; the failure tells maintainers to regenerate the numbers
  rather than relaxing the assertion.

- **Caller path parameters can no longer escape their reviewed URL segment** (C-478). The pack
  derives placement from emitted Flux, including guarded branches, and validates caller strings
  before credentials, permission subjects, or transport are reached. This closes the prerequisite
  for Zendesk's unconstrained Messaging identifiers.

- **The board header describes the released repository instead of its initial scaffold** (C-454
  follow-up). It now carries the measured v0.14.0 catalogue snapshot and the complete Rust, Node and
  packaging gate; C-81 keeps its provider and artifact counts tied to the full build plan.

## [0.13.0] — 2026-08-02

### Changed — breaking for `connector-pack` consumers

- **The connector runtime seam moves from flux 0.47 to flux 0.49** (C-455). `connector-pack` exposes
  `flux_runtime::Tool` and `flux_core::Result` in its public boundary, so a host and the pack must be
  built on the same pre-1.0 minor line; mixing them is two unrelated traits, not a conservative
  downgrade. Downstream hosts must move their engine pins with this connector release.

  The impact here was measured against the registry sources and the gate. `flux-core`, `flux-web`
  and `flux-credentials` have byte-identical `src/` trees between 0.47.1 and 0.49.0; runtime adds an
  identity constructor, lang adds canonicalization/CLI support, and system adds guarded UDP/raw-ICMP
  dial variants. This workspace does not exhaustively match the newly widened `DialTarget`. All
  eleven resolved engine packages moved together, the workspace builds, and all 951 generated
  artifacts remain current. No connector operation changed.

### Fixed

- **The whole-catalogue network safety gate no longer rebuilds the catalogue once per operation**
  (C-456). Its four permission-subject and intent assertions keep the same coverage while their
  targeted test time falls from 191.31 seconds to 1.48 seconds.

- **Forgotten worktrees and their story state are reconciled before release** (C-454). Five stories
  whose implementations and Progress were complete now say `done`; blocker chains and epic
  checklists match the commits already on `main`. All 75 merged local branch pointers were removed.
  Recoverable Git objects were reviewed before pruning: completed work was already superseded,
  C-403's leftover would have reverted the engine and response contract, and the unsanitized C-25
  document was never merged. A concurrently-created Flux 0.49 worktree was detected and integrated
  as C-455 instead of being mistaken for stale debris.

- **A release cut now runs both Node consumer gates before it creates a tag** (C-453). v0.12.0
  demonstrated the hole: its crates.io workflow published successfully while CI's public-site job
  found three red assertions. `scripts/cut-release.sh` now runs the public-site build and 42 tests
  plus the host page's 15 tests inside the same transaction as the Rust gate; a red Node suite
  restores the tree and leaves no commit or tag. The release fixture executes and records those
  commands, and proves the red path rather than inspecting shell text.

  The three failures are closed too. `web/data/catalog.mts` now carries the emitted
  `Provider.config_choices` / `ConfigChoices` / `Choice` shape, and the operation page says
  “username” where the bare English word “user” collided with a newly shipped service name. The
  guard that refuses hand-maintained catalogue data remains strict. `WHATS-NEW.md`'s displaced
  `[Unreleased]` section and malformed 0.11.0 / 0.12.0 headings were repaired, and the cut now
  refuses that ordering before it changes a file.

## [0.12.0] — 2026-08-02

### Changed — breaking for `connector-flux` consumers

- **`connector_flux::Error` gained three variants** — `SparseBodyArray`, `BadArrayIndex` and
  `NumericWirePathSegment` (C-185) — and the enum is not `#[non_exhaustive]`, so a downstream
  exhaustive `match` on it will no longer compile. Add the new arms or a `_` arm. This is the same
  shape as 0.11.0's `Service` field: the crate publishes, so adding to a public type is breaking.

### Added

- **A request body can carry an array, at a declared length** (C-185). A `wire` path segment may now
  carry a bracketed index, so `personalizations[0].to[0].email` puts a caller's value inside two
  nested arrays of objects — SendGrid's envelope shape, which previously could not be addressed at
  all: every segment was an object key, so the body came out as `{"personalizations": {"0": …}}`, a
  400 rather than a shorthand.

  **What it solves is the fixed-length envelope, not the caller-supplied batch**, and keeping those
  apart is the point. The indices come from the provider file, so an array's length is a property of
  the declaration. A batch — n items in, n elements built — would need the emitter to compute over
  caller data, which is the expression language `AGENTS.md` §Flow graph refuses. An index may address
  a whole element (`properties.title[0]`), which splits the difference: this repository supplies the
  array wrapper, the caller supplies the element.

  Five refusals, each because the alternative is a request the vendor answers with a 200 and ignores:
  a hole in the indices, a bare numeric segment (which used to build an object keyed `"0"`), an array
  directly inside an array, a root-level array, and an indexed path under `form` encoding.


## [0.11.0] — 2026-08-02

### Changed — breaking for `codewandler-connector-spec` consumers

- **`Service` gained a public `tags` field** (C-153), and the struct is not `#[non_exhaustive]`, so
  any downstream code constructing a `Service` with a struct literal will no longer compile. Four
  literals inside this workspace needed the field added. Add `tags: Vec::new()` (or `..Default::default()`).
  Serialization is unaffected — the field is `skip_serializing_if`, so a service declaring no tag
  encodes exactly as before.

### Documented

- **The Managed Agents surface, inventoried before any TOML — and it contradicts its own epic's
  premise in three places** (C-445). Eighty endpoints catalogued with a carry/withhold decision and a
  reason for every withheld one. The three findings are why the inventory came first:
  the SSE and webhook **event vocabularies collide by name** (`session.status_terminated`,
  `session.status_rescheduled` and `session.thread_created` appear in both with different payloads;
  `session.status_idle` / `session.status_idled` differ by a letter), and a service is one member
  namespace — so "two bindings on one service, the shape slack proved" was wrong, because slack's two
  bindings share *one* vocabulary. The beta header must be **per-operation** `const_headers`, not
  provider-level: `distribute_const_headers` (`crates/connector-spec/src/provider.rs:2010`) copies the
  provider table onto every operation, which would beta-gate the five existing non-beta Anthropic
  reads. And the "management plane yes, session plane later" middle is **not** obviously safe — the
  vendor treats agents and environments as control-plane resources applied once from CI, so a
  management-only connector risks a catalogued surface with no caller.

  No provider TOML was written, which is the outcome C-130's precedent exists to make legible.

### Added

- **The host's explorer is an operator console, not a catalogue viewer** (C-237). Opening a connector
  no longer fires ~30 requests to render a list the host already sent whole — `operations[]` carries
  every field those responses did, so expanding one operation fetches exactly that one. Operations are
  grouped by service with idempotency and hosts shown; the connector list is searchable by connector,
  vendor and operation id, and filterable down to what still needs setup. A parameter editor refuses
  invalid JSON before it reaches a vendor, and a **dry run** shows the composed request without
  sending it — reaching no socket and no secret store, placing a credential *reference* where the
  value would go.

  Resumed from a preserved WIP commit after its first implementor was stopped mid-gate. The three
  safety constraints that session left unproven were unproven only because a fresh worktree has no
  `node_modules`; they are real tests and they pass.


- **The Anthropic Admin service goes from three reads to nine** (C-441). Organization members (list
  and get), workspace members (list and get), one workspace by id, and outstanding invites. Every one
  is an authenticated, idempotent `GET` against `anthropic.admin_key`, and every one is
  **unparameterized** — C-30 leaves no query encodable, so each list declares its `first_id` /
  `last_id` / `has_more` cursor fields as *unusable here* and says in its description that the call is
  unpaginated. No write was added: the mutating surface still needs a request/response shape decision
  nobody has made.

  **Every field that names or contacts a person carries the personal-data sentence**, pinned by JSON
  Pointer in `crates/connector-spec/tests/anthropic_admin_surface.rs` rather than left to review. Two
  shipped lists (`anthropic-workspaces-list`, `anthropic-api-keys-list`) had bare cursor fields with
  no descriptions and were brought up to the same convention.


- **A service says what kind of thing it is, so 54 providers can be filtered by domain** (C-153).
  `Tag` is a closed 27-value vocabulary on a service, beside `roles` — `telephony` for babelforce and
  twilio, `payments` for stripe, and `email`/`scheduling`/`storage` split across google's three
  services, which is why the field hangs off a service and the provider's is derived. The vocabulary
  was read off all 54 shipped providers rather than designed ahead of them, and the clustering is
  recorded in [provider-roles.md](docs/designs/provider-roles.md) — including why seven singletons
  were kept rather than folded into parents that describe them worse.

  **A tag is deliberately not a role.** A role answers "can this service *do* X, checkably?" and
  carries required members the loader verifies; a tag answers "what kind of thing is this?" and
  carries nothing, because no operation makes a service `storage`. The vocabulary is closed anyway: a
  typo'd tag never matches a filter and nothing downstream can notice.

  Landed as **partial** — the declaration ships, the consumer does not. Tags reach no artifact yet,
  and neither has `roles` since C-120; both are now C-442, because building a tag-only path would
  have left `roles` dead beside it and pre-empted the mapping `connector-surfaces.md` owns.

### Fixed

- **A route on the host's auth surface was gated but not pinned** (C-237, found in review). The new
  dry-run route took a `Principal` like every other `/v1` route, but `tests/tenancy.rs`'s enumeration
  was not extended, so removing the gate would have failed no test — the same omission that file's own
  comment already records against C-204. Added at integration and falsified: dropping the `Principal`
  now fails with *"POST /v1/operations/anthropic-models-list/dry-run answered without a session"*.


- **`connector-cli`'s scaffold would have mistaken a tagged single-surface provider for a
  multi-service one** (C-153). `declares_services` asked `!services.is_empty()` when it meant "declares
  a *named* service". Once a provider carries a `default` entry to hold its tags, that reads as a
  named service and the scaffold emits a blocked note naming a service that does not exist. It now
  asks `is_default_only()`. Three tests spelled the same claim the same wrong way and were corrected
  with it.

## [0.10.1] — 2026-08-01

### Fixed

- **One credential fact, two dispositions, and the loader refuses an operation claiming both**
  (C-432). Two declarations had grown up describing the same thing with opposite consequences:
  C-430's `credential_response` marks a response as carrying a credential and **withholds** the
  operation; C-136's `produces_credential` marks one and **ships** it, returning a handle. An
  operation declaring both was a contradiction nothing caught. It is now refused, and the refusal
  names which declaration governs and why — selected by *purpose* (does this operation exist to mint
  a credential?) rather than by the shape of the response.

### Changed

- **No vendor logo will be vendored, and no `logo_url` declared** (C-437, closing C-40). A brand
  guideline grants *identification use* to the party **displaying** a mark — revocably,
  non-transferably, non-sublicensably, conditioned on not modifying it. `LICENSE-MIT` and
  `LICENSE-APACHE` grant *copy, modify and sublicense*, perpetually, to everyone, over everything
  here. Vendoring puts those in direct contradiction over bytes this project does not own, and git
  history means a revocation could not be honoured. A `logo_url` was refused separately, on the
  privacy fact: 54 `<img src>` elements fire third-party requests from every visitor's browser before
  anyone has chosen anything. A listing individualises a connector with a monogram derived from the
  published `vendor` and `id`, or brings its own asset pack.

  C-415's spec-vendoring split does not transfer, and the reason is worth keeping: an OpenAPI
  document is published *in order to be* implemented against, so scrubbing what must not travel makes
  the bytes publishable. A trademark exists *in order not to be* copied — there is nothing in the
  file to scrub, because the file is the problem.

- **What flux's credential boundary actually keys on is now recorded** (C-432), because a design was
  about to be built on a premise that does not hold here. `PlatformSourcing` is an opt-in to
  **refusal**, not a permit — `None` is the default and the only other states, `Operation` and
  `Activation`, are what *turn refusal on*. There is no state meaning "this response carries a
  credential and that is expected". And the boundary applies to a plugin `OperationSpec` arriving
  over the NDJSON plugin protocol, which is a seam this repository's `.flux` module and
  `.connector.toml` never reach.

## [0.10.0] — 2026-08-01

### Added

- **One collected value can reach more than one request position, and Algolia ships** (C-229). Algolia
  wants its application id in **two** places — the hostname and a header on every call — and a
  configuration field could bind only one. The loader was right to refuse the alternative by name
  (*"Two questions that share an answer are one question"*), so the C-164 implementor refused to ship
  rather than ask a person the same question twice. That refusal still fires; what is new is the other
  half.

  `also_binds` is a **head plus destinations**, not a list of peers, and the reason is load-bearing: a
  list has no head, so the placeholder rule would be conditional and the slot would not provably be
  one. With a head, `binds`' own target is the slot everywhere, and every field that existed before is
  byte-for-byte unchanged.

  The value is validated against **each** destination it reaches and substituted unencoded — the
  intersection of every position's rule, host rule included. A value that passes as a path, a query and
  a header but is not a safe hostname is refused, which is the case no single position would catch.

  **Algolia is the 54th provider**: five curated operations, 937 → 945 artifacts.

### Added

- **A credential-producing operation returns a handle, never the secret** (C-136). An operation's
  result becomes a session value a model can read and a log can print, so a login that *returns* its
  token has already lost — and redaction cannot save it, because the host's redactor holds only values
  the host itself resolved and cannot know a secret minted by the very call returning it.
  `produces_credential` names the secret field and the `CredentialRef` it is stored under; the declared
  output is the **handle**, and the secret is absent from the effective output entirely.

  **Seven refusals, not the three the story asked for.** A response schema still exposing the secret; no
  secret field named; `idempotent` declared (minting is a write and some vendors invalidate the previous
  token); plus an undeclared credential, a connector with no `authority`, two operations minting one
  credential, and a wildcard location — each a case where the first three could not be enforced at all.

  **The module path is closed by refusal, and that is the honest outcome.** Review found that the
  diversion existed only on the host path while the *emitted Flux* still performed the login and
  returned the vendor's response — the two artifacts disagreeing, with the Flux one being what runs.
  Teaching the emitter is forbidden by the invariant it would violate and unimplementable anyway, since
  the diversion is a write to a bound port and Flux holds no handle on the credential store. So a
  connector declaring `produces_credential` **no longer builds**, and the refusal states the true thing:
  this execution format cannot express a credential-producing operation. The test pins it with the
  unmodified operation's emitted body as the control — the failure quotes the `return response` a login
  would have shipped.

  **It does not restore the four operations withheld in v0.9.1.** Those return a credential
  *incidentally*, beside the meeting or the server the operation exists to deliver, so diverting the
  field would delete the answer rather than the exposure — and one of them returns N servers' tokens,
  which a single credential name cannot address. They remain C-79's.

### Fixed

- **The embedded catalogue can say *why* an operation names no credential** (C-235). It emitted `[]`
  for both a positively-public operation and one whose credential is deliberately withheld, so no host
  linking the catalogue could tell them apart. `CredentialRequirement` now distinguishes `Declared`,
  `Withheld` and public, rendered from a **single** classifier both backends read — the two words are
  C-206's published vocabulary rather than a third spelling of the same idea.

  **The half that was actually wrong was the host, not the published document.** `status::of` has gated
  the freshdesk wording correctly since C-206 landed; what a person met was `connectors-api` serving
  freshdesk as `no-credential-required` — "nothing to supply", which an operator reads as *ready* —
  while every call 401s. That is now a state of its own, recorded in `providers/freshdesk.toml`.

  **`is_callable` stops treating an empty mechanism list as "callable by anyone".** Freshdesk's nine
  operations move from `callable: true` to `false`, because an unauthenticated request to an endpoint
  that wants a credential is a 401 — the old value was a lie in the same family as the one this story
  is about.

  `web/public/catalog.json` and the catalogue index are **byte-identical**; the 53 per-provider tables
  and the lockfile carry the new fact, and the artifact count is unmoved at 937.

### Added

- **A signature can cover the request URL and its reassembled form fields** (C-188). `HmacSpec::signed`
  admitted `{body}` and `{timestamp}` and nothing else, so Twilio — which signs the **URL** plus its
  sorted, percent-decoded form fields — shipped its events with **no channel binding at all**. That was
  the honest outcome and it is now closed: the shipped `providers/twilio.toml` binding, read through the
  real loader, reproduces Twilio's own published signature `L/OH5YylLD5NRKLltdqwSvS0BnU=` from its
  documented URL, body and auth token, with no vendor branch anywhere in the verifier.

  **The template gained the name of a derivation, not the ability to perform one.** `{sorted_form}` is a
  closed placeholder the host resolves; the template still has no operator, no repetition and no
  ordering primitive. Twilio's file kept its original objection — that a template cannot re-sort N form
  fields — because it is still true.

  **C-141's failure mode is now structurally impossible rather than merely unlikely.** The rule is that
  a signed template must cover a *payload* placeholder, not that it must contain the literal `{body}`,
  and two tests demonstrate the forgery first and then refuse it: a template covering only the URL, and
  one omitting the body, each verify a forged payload before the guard rejects them.

  A **repeated form field name is refused rather than guessed** — `a=1&a=2` has no defined answer, and
  Twilio's own helpers build a map, so the winner is language-dependent. The verifier reports that it
  cannot verify instead of picking one.

  **Known limit, stated rather than implied:** flux's `verify` block does not know these placeholders
  yet, so Twilio's binding is correctly declared and not yet actionable by a host. And `{url}` carries a
  deployment hazard — behind a proxy the host sees a rewritten URL while the signature covers the
  configured one.

## [0.9.1] — 2026-08-01

### Added

- **A configuration field can declare a closed set of values** (C-225). A two-choice region rendered as
  free text, so a wrong answer looked exactly like a bad key — the user got a 401 and no way to tell
  which mistake they had made. Two vendors were waiting on it, and `providers/intercom.toml` recorded
  *"the regional hosts are not selectable"* as an open gap in the file itself. Both now declare their
  set, and that comment is gone.

  Each choice carries a **label**, so a UI shows a person "European Union" rather than
  `api.eu.newrelic.com`. A value outside the set is refused where it is supplied, and the refusal names
  the permitted answers. The set is **published** to the manifest and `catalog.json`, because a
  hosted product that cannot read the choices still renders a text box. Every permitted value is
  checked against the field's own `format`, and against its request position where it is pinned.

  A stored value that later leaves the set is deliberately **not** re-validated on the way out: this is
  a write-path check, and silently rejecting a credential that was valid when it was saved is a worse
  failure than the one it would prevent.

  Intercom's `base_url` becomes `https://{host}`, so an EU or AU workspace can be connected at all —
  a behaviour change to a shipped connector, taken deliberately, and it fails closed with
  `MissingConfig` naming the field rather than defaulting to a region.

- **`scripts/cut-release.sh` cuts a release in one transactional command** (C-427). Cutting v0.9.0 was
  nine hand-run steps, and the dangerous one is the bump: this repository is a compiler whose output
  records its own version, so **120 generated manifests carry `generator = "flux-connectors <version>"`**
  and `connectors.lock` hashes them. A bump without a regenerate in the same commit leaves the tree
  inconsistent with itself, and it surfaces *after* the commit. Doing it by hand rewrote 184 artifacts.

  A red gate restores the tree exactly, so a failed cut is safe to re-run, and the script stages **only**
  the release files — a property that matters in a repository where several sessions work at once, as
  one did during this very run.

  **A transactional hole in the script this was ported from was found by hitting it.** An `EXIT` trap
  alone does not run for an untrapped fatal signal, so a `SIGPIPE` mid-cut left the changelogs promoted
  and the manifest bumped with the snapshot restoring nothing. Fixed here with
  `trap 'exit N' HUP INT PIPE TERM`, and a test fails without those four lines. **The upstream script
  has the same gap.**

  It does not push and does not publish. Pushing the tag *is* the crates.io publication, so the script
  prepares that moment and never takes it by accident.

### Added

- **`build` sees the artifact no plan claims, and refuses to ship it** (C-429). `build` and `diff`
  compared each *planned* artifact against what was committed and had no view of the inverse: a file
  under an artifact root that **no plan claims**. A rendering whose operation was deselected was never
  looked at again, and `diff` reported `N artifacts up to date` with a straight face. It bit **nine
  times across three stories** in a single day, every one deleted by hand — which worked only because
  the diffs happened to be under review.

  **It refuses; it does not remove.** A root is *derived* from what the emitter says it writes, so an
  emitter bug is a mis-derived root, and deleting on one turns a bug into data loss. Removal is the
  cheap half — one `git rm`, reviewed in the same diff as the change that orphaned the file — while
  *noticing* was the expensive half. The refusal lands before any write, so a refused build leaves the
  tree byte-identical.

  Roots are **derived, never listed**: every planned artifact now declares which directory family it
  belongs to, and that is a required argument, so an artifact cannot reach the tree without its author
  answering the question. A singleton's directory is deliberately not a root, which is what keeps
  `Cargo.lock` and `crates/catalog/src/lib.rs` out of the report.

### Fixed

- **Four operations that returned a secret are withheld, and a declaration now gates it** (C-430).
  Each was condemned by its own connector's description. Postmark's `ApiTokens` is *"ACCOUNT-PRIVILEGED.
  This server's own live Server Token(s), **in plaintext** — the Account API's own mechanism for
  retrieving one"*; Zoom's `start_url` *"embeds the host's ZAK token: anyone holding this URL starts
  the meeting as its host"*. Both connectors documented the hazard precisely and returned the field
  anyway. **Describing a credential is not withholding it.**

  **Stripping the field was considered and is strictly worse**, which is the finding worth keeping:
  the emitter returns the response whole and the pack hands back what the transport produced, so
  removing a location from the published schema deletes the *disclosure* and leaves the *exposure*.
  The operation is withheld instead, until C-136's diversion can return a handle rather than a secret.

  The gate reads a **declaration**, never a field name. A catalogue-wide scan of 31 name-shaped hits
  had 28 false positives — Klaviyo's `public_api_key` (*"Public by design"*), Typeform's `token`
  (*"This response's own opaque id"*), Okta's `credentials` (*"Never carries a password or a secret
  value"*), Anthropic's `max_input_tokens` — every one correctly documented by its connector. A regex
  would fail all of them and teach authors to fight the gate. Its resolver walks `properties` **and**
  `items`, because the Postmark pair was missed by a one-level scan and only found on a second pass:
  `/Servers/*/ApiTokens` resolves, `/ApiTokens` does not.

  Postmark loses its whole `account` service with them — they were its only two operations, and with
  the service goes the credential it needed and the configuration field that asked a human for it. A
  connector asks for everything it needs and nothing it cannot use.

  Operations 678 → 674, artifacts 943 → 937, response-schema coverage floor 610 → 606.

### Changed

- **The flux engine line moves to 0.47, and nothing in this repository changed with it** (C-431).
  The successor [C-428](docs/stories/C-428-move-the-flux-pin-to-0-46.md) wrote out when it deferred
  0.47 during a release cut. All six pins move together — `flux-lang`, `flux-core`, `flux-runtime`,
  `flux-web`, `flux-system`, `flux-credentials`, plus `ENGINE_LINE`, where the line is recorded once;
  `SPEC_LINE` stays `1.3`, because the wire vocabulary a guest plugin compiles against is a different
  promise. `cargo update --workspace` moved all eleven `codewandler-flux-*` packages 0.46.0 → 0.47.1
  and nothing else.

  The reason is compatibility, not a fix. `connector-pack` hands a host
  `Arc<dyn flux_runtime::Tool>`, and a `0.x` requirement is `>=0.N.0, <0.N+1.0` to cargo — so a pack
  built on 0.46 and a host on 0.47 are **two unrelated traits in one graph**. Staying a minor behind
  is not the conservative option; it is the unlinkable one.

  **What 0.47 changed at this boundary was measured, not read off a changelog.** flux 0.47.0 fixed
  the credential boundary for a host-dispatched plugin response and gated three ungated analyzer
  doors; 0.47.1 only re-shipped binaries a broken release workflow had dropped. A `diff -rq` over the
  vendored sources cargo resolved reports **no file differing** in any of the six engine crates, nor
  in `flux-provider`, `flux-config`, `flux-skill` or `flux-markdown`. Across the whole resolved
  closure exactly two files move, both in `flux-plugin`: a fixture binary, and 39 lines of *comment*
  in `src/host/credential_boundary.rs`. The executable halves live in flux's own binary, which
  nothing here links.

  So the three green results are explained rather than merely observed: 1490 tests pass,
  `diff` reports `937 artifacts up to date (53 providers checked)`, and `flux-lang` 0.47.1 emits
  byte-identical Flux because it *is* `flux-lang` 0.46.0. The multipart impossibility C-426
  established was re-verified against the 0.47.1 sources at the same time and still holds:
  `http.request`'s `body` is `{"type": "string"}` and `parse`'s `as_type` is the same closed list of
  six.

- **The babelforce auth-flow endpoints are withheld, and multipart is established as impossible**
  (C-426). Four operations that shipped in v0.9.0 no longer do: `babelforce-authorize`,
  `babelforce-revoke`, `babelforce-get-user-customer`, and — already withheld in v0.9.0 —
  `/oauth/token`. **An authentication endpoint is never a connector operation**: it describes *how to
  authenticate*, which the host performs, not something a caller invokes and reads a result from.
  `babelforce-authorize` was the PKCE browser redirect; `babelforce-revoke` took a `client_secret` as
  a plain operation argument.

  `babelforce-get-user-customer` was withheld for the independent reason that its response carries a
  credential — and it was verified against the document rather than accepted on a field name. The
  containing schema is described *by the vendor* as **"REST API access credentials"**, and it carries
  a second credential a name-scan would have missed, `stream.token` (*"Push API token"*). The
  `format: uuid` description that would have excused it is contradicted by the document's own
  example: 32 undashed hex characters, not a UUID.

  The `auth` service is gone with them — a declared service with zero operations emits an empty
  module, which an existing invariant refuses. **Drift detection on that document is not lost**: its
  hashes stay in the provenance file and are checked against its bytes independently of the provider.

  **`multipart/form-data` cannot be emitted, and the story stopped rather than pretending otherwise.**
  Established against the pinned engine line before any emitter was written: `http.request`'s `body`
  is declared `{"type": "string"}` and read with `as_str`, so a structured body is silently dropped to
  *no body at all*; `parse`'s `as_type` is a closed, analyzer-enforced list that does not contain
  `multipart`, so such a module could never pass the parse-and-analyze gate; and `multipart` appears
  nowhere in flux 0.46 at all. The five uploads stay excluded with that as their recorded reason, and
  the fix is an upstream flux encoder. Adding the IR variant would have let a connector describe a
  request no emitted module could perform.

  babelforce is now **388 emitted + 5 inexpressible + 4 withheld = 397**, three categories counted
  from each exclusion's own reason rather than by position.

### Added

- **`connector-cli scaffold` writes the patch set from a vendored document** (C-419). The manifest side
  of the spec front-end was done — a selector, a naming rule, risk by selector, omission, exposure —
  but a human still had to *write* those statements. This writes them, to **stdout**, so pointing a
  connector at a spec is a review of generated text rather than an authoring job.

  **The design decision that matters is not in the story: a claim a human already made is carried
  forward; a claim nobody has made is a hole.** A `[[patch.select]]` restates `risk`/`idempotency`
  only when every operation it matches is already published *and* they agree; one unclaimed operation
  and it states neither, while every claimed sibling keeps its own block and the loader refuses over
  the gap alone. Nothing is ever derived from an HTTP method. Without that, converting the other 52
  providers would have thrown away **254 reviewed safety claims** and become a re-authoring job.

  It refuses to propose an authentication endpoint at all, applying the ruling recorded the same day —
  `/oauth`, `/oauth2`, `/openid`, `/.well-known` are withheld by not being selected, since
  `expose = false` is not the mechanism. A path merely *ending* on `token` or `authorize` is
  **reported and not withheld**: only the vendor's prose settles it, and a heuristic deciding what a
  connector offers is what this repository does not do.

  `--diff` reports the document against the connector as it stands — `2 added, 0 removed, 2 changed,
  389 unchanged` for babelforce — which is what makes a re-build repeatable rather than a migration.
  Every run ends with what it could not carry, counted: 5 multipart uploads dropped, 23 operations
  with no description, 3 auth-flow endpoints withheld and 1 ambiguous one reported for a human.

- **The published crates are proved consumable from outside the workspace** (C-190). The claim that a
  consumer writes `use catalog::…` and `use connector_pack::…` rather than the `codewandler-` package
  names had never been exercised from outside; nor had the engine-line coupling. A scratch crate
  built against the **registry** now does both, and `docs/integrating-with-flux.md` carries the exact
  `Cargo.toml` a consumer needs.

  **The flux engine line is not optional and is now written down.** `connector-pack` hands a host a
  `ToolRegistry`, so a consumer must link the *same* flux line — `^0.46` for 0.9.0. Proved by a
  negative probe rather than asserted: a consumer on 0.45 fails with *"expected
  `flux_runtime::ToolRegistry`, found `ToolRegistry` … there are multiple different versions of crate
  `flux_runtime` in the dependency graph"*. That is the failure this documentation exists to prevent,
  demonstrated instead of described.

  Two further facts established from a consumer's **resolved graph** rather than from a manifest:
  `connector-secrets` pulls no HTTP client by default (no `reqwest`, no rustls, no hyper, no
  openssl), and none of the four published crates supplies an `http.request` implementation — a host
  brings its own transport.

## [0.9.0] — 2026-08-01

### Fixed

- **A credential address can name which connection it belongs to** (C-406). Two Zendesk subdomains
  for one tenant rendered **one address**, so the second connection silently overwrote the first and
  every later call resolved whichever credential survived — a `200` from the wrong account, with no
  compile signal and no runtime error. The grammar gains an optional instance level,
  `tenants/<tenant>/<authority>[/@instances/<uuid>][/<service>]/<credential>`.

  The `@` in `@instances` is the argument, not decoration: a uuid is a well-formed service name, so
  a bare `…/<authority>/<uuid>/<credential>` would be two addresses wearing one spelling. No
  component grammar admits `@`, so the level cannot be forged and no vendor name is reserved away.

  Additive by construction — a tenant holding one connection renders **byte-identically** to before,
  asserted against a written-out literal rather than against the renderer, because an address that
  shifted would strand every credential already stored. The ambiguous case — several connections and
  none named — **refuses and lists the uuids that would have worked**, rather than guessing.

### Changed

- **The flux engine line moves to 0.46** (C-428). Raised in review: flux released 0.46.0 and this
  repository pinned 0.45. flux treats the minor position as the breaking signal at `0.y`, so `^0.45`
  does not resolve it — and `connector-pack` hands a host `Arc<dyn flux_runtime::Tool>`, so a pack
  built against 0.45 and a host on 0.46 resolve two copies of the runtime and the types do not unify.
  That made the bump a prerequisite for this release rather than a follow-up: publishing on 0.45
  would have shipped a crate its intended consumer cannot link.

  All six pins move together — a split line is worse than a stale one, which is what
  `flux_engine_line.rs` exists to refuse. **No generated artifact moved**: `flux-lang` owns the
  formatter this repository emits through, and all 948 artifacts are byte-identical under 0.46.

- **babelforce carries manager-sdk's whole API surface: 391 operations** (C-417). The connector went
  from 9 curated operations to **391 emitted**, which reconciles against manager-sdk's canonical 397
  as `391 + 5 inexpressible + 1 withheld`. It is described in **751 lines with 247 declarations** —
  against roughly 6,000 hand-authored, and more than 1,600 as one patch block per operation.

  **9 are exposed to a model; 382 are callable and unexposed.** That distinction is the entire reason
  C-413 exists: the full SDK surface is reachable by name through the host, while a model's tool
  catalogue stays the curated set it always was. The nine keep their contract byte-for-byte — ids,
  methods, paths, risk and idempotency all unmoved, verified against the previous release.

  **`POST /oauth/token` is deliberately withheld**, and it is the one operation manager-sdk covers
  that this connector does not. Its declared 2xx response carries `access_token` and `refresh_token`
  as required fields — the response *is* a credential. `expose = false` is not sufficient protection,
  because the execute route resolves an operation by name regardless of exposure (by design, C-413),
  and the host's redactor holds only values the host itself resolved, so a token minted by that call
  is unknown to it until after it has arrived. Shipping the first credential-minting response into
  the catalogue one story before the diversion mechanism (C-136) would invert the order that story
  exists to impose. The exclusion is recorded beside its reason and names what would let it back.

  A vendor document that publishes a response schema constraining nothing — a bare
  `{"type": "object"}` — is now dropped at ingest with a diagnostic rather than recorded as coverage.
  C-126 has refused that shape from a human author since it landed; ingest was laundering it in from
  a vendor. Zero shipped operations moved, verified across all 53 providers.

  Response-schema coverage moves accordingly: `COVERED_FLOOR` 277 → **611**, `ABSENCE_CEILING`
  24 → **71**.

- **The compiler leaves the crates.io publish closure** (C-407). `connector-secrets` re-exported
  `CredentialRef`, so `connector-spec` was in its public API — and `connector-spec` is the connector
  IR, both front-ends, validation and the lockfile writer: **11,832 lines with 128 public items,
  published so that a credential address would resolve.** The roadmap recorded that as a fact of
  life; it was a dependency-direction problem.

  The address vocabulary moves to `codewandler-connector-address`, which both `connector-spec` and
  `connector-secrets` depend on. `address.rs` travelled with `credential.rs` and that was not
  optional — `CredentialRef::build` calls the address validators, so leaving it behind would have
  made the new crate depend on the compiler, which is the cycle the extraction exists to break. The
  closure is still four crates; which four changed:

  ```
  codewandler-connector-address → codewandler-connector-catalog
    → codewandler-connector-secrets → codewandler-connector-pack
  ```

  Consumers see no change: `connector-secrets` re-exports the same nine names, and `connector-spec`
  re-exports the vocabulary so `connector_spec::credential::…` and `connector_spec::address::…` still
  resolve everywhere they already did. The proof is a test over the **derived** closure —
  `publish_closure.rs::no_machinery_crate_is_published` — rather than a hand-written list, so the
  edge cannot grow back unnoticed.

  **This does not undo what is already out.** `codewandler-connector-spec` 0.7.0 and 0.8.0 are live
  on crates.io and cannot be withdrawn. C-407 stops the next version shipping.

### Added

- **One statement selects many operations, names them, and states their risk** (C-411, C-412, C-414).
  Three stories describing one declaration, landed together because they are one struct. The result is
  the number the epic was built to produce: **the whole babelforce surface selects in 175 declaration
  lines**, of which 135 are the selection itself — 13 `[[patch.select]]` blocks, a naming rule with
  nine compatibility pins, and 10 per-operation exception blocks. One block per operation would have
  been **north of 1,600 lines**.

  - **`[[patch.select]]`** matches by service, path prefix and method, on **whole path segments**.
    Selection stays opt-in: there is no `hide` key and a test asserts there never is one. A selector
    matching nothing is refused, and two overlapping selectors that *disagree* are refused while two
    that agree are not — silence is not disagreement.
  - **`[patch.naming]`** derives an op id from `operationId` through one declared rule, with pins for
    exceptions. Precedence is total: `rename`, then a pin, then the rule. Measured against the real
    documents, the rule derives **397 distinct ids from 398 operationIds** — exactly one collision,
    `getUser`, which babelforce declares in two different documents. Collisions **refuse**; an
    `operationId` that cannot produce a legal name is reported rather than mangled, because
    non-alphanumerics are passed through precisely so the result fails.
  - **Risk and idempotency by selector**, with silence on a mutating method refusing by name. The one
    default in the whole overlay is that a *selector-matched* read may go unstated — `low` and
    `idempotent` are not flattering guesses for a `GET`, they are the only values a read can have. A
    `[[patch.operations]]` block still states both, so no existing provider moves.
  - **C-186 has no bulk escape hatch**: a selector carries no `repeatable_because` key at all, so a
    bulk `conditional` arrives with no condition and is refused per operation, by name.
  - **No operation on a path carrying an `internal` segment is ever selected.** Zero exist across the
    five documents, so this is a guard against a future spec pull rather than a filter.

- **`connectors.lock` is written** (C-189). Three CHANGELOG entries asserted byte-identity of a file
  that **did not exist**. `lock.rs` was a complete, tested hash domain whose *writer* was never built,
  so provenance was computed and discarded and drift detection — vision principle 1 — was unenforced
  across the whole catalogue. The file now exists: 53 provider rows, written by `build`, checked by
  `diff` as the 558th artifact, and a fixed point (a rebuild reproduces it byte-for-byte).

  The decision it forced was **write it, not delete the design** — and the timing is the argument.
  babelforce became the first spec-backed connector the same day, pinning a vendored document by
  `sha256`, with more OpenAPI-sourced providers intended. Deleting the design would have meant
  deciding where drift detection lives instead, on the day the first connector started needing it.

  **A multi-document connector would have been recorded with no spec hash at all** — a row that looks
  complete and detects nothing — because `Provenance::spec_sha256` is `None` once a connector declares
  several documents (C-410). `LockEntry` gained a per-document list to close that. It is deliberately
  *not* `SpecSource`: that type carries `fetched_at`, and re-vendoring byte-identical bytes must not
  rewrite the lockfile, so the projection is the named place that field is dropped.

  Artifact keys are repository-relative paths rather than bare file names, because bare names
  collide: an operation named after its own provider would land on the same map key as the provider's
  module and silently drop a hash.

### Fixed

- **A component can say "this source does not publish that", instead of claiming the connector lacks
  it** (C-408). Two different facts rendered identically, and one of them was a red claim a service
  never made: `ProviderCard` rendered an absent `auth` as **"not configured"** in the danger colour on
  **every card**, and `OperationDetail` told a reader *"No safe credential configuration is available
  for this operation. Live calls are disabled."* — a true and important sentence for freshdesk, where
  a credential exists and is deliberately withheld, and a false one for a catalogue that simply
  carries no credentials field.

  The distinction is carried by the **document**, not by the component: a field a thinner source may
  omit is typed `Published<T> = T | null | undefined`, and one `published()` predicate is the only
  place either spelling of an absence is interpreted. No source-capability descriptor and no new prop
  — a component that learned *which source* it renders would be the same defect wearing a fix.
  Deliberate withholding still renders exactly as it did, in red; a three-way branch keeps them apart
  and a test asserts the middle branch stays an `else-if`, because merging it back for tidiness
  reinstates the bug.

  **One absence was worse than a wrong colour.** `signature()` called `operation.flux.split(…)`, so a
  page over a source omitting `flux` **threw** rather than misleading — and `OperationList`'s search
  called `operation.path.toLowerCase()`, the same crash one keystroke away. Six fields are covered,
  not the three the story named.

  `web/`'s own rendering is unchanged, measured rather than asserted: the base was rebuilt into a
  separate tree and all 1,241 files compared — **visible text of all 379 pages identical**. Byte
  identity is unobtainable for any change that adds a CSS rule, because Vue's scoped-style hash moves
  on every element of that component.

### Changed

- **babelforce is the first spec-backed connector** (C-416). `providers/babelforce.toml` now points at
  the vendored manager document and selects its nine operations through `[[patch.operations]]`
  instead of writing them out. This is the claim `docs/designs/connector-pipeline.md` has carried
  since C-2 — *"if patching a bad vendor spec turns out harder than hand-writing the integration, the
  whole premise needs revisiting"* — settled with a number rather than an impression:

  | | before | after |
  |---|---:|---:|
  | whole file | 533 lines | **420** |
  | declarations | 306 | **98** |
  | operation blocks | 293 | **80** (32.6 → 8.9 per operation) |

  **Zero parameter patches were needed.** The document's descriptions and schemas beat the hand
  transcription everywhere, and the one place they did not — a 38-parameter reporting endpoint with 18
  `filters.`-prefixed synonyms — is what C-422 was built for. The transferable finding is the shape of
  the saving: the old file paid ~5 lines to *describe* each of 14 kept parameters, the new one pays ~1
  line to *name* each of 24 dropped ones. **Curation by exclusion beats curation by transcription
  whenever a vendor documents more than half of what you want**, which for a 356-operation document is
  always.

  Byte-identity was **deliberately refused**: `connectors/babelforce.flux` grows because the document
  publishes response schemas for all nine operations where the hand-authored file had none — the exact
  gap C-126 recorded babelforce as the largest block of. Coverage moves 0/9 → 9/9, so the ratchet
  turns: `COVERED_FLOOR` 250 → **277** and `ABSENCE_CEILING` 33 → **24**, both at the measured figures.

  Two premises the conversion disproved, both recorded rather than quietly corrected: there is no
  POST/PUT disagreement on `babelforce-call-session-set` (both say `PUT`), and `servers[0]` is
  Production rather than staging, so the `base_url` comment's stated reason was wrong.

  **One change is unverified and is flagged as such.** `babelforce-call-session-set`'s request body:
  the hand-authored connector sends a bare map, the document declares a `{"variables": {…}}` wrapper.
  The document was taken, then the implementor re-examined and lowered its own confidence to roughly
  even — five other session-variable payloads in the same document are bare maps, one of them saying
  in prose "the body is a key/value map", and a bare `SessionVariables` component exists that this
  operation does not reference. **Both failure modes are silent**: non-`app.` keys are ignored, so the
  wrong shape either way returns `200 {"success": true}` and nothing notices. One live call settles
  it; nothing offline can.

### Fixed

- **Loading a provider file means the same thing everywhere** (C-421). `provider::load` took bytes and
  no spec cache, so a spec-backed provider loaded as a **zero-operation skeleton — successfully, with
  no error**. 91 files call it and **86 are tests**, so the moment one shipped provider converted to
  `[spec]`, 53 tests across 17 binaries went red. This blocked every remaining story in the epic.

  The pure entry point now **refuses** rather than answering with a skeleton. The alternative — a
  `documents` parameter on `load` — looks like it gives "load" one meaning, but every caller without a
  cache can only pass `&[]`, and `&[]` against a pinned `[spec]` already refuses one layer down. So it
  buys one signature and two meanings, the second spelled `&[]`, plus a vestigial argument on ~40
  golden-error tests. All 53 hand-authored providers keep loading byte-identically.

  **The part that makes conversion cheap is the test seam, not the refusal.** There was no shared way
  to load a shipped provider — 18 test binaries, each with its own loader, so one wrong convention was
  replicated everywhere. There is now one, reading the definition *and* every document under
  `specs/<name>/` and passing the whole cache so the pin is resolved where the pin is read.
  **Consequence: a provider converting to `[spec]` now needs no test change at all**, which is what
  makes the remaining conversions affordable rather than 53 × 53 test edits.

  47 of the 53 failures are resolved by the seam; three were hand-authored-shape assertions rewritten
  to hold in **both** front-ends, and the last two are response-schema ratchet constants that move
  with the conversion itself rather than ahead of it.

### Added

- **A patch can drop a parameter the vendor declares** (C-422). Measured, not anticipated: converting
  babelforce to the spec route was cheaper everywhere except one endpoint. The document declares **38
  query parameters** on `listReportingCalls` — **18 of them `filters.`-prefixed restatements** of
  names already present — where the hand-authored connector curated 14. Without omission, the one
  operation the conversion could not curate became a 38-argument tool full of exact synonyms, which is
  a tool a model has to choose between. This was the single place hand-authoring beat patching.

  `omit` is a **name list grouped by position**, not a flag per parameter: a three-line block per name
  would have cost 72 lines to remove 24 synonyms — more than the entire 293 → 54 saving the conversion
  won — and would have told a reviewer the same thing 24 times. It costs 7. Identity is still name
  *and* position; position became the table key rather than a field.

  Omission is **explicit and never inferred**, which lands the opposite way from `Patch` having no
  `hide` at the operation level, and deliberately: an operation reaching this point has already been
  selected, so the author is narrowing a stated intent rather than opting out of review.

  Two refusals beyond what the story asked, both the same sentence pointed elsewhere. A **path**
  parameter cannot be dropped whatever its `required` flag says — the path template keeps its
  placeholder, so `/tickets/{id}` with nothing to fill it is a URL nothing can complete. And
  **corrections apply before omissions**, so requiredness is judged as the *connector* states it: an
  author who believes the vendor's flag is wrong corrects it — a reviewable statement of its own — and
  is then free to drop the parameter. Without that ordering, a vendor that wrongly marks a filter
  required would pin that argument into the tool with no way out.

- **One connector can ingest many vendor documents, one per service** (C-410). One document per
  provider was never decided, it was assumed: `SpecSource.path` was a single string and
  `Provider::spec()` returned *the last file by stem*, which for babelforce would have selected the
  4-operation `user` document over the 356-operation `manager` one. babelforce publishes **five**
  documents across two API versions and two security models, so the assumption had to go before any
  of it could be described.

  `[spec]` and `[[spec]]` are one key in two TOML spellings, dispatched by a map-or-seq visitor
  rather than `#[serde(untagged)]` — untagged discards the `deny_unknown_fields` key list and toml's
  span, and this loader's error text is a deliverable, not a side effect.

  Each document joins a **declared** service and may not share one, because a service is one name
  namespace and two documents can publish the same `operationId`. `getUser` genuinely exists in both
  the manager and user documents; a patch names its service, and the duplicate check widened from
  `select` to `(service, select)`, so those are two operations rather than a silent collision.
  Verified end to end at real scale: all five documents in one connector emit five service modules,
  with both `getUser` operations distinct.

  `Provider::spec()` was **deleted** rather than taught to choose better — which document a connector
  compiles from is the provider file's decision, and discovery cannot make it correctly at all.

  Provenance is per document, each declared `sha256` checked against its own bytes, so drift can name
  *which* document moved. `LockEntry` is still single-document shaped and a multi-document connector
  therefore records no spec hash in the lockfile — stated here rather than discovered later; widening
  it belongs to C-7/C-14.

- **An operation can be callable without being an LLM tool** (C-413). `expose true` was a hard-coded
  literal in the emitter, so every generated operation reached a model as a tool. That is why
  babelforce ships 9 operations out of 163 — curation was the only lever, and it is the wrong lever
  for a connector that has to serve every caller. Two claims now come apart: **catalogued and
  callable**, and **advertised to a model**.

  The field defaults to exposed, so nothing shipped moved: all 557 artifacts stayed byte-identical
  and every `ir_sha256` is where it was.

  **What review caught, and it was the whole story inverted.** A `ToolRegistry` is both the
  advertisement surface a host hands a model and the resolution surface an execute route reads, so
  filtering the registry withheld the *call* as a side effect of withholding the *tool* — and
  `connectors-api`'s execute path is the only one in the workspace. An unexposed operation was
  catalogued, manifest-listed, documented as reachable by the published provider schema, and
  unreachable in fact. The answer is a second seam: `connector_pack::resolve` admits **one named
  operation regardless of exposure**, under the identical flux admission checks a packed tool passes
  — verified by an independent reviewer building a catalogue that contains an unexposed operation and
  proving both directions at runtime. `pack` stays model-facing.

  Three existing tests asserted that *every* embedded operation is exposed. They passed only because
  nothing had used the feature yet, and would have turned red together on the first provider that
  did; each now tests a durable invariant instead — the registered set is exactly the exposed set,
  and every operation resolves whether or not it is exposed.

  The predicate **fails open**: a rendering stating no `expose` reads as exposed, so the accidental
  failure mode is a tool staying visible rather than one silently vanishing.

- **A provider can point at a vendored OpenAPI document instead of writing every operation out**
  (C-4). The `[spec]` front-end was designed in C-2 and never built: `SpecSource` and `Patch` landed
  with C-3 and sat unused, and `connector-cli` refused every spec-backed provider with "spec ingest
  (story C-4), which is not wired yet". That refusal is deleted. JSON **and YAML** parse — every
  babelforce document is YAML — with `$ref` resolution including nested, repeated and cyclic refs.

  **Ingest makes everything available and selects nothing.** A pointer with no patch is a connector
  with **zero** operations, enforced structurally rather than by convention: an operation can only
  enter through the patch list. That is what stops a 398-operation vendor document from becoming 398
  LLM tools by default, and two tests hold it.

  **Two grades of failure, and the split is the design.** A document that is not OpenAPI 3.x fails
  the provider. One bad endpoint is a diagnostic naming method and path, and the operation is
  *skipped* — never ingested half-formed. There is no "ingest it without its body" path: a `POST`
  that quietly stopped sending a body is indistinguishable from a legitimately bodiless write.

  Measured on the real corpus: 393 operations ingested and 5 diagnosed out of 398, the manager
  document's 356 in 220 ms, largest `$ref` expansion 3,580 nodes against a 50,000 ceiling.

  **`[spec] path` decides which document is compiled** — found in review, and it was not merely
  unverified but *ignored*: the pin reached only an error label while the build took the last file in
  the directory. A provider pinning `manager-2026-07-10` beside `user-2026-06-25` emitted `getUser`
  out of the document it never named, exit 0, no diagnostic. `specs/<provider>/` is a cache of
  *versions of one document*, so this was the ordinary pin, not an exotic one. Resolution lives in
  the loader, because which document a connector compiles from is the provider file's decision and
  choosing it in the CLI was the defect itself. A pin resolving to nothing refuses and lists what the
  cache holds, and a declared `sha256` is now checked against the bytes actually ingested rather than
  copied past them into `connectors.lock`.

- **The five babelforce OpenAPI documents are vendored** (C-415), 890 KB under `specs/babelforce/` —
  the blocker `providers/babelforce.toml` has recorded since C-17, which said the spec was "not
  vendored in this repository, and it is not an oversight". The resolution splits what was one
  question into two: the **pulled bytes** are publishable, the **pull configuration** is not.
  `sources.json` and `pull.sh` hold an internal GitLab host and project ids and stay where they are;
  `source_url` is omitted rather than naming that host, and `sha256` carries the identity instead —
  which it has to, because `info.version` is the string `0.0.0-dev` on three of the five documents.

  Six literals were scrubbed by a script, none of them written into this repository — not even into
  the thing that removes them. Each is discovered from the source, replaced wherever it occurs under
  any key, and recorded as a **SHA-256** so an exact gate can exist without the repo holding the
  preimage: three credential values, two email addresses, one telephone number. Two of the six were
  reachable only because a value also appears under a plain `id:`, where a key-scoped rule cannot see
  it — which is why the shape gate and the digest gate both exist.

  Ten tests hold it, each mutation-tested rather than observed green: reinserting the `accessId`
  literal under a plain `id:` turns the digest gate red while the shape gate stays green.

  **A correction the vendoring itself produced:** `X-Auth-Access-Id`/`X-Auth-Access-Token` are not
  declared in any of the five documents. `providers/babelforce.toml` says at length that ingest must
  keep *seeing* the deprecated pair so drift-check keeps reporting on it — against these documents
  that is unsatisfiable. The connector's refusal to model the pair is unaffected and still correct;
  it is now enforced by upstream's silence rather than by our overlay.

- **The catalogue publishes each connector's runtime** (C-405). `http`, `socket`, `process`,
  `container`, `plugin` or `remote` — a closed set with `http` the default, reaching the manifest,
  `catalog.json` and the Rust catalogue. A host that must refuse a locally-executing runtime when it
  serves more than one tenant can now read that fact instead of deriving it: every shipped connector
  is HTTP, so the derivation was right today and would have gone silently wrong for exactly the case
  the refusal exists to catch. An unknown runtime is refused at parse and names the accepted set,
  never defaulted — a typo falling back to `http` is the failure this closes.

### Changed

- **The flux engine line moves from 0.41 to 0.45 (C-403).** All seven `codewandler-flux-*` pins
  advance, with `flux-spec` going `1.2` → `1.3` on its own `1.x` line. This is the change a consumer
  has been waiting on: `connector_pack::pack` hands out `Arc<dyn flux_runtime::Tool>`, and while this
  repository built against 0.41 no host on current flux could link the pack at all.

  **The emitted Flux is unchanged.** A full build rewrites 2 of 557 artifacts — the two README
  snippet SVGs — and the change is four `fill=` attributes: flux-lang 0.45 classifies a reference to
  a previously-bound local as `Op`, so `payload`, `content_type`, `url` and `response` take the
  identifier colour. Every text node is byte-identical, and no `.flux` module, `.connector.toml`,
  catalogue table or `catalog.json` moves.

  **The behavioural change to know about** is flux 0.43's: `http.request` returns a
  `{status, headers, body}` record instead of one flat string, and `Egress` returns it unchanged.
  `ToolResult`'s Rust type did not change, so a consumer that string-matched the old block gets **no
  compile error** — only different bytes. `connectors-api`'s `/execute` response text changes
  accordingly. The shape is now pinned by
  `connectors-api/tests/live_egress.rs::the_response_comes_back_as_a_record_not_a_flat_string`.

  **One lock movement is not a consequence of the seven bumps:** `codewandler-flux-secret` 1.0.1 →
  1.1.1. `flux-runtime` 0.45.0 declares `flux-secret = "1"` but calls `Redactor::try_add_secret`,
  which first exists in 1.1.0 — so any resolve that legally selects a 1.0.x fails to compile, as this
  repository's committed lock did. That is an upstream under-declaration; the requirement should be
  `"1.1"`. The 1.0.1 → 1.1.1 diff is strictly more redaction, so the movement is in the safe
  direction for the secrets invariant.

  This does **not** yet unblock a downstream host: published `connector-pack` 0.8.0 still requires
  `flux-runtime ^0.41`. Only the next release closes flux-exchange's X-11.

## [0.8.0] — 2026-08-01

### Added

- **This software identifies itself on every outgoing request (C-223).** Every request left the host
  with no `User-Agent` at all — `codewandler-flux-web` builds its client at two places and calls
  `ClientBuilder::user_agent` at neither, and reqwest sends no default. Resend **rejects** such a
  request with `403` while carrying a valid key: a status that says *authorization* when the cause is
  a missing header, so the operator rotates a key that was never wrong and nothing points at the real
  cause.

  The identity is set in `request::build` — the single funnel the live path, `DryRunTransport` and
  the rehearsal already share, so a rehearsal agreeing with the wire is structural rather than two
  paths kept in step. Deliberately **not** on the host's client: a client-level header is invisible
  to a dry run that holds no client, and the rehearsal would then describe a request the host does
  not make. A connector declaring its own still wins, matched case-insensitively because
  `Request::headers` is a `BTreeMap` and two casings would be two headers on the wire.

  Reviewed by dumping all 299 shipped operations before and after: **295 gained exactly one header,
  4 were unchanged, 0 anomalies** — method, URL, body and every other header byte-identical.

- **Credentials survive a restart (C-207).** The host held them in memory and the process exiting was
  the cleanup, so wiring a connector and restarting lost everything. They now live in a `0600` file
  inside a `0700` directory, with the mode set in the `open(2)`/`mkdir(2)` call rather than
  `chmod`-ed afterwards, re-checked on every open, and a **widened store refused rather than quietly
  repaired** — it was already exposed, and repairing it silently would hide that.

  Writes are atomic (`create_new` → `write` → `fsync` → `rename` → directory `fsync`) with the
  in-memory map rolled back on failure, so nothing resolves that is not on disk. **There is no
  encryption**, and the module docs, the README and the startup banner all say so in those words:
  values are hex-encoded, and hex is framing — it stops a newline forging a second entry and a
  careless `grep` matching a token — not protection.

  Verified against the running binary rather than only by test: mode bits confirmed by `strace` under
  `umask 000`, a widened file *and* a widened directory each refused and left widened, and an
  adversarial value containing a newline and a forged address line round-tripping as one entry with
  the forgery unresolvable.


- **A dev sign-in, so the app is usable without a Google registration (C-234).** `cargo run -p
  connectors-api -- --dev` mints a session through the same machinery a Google sign-in uses — same
  cookie attributes, same opacity, same tenant resolution — for an account labelled
  `DEVELOPER — NOT A REAL ACCOUNT` in tenant `dev-local`.

  Without the flag the route does **not exist**: `404` with an empty body, not `403`, because an
  absent route cannot be reached by a misconfiguration. Probed with 156 raw HTTP requests written on
  a bare socket so nothing normalised them — `/auth/%2564ev`, `/auth/x/../dev`, `/auth/dev%00`,
  method overrides, `X-Original-URL`, `X-Forwarded-*` — all refused, none set a cookie. No
  `id_token`, with any attacker-chosen `sub`, can reach the dev tenant: `from_claims` prepends a
  literal `google-` and `developer()` takes no arguments. The binary now also refuses unknown
  arguments, because the loopback-only bind is what makes the dev door safe enough to exist.

- **The first byte (C-202).** A test now sends one request through a real `HttpRequestTool` wrapped
  in `Egress` to a loopback server under test control, and asserts the vendor received exactly the
  `{ method, url, headers, body }` the pack built. The request path was a proposition asserted
  against stubs; it is now something that has sent.

  The loopback-versus-SSRF-guard tension — the host sets `PrivateNetAllow::None`, which refuses the
  very address such a test must reach — resolved as a one-host grant on one `App`, leaving
  `WebOptions::default()` and `App::new` untouched. The grant is proved load-bearing by running the
  same operation under `App::new` and requiring a refusal with nothing recorded by the vendor.

### Fixed

- **The declared MSRV is a checked claim (C-213).** `resolver = "2"` performs no MSRV-aware
  selection, so a caret requirement resolved to a version declaring a higher `rust-version` and
  nothing warned — it was caught by a person reading the lock. The workspace now uses
  `resolver = "3"`, proven rather than asserted: relaxing the pin back to a caret makes cargo report
  `Unchanged jsonwebtoken v10.3.0 (available: v10.4.0)` — it saw the MSRV-breaking version and
  declined it.

  **The new fence was red before it changed anything**, for a breach nobody had filed: `connectors-api`
  declared `rust-version` 1.87 while reaching `zip v8.6.0` through `flux-web` → `flux-plugin`, which
  requires 1.88. That declaration has been false since C-202 put `flux-web` in the graph, and no pin
  can make it true — every published `zip` 8.x declares 1.88. Corrected on that crate, which is
  `publish = false`; the workspace-level decision belongs to the owner and is left open.

  Recorded plainly: **CI compiles the declared MSRV nowhere.** All three workflows pin one toolchain
  far above it and no job builds on `rust-version`, so the four crates published to crates.io carry
  an MSRV nothing has ever checked. The fence asserts that gap rather than implying coverage.

- **A repeatable write must state the condition it depends on (C-186).** The story was filed because
  `check_write_metadata` derives write-ness from the HTTP verb, so a POST or PATCH that genuinely is
  safe to repeat could not say so. The investigation found the premise was **false**:
  `Idempotency::Conditional` on a POST always emitted. What actually blocked those connectors was
  this repository's own gloss on a flux-owned value — `Conditional` was documented here as
  *"idempotent only under a condition the caller supplies"*, and the refusal message repeated it — so
  connectors whose repeatability comes from what the **endpoint** does read it as out of reach and
  under-declared.

  So this **tightens** rather than relaxes. `Idempotent` on a POST or PATCH is refused
  unconditionally, and a mutating `Conditional` must now state its condition: flux says *"under
  stated conditions"*, and nothing was making anyone state them. Nine shipped connectors are
  corrected, six of them found during the rework at zero artifact cost because the condition reaches
  only `catalog.json`.

  Recorded and not resolved: flux's I3 coherence rule hits **twelve** mutating operations, not the
  three this story began with. The other nine are `PUT`s, permitted by RFC 9110 §9.2.2 and refused by
  flux, which ignores method entirely — replaying a PUT is safe, but *skipping* one is not. Pinned by
  a two-way count so it cannot grow unnoticed.

- **Resend inherits the versioned host identity (C-241).** It was the catalogue's sole `User-Agent`
  declaration and the worse of the two available values — no version, and the bare product word
  C-223's acceptance rules out. The vendor fact that made the workaround necessary is kept beside the
  connector: Resend answers `403` to a request carrying no `User-Agent`, which is why this connector
  surfaced the gap at all. The catalogue now declares none.

- **`connector-pack` is fenced against linking an HTTP client (C-199).** The guard reads the
  **feature-resolved** graph from `cargo metadata`, not `Cargo.lock`, because the two answer
  different questions: the existing lock fence exists precisely to catch optional dependencies, and
  this one exists precisely not to count them. The pack does reach `reqwest` through
  `connector-secrets`' Vault client, which is `optional`, `default = []` and requested by no
  workspace member — so a lock-idiom fence would have gone red on a correct build, which is what
  forced the change of instrument.

  Eight mutations, each reverted: a dev-dependency edge reddens only the dev-build test, and asking
  for `vault` from `connectors-api` reddens the *pack's* fence, so feature unification is measured
  rather than asserted.

- **The test suite could reach an operator's real credential store, and send a real request (C-207).**
  Found in security review. `App::new` honoured `CONNECTORS_CREDENTIAL_STORE`, which this crate's own
  README instructs an operator to export — so running the gate wrote their live store, and
  `tests/host.rs` then answered `200` instead of `400`, **dispatching an operation to
  `api.anthropic.com`** because a credential left by an earlier run resolved.

  Fixed structurally rather than by convention: exactly one constructor reads the environment, and it
  is the one `main.rs` calls. Clearing the variable in the test harness would have been a rule
  somebody has to remember.

  Two claims that could not fail were made falsifiable in the same pass — *"there is no silent
  fallback to memory"*, and `App::deployed`'s default, which was the whole of this story for the
  binary and which the story's own acceptance cited as its evidence. And a crash mid-write left a
  `0600` temporary holding the full credential that the documented revoke did not remove; temporaries
  are now reaped, and the revoke is `rm -r` on the directory everywhere it appears.

- **A whole-catalogue test that could not fail, and the blind spot behind it (C-232, C-233).**
  `every_shipped_operation_builds_an_absolute_request` manufactured a value for every variable the
  pack's own scan discovered, so its input came from the thing it was meant to check — it could never
  fail for a missing value. That is how eight GraphQL operations shipped in review with **zero**
  callable requests while `cargo test --workspace` was fully green.

  The root fix is upstream of the test: a brace in a bound string literal is now read as configuration
  only for the two kinds the module always *claimed* — a templated URL and a C-187 pin bind — and
  anything else is refused at **both** entry points. The scan can no longer invent a variable out of a
  vendor's syntax. The test then binds what a provider **declares**, and the empty-configuration case
  — the production shape, and the one that had never run once — now runs **43 times**.

  `connector-pack` also gains a rehearsal so a provider implementor can ask "can this connector
  compose a request at all?" before integration, which was structurally unanswerable. It constructs no
  `catalog::Operation`, so `#[non_exhaustive]` keeps its full guarantee.

  Two claims were corrected rather than defended. The pin-name grammar was reconciled toward the
  loader after review measured that `binds = "query.page.size"` loads, emits, and was then reported as
  "neither a URL nor a pin" — a wrong diagnosis for a literal that is exactly a pin. It now stops one
  clause short of the loader, because a JSON object literal necessarily quotes its keys and that
  clause is what separates `{page.size}` from `{"already": "json"}`. And C-233's own premise — that no
  synthetic `catalog::Operation` can be built outside the `catalog` crate — is true of *construction*
  and false of *copying*: the fields are `pub`, so a shipped entry can be cloned and doctored, which
  is how the second call site ended up pinned rather than merely documented.

- **The host serves three wiring states where a boolean carried two (C-212).** `connected` was
  `false` both for "supply a credential" and for "this vendor needs none" — two opposite situations,
  one value, in the view a person uses to choose among 53 connectors.

  It also fixes the second half: `all_stored` required **every** declared credential, so supplying
  Anthropic's `api_key` — which nearly every operation uses — left the connector reading as unwired
  because `admin_key`, a management-surface value no ordinary request carries, was unset. The code
  already contained the argument against itself, excluding inbound signing secrets for exactly that
  reason; the principle now holds by construction rather than as a special case, so Slack reads as
  wired on its bot token alone.

  Verified against a running host, not only by test: freshdesk `no-credential-required`; anthropic
  `not-wired` 0/5 → `partly-wired` 2/5 → `wired` 5/5. Uses C-206's own `no-credential-required`
  token rather than a second vocabulary for the same distinction.

- **The route-level login guard had no coverage anywhere, and now does (C-228).** C-204's fix is
  sound — a security review reproduced the cross-account capture at the base and proved it dead on
  the fix. This is the residue: when a new guard runs *before* an old one, tests aimed at the old one
  stop reaching it and keep passing.

  **The measurement came out worse than the story predicted.** The story said the route-level
  `take_login` refusal was "covered only by store-level unit tests". Deleting the entire guard at the
  merge base left **all 60 tests in the crate green, across all six binaries** — nothing anywhere
  observed that the issued-here/single-use check had been removed from the route.

  Split into two tests, each named for the branch it exercises: no cookie, and a cookie matching an
  unissued state. Every refusal in the callback answers `400` and clears the binding, so the *message*
  is the only observable that distinguishes the three branches — the three are now `pub const`s and
  the tests name the constant, because a test holding its own copy of the string would drift exactly
  as the original did. Also adds the missing negative test for `/v1/operations/{operation}`, and makes
  the no-secrets sweep in `tests/host.rs` fail on a `401` rather than pass on it.

- **An `example` on a secret configuration field is refused at load (C-231).** It was enforced by
  per-connector goodwill, and the gap was three times wider than it looked: 38 providers declare a
  secret field, **24 had a local test guarding it and 14 had nothing** — two dozen duplicated
  spellings of one rule that still left a third of the exposed surface uncovered.

  It lands as a **loader refusal** rather than a test, because these crates published today: a
  downstream author writing their own provider TOML is now a real person, and `provider::load` is the
  only form of the rule that reaches them. **This rejects input the live 0.7.0 accepted**, so it ships
  as 0.8.0. No shipped provider is affected — all 43 secret fields across 53 files were parsed and
  none carries an `example`.

  Still open, and deliberately not smuggled in: nothing stops a credential-shaped literal in a `help`,
  `description`, `label` or TOML comment. Push protection matches the string, not the key it sits
  under.

- **A per-provider test can no longer assert over the catalogue (C-230).** Trello's test asserted the
  query-placed credential set equalled a two-element literal — green only because no provider since
  Trello had placed a credential in the query string, and the next one that did would have turned
  *Trello's* test red from a worktree that could not see it.

  The guard refuses the **walk**, not the walk-plus-literal, because detecting "compared against a
  literal" textually fails open — and because the author of a per-provider test cannot review the
  population they quantify over, so even a monotone claim there is correct by luck. `AGENTS.md` now
  names three homes for a genuine catalogue-wide claim, and the guard's failure message points at
  them. All 122 test files were audited; no other instance.

- **`RATIO_FLOOR_PERCENT` could only drift, and is replaced by a unit that cannot (C-196).** It
  guarded against a connector arriving with no response shapes at all, as a share of the catalogue,
  and nothing ratcheted it — it was moved by hand twice, each time *after* somebody noticed.

  Both obvious fixes were rejected on evidence. Deriving it from `COVERED_FLOOR` puts the same
  denominator on both sides and collapses to the assertion above it, deleting the guard along with
  the constant. Keeping a percentage fails for a sharper reason: **one point of 110 operations is one
  operation; one point of 299 is three.** At a floor of 88, five operations could arrive carrying
  nothing before it fired — and 27 of the 53 shipped connectors are five operations or fewer, so over
  half the catalogue could have landed with nothing and passed. The unit was the defect, not the
  number.

  `ABSENCE_CEILING` and `ABSENCE_SLACK` count absences directly, ratcheted both ways, with the slack
  read off the catalogue rather than copied: datadog and google each landed with exactly two honest
  absences, so a slack of one would have turned both red for doing nothing wrong.

- **An SSRF guard that was asserting a third-party constant (C-202).**
  `the_default_egress_guards_the_private_network` read `flux_web::WebOptions::default()` rather than
  this host's policy, so a host shipping `PrivateNetAllow::Any` passed it. Found by mutation, and now
  caught by a behavioural test instead.

## [0.7.0] — 2026-07-31

### Added

- **A GraphQL vendor cannot be a connector yet, and the reason is recorded rather than rediscovered
  (C-110).** The story allowed either outcome — ship it, or record why not. Linear was built, found
  unusable, and withdrawn; `docs/designs/graphql-vendors.md` is the record.

  `connector-pack` derives an operation's configuration variables by scanning every string literal
  for `{…}`, and a GraphQL selection set is braces. Unconfigured, all eight operations refused. **With
  configuration supplied, the whole selection set was replaced by a configuration value** — a
  connector that looks callable and silently ships a mutilated document, which is worse than one
  that refuses. The fix is C-87 (publish the configuration surface so the pack reads an operation's
  variables instead of inferring them from syntax), which is a three-crate change and not a provider
  story.

  Four boundaries are recorded as already solved so a future attempt need not re-derive them:
  path-per-operation is a non-event, C-55 covers a pinned query document, multi-line documents
  round-trip verbatim, and the `data.<field>` envelope is declarable. Two tests now sit inside
  `connector-pack` pinning the collision, one executing the real build against an **empty**
  configuration — the case whose absence let a fully green gate coexist with eight dead operations.

- **The Confluence and New Relic connectors (C-219, C-220).**

  **Confluence** was shipped to answer a question, and the answer is its real deliverable: two
  connectors sharing one vendor authority do **not** share a credential. `jira` and `confluence` use
  the same host, account and token, and resolve to `com.atlassian.jira/api_token` and
  `com.atlassian.confluence/api_token` — same tenant, same leaf, the authority segment the entire
  difference. So the operator pastes the token twice. The addressing model is not broken; C-90's
  "an address is a place, not a per-connector copy" holds *within* a connector and was never wired to
  reach *across* two. The rejected alternative — one `atlassian` connector with both as services —
  is **constructed in a test** rather than asserted, showing it would have delivered single-paste
  sharing, and refused because `com.atlassian.jira` is published and a published address is never
  repointed. Filed as C-226.

  It also cannot read any content body: `body-format` is a query parameter and the connector declares
  none, so it is curated as a connector that navigates and writes rather than one that reads back,
  with every read saying so in the description a model receives. That drove two exclusions, both
  named in the header. Filed as C-227.

  **New Relic** ships with a gap written down and machine-checked rather than smoothed over: the IR
  cannot express a closed set of values, so a two-region vendor's host field accepts
  `api.not-new-relic.example`. A wrong region returns 401 on every call, indistinguishable from a bad
  key. Filed as C-225.

- **The Discord connector (C-216).** Six curated operations behind a bot token whose `Bot ` scheme
  word is the first prefix in the fleet whose *neighbouring* value is also valid vendor syntax for a
  different credential: `Bot <token>` sent as `Bearer ` is a well-formed Discord request for an
  OAuth2 user principal the caller does not hold, answered with a 401 indistinguishable from a
  revoked token. The story's stated premise — that every shipped connector using the prefix axis
  spells `Bearer ` — was measured and found false in both directions (okta ships `SSWS `, pagerduty
  `Token token=`, statuspage `OAuth `; and no connector spells `Bearer ` as a header prefix at all,
  because it is a preset variant). The correction is asserted by test rather than filed away.

  No `quirks.rate_limit` is declared, deliberately: Discord's limits are per-route with a bucket per
  major path parameter and are discovered from `X-RateLimit-*` headers, which the fixed
  `requests`/`per_seconds` pair cannot express. Recorded as C-224.

- **Google sign-in, accounts and sessions (C-204).** The host now answers "who is asking" before it
  injects anything, which `docs/designs/connectors-proxy.md` names as the precondition for a
  credential-injecting service. Every `/v1` route refuses without a session, resolving the tenant
  from the session rather than from anything the caller supplies.

  **A login-CSRF hole was found in review and fixed before this shipped.** `/auth/signin` returned a
  redirect carrying no cookie, and the callback redeemed the OAuth `state` from a process-global map
  keyed on `state` alone — so nothing bound a sign-in to the browser that began it, and an attacker
  could land a victim's session on the attacker's account, after which every credential the victim
  pasted went to the attacker's tenant. `/auth/signin` now sets an `HttpOnly`, `Secure`,
  `SameSite=Lax` `connectors_login` cookie scoped to `Path=/auth/callback`, and the callback compares
  it against the `state` parameter in constant time **before** redeeming it.

  Independently re-reviewed by reproducing the attack end to end at the base and confirming it dead
  on the merged fix, together with absent-cookie, mismatched-cookie, uppercase-name, duplicate-param,
  replay, wrong-method and path-variant probes — all failing closed.

- **Four more connectors: Mailchimp, Klaviyo, Supabase and Resend (C-215, C-218, C-221, C-222).**

  **Mailchimp** asks for its datacentre as ordinary configuration rather than deriving it from the
  key's suffix, and ships **bearer** because a Basic mechanism with a *constant* username is not
  expressible in this IR — `user_env` names environment variables, not literals, and `user_suffix`
  appends to a resolved value rather than replacing it. The vendor documents both forms, so this
  routes around the gap rather than closing it; a vendor with a constant Basic username and no
  bearer alternative is still unshippable, and the loader refusal is pinned by test.

  **Klaviyo** carries a vendor-enforced API revision as a constant header, with the pin and the
  response schemas asserted to name one date.

  **Supabase** declares exactly one credential, named `anon_key` rather than `api_key` — both of its
  keys are "the API key", and a slot called `api_key` presents a box rather than a choice while the
  key that bypasses row-level security always works. `service_role` appears nowhere in the IR and is
  explained in the `help` text instead. Every shipped operation is a read, which is the entire
  justification for one key; a test fails the moment a write lands, reopening the question in review
  rather than in production.

  **Resend** declares a `User-Agent` as a constant header because the vendor rejects requests without
  one — verified missing rather than assumed, and filed as C-223 since the host supplies none.

- **The Bitbucket connector (C-217).** Seven curated operations — repository and pull-request
  reads, plus create, comment and approve — behind an operator-pinned `workspace`, and the first
  connector whose [C-187] pin is the **final** path segment rather than an inner one. That edge is
  the whole reason its `verify` can be argument-free: `GET /repositories/{workspace}` needs nothing
  from the caller once the workspace is pinned.

  The curation runs in an uncomfortable direction and the provider header says so rather than
  leaving it to be discovered: the connector can **add** an approval and cannot withdraw one.
  Withdraw-approval and decline are held back together, because a set that can approve, un-approve
  and decline is a governance surface deserving its own story. The PR merge endpoint is excluded
  too — its `pullrequest_merge_parameters` discriminator is unverified against a live response, and
  a merge is too consequential to ship on an inference.

  Independently reviewed by falsification: nine single-fact mutations of the provider file each
  turned exactly one contract test red.

- **An operator can pin a tenant scope at install time, not only a base URL (C-187).** `[[config]]`
  bound `base_url` and nothing else, so Cloudflare's `zone_id` (a path segment) and Vercel's `teamId`
  (a query parameter) stayed per-call arguments a model chose on every request. For Vercel that was
  sharp: `teamId` is optional at the vendor and its absence is not neutral — omit it and the call
  lands on the personal account instead of the team, so the connector shipped a parameter whose
  omission silently redirects a write.

  `ConfigField::binds` now reaches a **path segment, a query parameter and a header**, through one
  closed `Position` enum. A pinned value is refused as a caller argument in two independent places —
  the loader and the emitter — and pins are **mandatory**: `connector-pack` refuses a request with an
  unresolved literal, so an optional pin is a connector that composes no URL. Verified at request
  time, where both an absent value and an empty string refuse without sending.

  Consequence, stated rather than discovered: Cloudflare and Vercel are now single-tenant per
  connection. One installed connector reaching several zones needs one connection per zone.

- **A dry-run transport that cannot send, and the first check that the pack and the shipped modules
  agree (C-145).** `connector-pack` gains a `Transport` seam whose live arm delegates to flux's
  `http.request` exactly as C-115 landed it, and a `DryRunTransport` that answers "what request would
  this operation make?" without making it.

  It is **structurally** unable to send, not a live client with a flag: a unit struct holding no
  client, no handle and nothing that could reach a socket, with `Egress`'s non-zero size as the
  control. And a rehearsal contains no credential *value* at all — not a redacted one. The dry run
  sits upstream of resolution, with no `ToolContext`, so it never calls `resolve`; it places each
  credential's declared *reference* through the real `auth::place`, so header names, prefixes and
  query separators come from the shipping code rather than a second copy of it.

  The differential test compares, for every one of the 254 shipped operations, the request the pack
  evaluates against the one the shipped `.flux` module declares. **These two artifacts had never been
  compared.** They agree everywhere — so `catalog::Operation::flux`'s claim that they are the same
  bytes is now checked rather than trusted, and a divergence fails by operation id.

- **The `connectors-api` host — the caller this repository never had (C-202, C-203).** Everything
  below it already worked and was tested: `connector-pack` projects a catalogue operation onto a flux
  `ToolSpec`, evaluates `{ method, url, headers, body }` from the operation's own emitted Flux,
  resolves the credential, registers it with the redactor, verifies the registration took, places it
  per its declared scheme, and delegates the send. What was missing was something to bind the ports
  and run the loop.

  `codewandler-flux-web` 0.41.1 is pinned, giving `Egress` its first concrete implementation — and it
  resolves inside the workspace's existing 0.41 line without dragging a 0.42 flux core into the lock.
  The host constructs no request and ships no transport of its own; `WebOptions::default()` carries
  `PrivateNetAllow::None`, so private, loopback and link-local hosts are refused.

  `dependency_fence.rs` gains a `NETWORK_CRATES` allow-list and a third bucket, so the four compiler
  crates are fenced *against the host* and a workspace member that is neither compiler, host library
  nor declared network crate fails rather than passing in silence. The exception is visible rather
  than merely unexamined.

- **The Trello connector (C-165)** — 6 curated operations, and the catalogue's **first
  `Placement::Query` credential**. `trello_is_the_only_query_placement_in_the_shipped_catalogue`
  bounds that to one connector and fails the moment a second lands.


- **A graph's node ids now map to Flux AST paths, so a diagnostic lands on the node that produced it
  (C-96).** `emit_graph_with_paths` returns the emitted module alongside a `NodePaths` map, and the
  round trip is asserted in both directions against real `flux_lang::analyze` diagnostics: every
  diagnostic path resolves to a recorded node, and that node's own path is the one the diagnostic
  sits at or inside. No second attribution mechanism was invented — the path grammar is flux's own,
  consumed from `analyze_flow` rather than re-spelled locally.

  **Boundary nodes are deliberately absent from the map, and the test asserts each absence.** A
  `trigger`, `schedule` or `endpoint` becomes a *parameter* of the emitted op rather than a
  statement, and flux renders no path for a parameter finding. Spelling one anyway (`params[0]`)
  would be exactly the second mechanism this story exists not to build.

  **What did not land, and why it could not:** the map is not yet a committed, drift-checked
  repository artifact. Nothing in `providers/` declares `[[graphs]]` and `connector-cli` never calls
  `emit_graph`, so `build` writes no graph module for a map to sit beside. Both fixture graphs' maps
  are committed as goldens instead, drift-checked on every run; the `connectors/*.paths.json` writer
  belongs with whichever story first gives the lowering a producer.

### Changed

- **The Algolia probe closes with the space measured rather than surveyed (C-164, still blocked).**
  C-187 lifted half of its recorded blocker — a non-secret configuration field can now bind a
  request header. The other half stands and is now the whole block: Algolia's application id must
  reach both a hostname and a header, and the declaration that would express that is refused by name
  — *"Two questions that share an answer are one question"*. Two tests now pin the refusals so the
  boundary cannot be re-litigated, and the remaining gap is filed as C-229.

- **The charter permits a deployed multi-tenant host, and the confused-deputy objection is re-argued
  rather than skipped (C-201).** `docs/vision.md`'s non-goal narrowed this repository's host to one
  that was *"loopback-bound, never published, and never a production request path"* — the
  **yes-narrowed** resolution of C-34 — and `crates/connectors-api` shipped in contradiction with it.
  The owner directed the wider shape on 2026-07-31: a deployed service an operator signs into,
  connects providers to, and calls operations from.

  The narrowing is **superseded, not deleted**. `designs/connectors-app.md` keeps the reasoning it
  rested on — the `Egress` analysis, the slice-1 sequence, why a host that builds its own requests is
  the failure mode — and the new `designs/connectors-api.md` records what replaces it. C-34's "no" to
  the credential-injecting proxy still stands: what was rejected is a service that adds authority to
  whoever asks, and no amount of deployment makes that acceptable. The deputy answer turns on the
  interface, not on "it is authenticated" — a caller names an operation id and cannot name a host, a
  credential, or a tenant, so the service returns authority its caller deposited rather than adding
  authority its caller lacks.

  Four things the amendment explicitly does not license: a second request path, publication, a
  reachable bind before an authenticated principal exists, and holding credentials the operator did
  not choose to give it. The old "no `--bind` flag, ever" prohibition becomes a four-item gate whose
  first item is that the tenant must come from a verified session — a PR adding `--bind` while the
  tenant is a constant is still the rejected proxy.

  The design leads with a measured table of what the charter permits against what the code does,
  because the host is still loopback-only and single-valued today and the gap is the thing most
  likely to be misread in the optimistic direction.

### Fixed

- **A configuration value is checked where it is substituted, not only where it is declared
  (C-214).** `Position::validate_value` existed and was correct, and had exactly two call sites —
  both in the loader, both running against an `example` or a parameter *name*. The value that
  actually travels was substituted with no predicate at all.

  The severe half was **pre-existing** and moved the origin: an `@` makes everything before it
  userinfo, so `subdomain = "acme.zendesk.com@evil.example"` resolved to the authority
  `evil.example.zendesk.com`. Nine shipped connectors carry a templated host. The check compares
  against the **template's** authority rather than the built URL's, because the composed string is
  itself a well-formed hostname and inspecting the finished URL sees nothing wrong.

  Each position gets its own answer, since "escape it" is wrong for two of them: refuse for host,
  path and header; refuse-then-encode through the existing `auth::query_encode` for query. No shipped
  connector's behaviour changed — 53 providers, `diff` clean. Independently reviewed with 36 probes
  against the host guard, including percent-encoded `%40`/`%2540`, CRLF, IPv6 literals, and fullwidth
  and ideographic dot homographs; none moved the origin.

- **A per-provider test no longer asserts a catalogue-wide literal (C-216).** Discord's prefix census
  walked every `providers/*.toml` and compared the result against a four-element list, which Klaviyo's
  fifth prefix falsified in the same wave — each branch green alone, red together, invisible from
  either worktree. It now loads Okta, PagerDuty and Statuspage **by name** and checks what each
  declares, because catalogue membership was never the evidence. The identical defect in
  `trello_connector.rs` is filed as C-230.


- **`no-credential` told a consumer that a public endpoint was disabled for their protection
  (C-206).** `effective_auth` answered `no-credential` for two opposite situations — a vendor that
  requires no credential, and a credential this repository will not yet hold safely. `status.rs`'s
  own comment described the conflation ("would report freshdesk and a genuine ping endpoint the same
  way for opposite reasons") and the code then performed it anyway.

  A provider author can now declare `auth = []` on an operation as a positive statement that the
  vendor needs nothing, distinct from an inherited absence. It publishes as a new `notes` entry,
  `no-credential-required`, rather than a fifth issue code: `works == issues.length === 0` is a
  documented and tested contract, so a new *issue* could not coexist with the `works: true` a
  genuinely-public operation must report. `NO_CREDENTIAL` keeps its exact previous meaning.

  The guard that should have caught the related drift is now derived rather than enumerated.
  `optional_fields_are_null_rather_than_absent` named three fields by hand; it now renders the
  document twice from the same emitter and requires the same key set at the same position, knowing no
  field by name. "Every key is always present" is a checked claim.

- **A `Request`'s derived `Debug` printed every credential in plaintext, and a query-placed
  credential travelled in a form the redactor was never told about (C-159).** `Request` is `pub` and
  derived `Debug` over `url`, `headers` and `body`, and it carries the plaintext *after*
  `auth::place` — the larger of the two exposures C-152 half-closed when it hardened `Assembled`.
  The hand-written `Debug` redacts every header value with no auth-name allow-list and every query
  value, while keeping what debugging actually needs: method, host, path, header *names*, and
  whether a body is present.

  The second half was found independently by C-165's review, which is why it is worth stating
  plainly: `credentials.rs` registered the **raw** credential with the redactor while `auth.rs`
  placed `query_encode(value)` on the URL. For any credential containing a reserved character those
  are different strings, so the redactor held one form and the wire carried another. `placed_form`
  now registers the form that travels. Trello (C-165) is the catalogue's first query placement and
  is what made the path reachable at all.

  Registration is also idempotent now, keyed on the value and verified against the redactor in hand
  rather than on a remembered `(CredentialRef, value)` pair — so a repeated resolve does not grow
  the registered set.


- **The site's hand-maintained-data guard read prose as data, and the web gate was red on `main`
  (C-205).** `npm run build && npm test` reported 27 of 28 at v0.6.0. The guard collected every
  catalogue service name into a forbidden-substring set and grepped the explorer sources with
  `String.includes`, so Postmark's `server` service matched the English word in a comment about the
  VitePress dev server. Nothing was hand-maintained.

  **It was ten false positives, not one** — `server`, `delivery`, `front`, `account`, `box` and
  `admin` across seven files; `server` was merely the first to fire. Thirteen of the catalogue's
  service names are ordinary English words, so this was a latent gate failure waiting for the next
  comment, with nothing to tell the next author why the build broke.

  Two narrowings, both principles rather than exception lists, because an allowlist naming `server`
  is this bug filed once per connector:

  - *A comment renders nothing, so a comment is not data.* Sources are reduced to what they
    contribute to the built site before matching. The scanner is string-aware rather than a
    `//.*$` sweep — a hard-coded `https://api.postmark.com` is exactly what the guard exists to
    catch, and a line sweep would cut it at its own `//`.
  - *A value is a word, not a fragment.* Matching requires word boundaries, which is what lets the
    first narrowing land at all: `catalog.mts` declares the field `delivery_id`, and that is
    structure rather than data. It also settles the `gmail`/`mail` and `drives`/`drive` misreads. A
    capital ends a word, so a hand-coded `zendeskTicket` is still caught.

  **The guard was proved still to bite, not assumed to.** A new test plants a real catalogue value in
  each language the sources are written in and requires each back; and on the integration branch,
  appending `export const FIRST_CONNECTOR = 'zendesk'` to `web/data/catalog.data.mts` turned it red.
  The gate is 30/30 — the 28 that existed plus this story's two.

## [0.6.0] — 2026-07-31

### Fixed

- **Two services of one connector no longer collapse into one configuration value (C-197).** The
  runtime port keyed on `(tenant, provider, kind, name)` with no service, while the IR distinguishes
  them: `providers/contentful.toml` declares `delivery_space_id` and `management_space_id`, each
  binding `endpoint.space_id` under a different service. So contentful had exactly one `space_id`
  slot.

  **Reproduced before it was fixed**, not argued: at the merge base, with the one slot bound to
  `space-for-delivery`, `contentful-entry-create` built
  `https://api.contentful.com/spaces/space-for-delivery/environments/master/entries` — a management
  write into the delivery space. Not a refusal; a `200` from a space nobody named.

  The port now keys on `(tenant, provider, service, kind, name)`, and `Error::MissingConfig` names
  the service — without it, an operator told `contentful` is missing `endpoint.space_id` has two
  fields answering to that description.

  **`catalog::Operation` gained `service`, and it is additive rather than breaking.** The struct is
  already `#[non_exhaustive]`, so an external consumer can neither build it with a struct literal nor
  destructure exhaustively — no downstream code can break on a field arriving. It moved 44 generated
  tables and 248 rows; `catalog.json` and the index came back **byte-identical**, because
  `catalog.json` had carried the service all along. That is the story's own diagnosis confirmed: the
  embedded Rust catalogue was the surface lagging, not the model.

  `connector-pack` *is* genuinely breaking — `ConfigStore::get` gained a parameter and
  `Error::MissingConfig` a field — which is precisely why this landed before the first publish.

  **One hazard has no type-level protection and is recorded rather than solved:** a host that
  implements `ConfigStore` itself and updates mechanically by ignoring the new `service` parameter
  compiles, passes its own tests, and silently restores the defect. The trait's doc says so; that is
  all there is.

### Added

- **Every shipped provider declares an authority (C-92) — and an entire authentication mechanism
  became reachable as a result.** The story said "15 of 16 declare none"; the measured figure was
  **37 of 44**, stale by two fleet waves. All 44 declare one now.

  Without an authority, `Credentials::reference` refuses with `NoCredentialAddress` — the credential
  path does not render, so the connector cannot authenticate at all. The sharper consequence, found
  by C-198's implementor: **all three `BasicJoin` connectors** — zendesk, jira, twilio — lacked one,
  so the refusal fired *before* the configuration port was consulted and **the entire Basic branch
  of `auth::acquire` had no shipped consumer.** A whole authentication mechanism that had never run
  against a real connector. It runs now: two tests moved out of `src/` into `tests/`, the `Box::leak`ed
  doctored provider is gone, and they drive the shipped `zendesk-ticket-show` through the public
  `Operation::build_authenticated_request` with nothing faked — asserting the composed
  `Authorization: Basic …` against a base64 literal computed outside the crate rather than by the
  crate's own encoder. The inverted test that pinned the old wall was **removed**, not relaxed.

  **An authority is permanent** (`AGENTS.md`: an address, once published, is not reused) and it is
  the second segment of every credential path, so each of the 37 is recorded with its reasoning in
  the provider file. The rule: multi-product vendors spell the *product* (`com.atlassian.jira`,
  `com.atlassian.statuspage` — separately provisioned credentials that must not share a directory),
  single-product vendors spell `api`. Three are flagged in-file as genuinely uncertain and are the
  ones to re-check before the first publish: `com.sendgrid.api` (vs `com.twilio.sendgrid` — SendGrid
  keeps its own domain and its own key), `com.frontapp.api`, and `com.notion.api` (vs `so.notion`).

  `api_version` landed on 12 of 44 — only where the connector's own file already spells it. The
  other 25 were left rather than asserting vendor facts from memory, since a version is published
  under the same never-reused contract.

### Fixed

- **A mutable `ConfigStore` could show the egress gate one host and send the request to another
  (C-198).** The pack calls `http.request`'s `execute` directly, bypassing `Executor::dispatch`, so
  `permission_subjects` is the **only** place flux's allow-list is consulted for the inner call — and
  it performed an independent `get` from the one that built the request. Two reads, one call, through
  the single gate. The failing-first test showed it concretely: `host-0` was gated, `host-1` went out.

  **Enforced rather than documented.** `Operation` now holds a `config::Snapshot` taken once at
  `project`, *instead of* the `Configuration` — so it has no handle to the store and there is nowhere
  left to read twice. The test asserts the store is read exactly once, which is the stronger claim
  than comparing two values. A documented invariant a caller can break silently is weaker than one
  the type prevents.

  The behavioural consequence is named rather than hidden: a host that mutates its store *after*
  building the pack now sees the snapshot, not the new value, and must rebuild the pack. That was
  already the module's stated advice; it is now the semantics.

### Changed

- **The four published crates are `codewandler-connector-{catalog,spec,secrets,pack}`.** The bare
  `connector-*` namespace is contested — `connector-cli` is already taken on crates.io by an
  unrelated project — so the published names take the prefix the flux family uses. `[lib] name` is
  pinned on each, so **no source file changed**: `use catalog::`, `use connector_spec::`,
  `use connector_secrets::` and `use connector_pack::` are all unaffected.

  The rename surfaced **five latent defects**, every one invisible while nothing in the tree was
  aliased, and two of which would have fired for the first time during a real publish — the one
  operation with no undo. The publish script matched dependency edges on the local alias rather than
  the package, dropping `codewandler-connector-spec` from the closure entirely and emitting an order
  that publishes a crate before its own dependency; the independent Rust recomputation of that order
  reached the same wrong answer by a different route (it read each member's manifest, where the alias
  is not); a `package =` key rebinds the extern away from `[lib] name`; a renamed package renames its
  crate; and `dependency_fence.rs` fenced a name that no longer existed — failing loudly with *"this
  fence has nothing to fence"* rather than passing vacuously and quietly retiring the offline
  guarantee.

  The two-implementation disagreement test in `publish_closure.rs` is what caught the first two, and
  it only worked because both were wrong in *different* ways.

### Added

- **The PagerDuty connector (C-162) — the third and last vendor C-184's prefix axis unblocked.** Six
  operations over `Authorization: Token token=<key>`, `pagerduty-service-list` as `verify`.

  The story was filed claiming the credential is *"a substructure of the header value, not a
  suffix"*, which would have needed an axis richer than a prefix. **That premise was wrong**, and
  C-161 had already measured why: the value is a fixed literal followed directly by the raw key, so
  `Token token=` is a prefix that happens to contain `=`. The `=` is its separator, which is why it
  satisfies the guard structurally rather than by luck.

  **The `From` header is a required parameter, because operator configuration is unspellable.**
  PagerDuty requires an actor email on writes. `parse_binding` admits exactly `endpoint.*`,
  `credential.*`, `username.*`, `oauth.client_id` and `oauth.client_secret` — there is **no
  `header.*` destination** — so a configured `From` cannot be written at all. It ships as a required
  `params.header` on the two writes only, on Stripe's `Idempotency-Key` precedent.

  **Acknowledge and resolve are separate operations** though the vendor exposes one endpoint, so
  acknowledging is not graded at the same risk as resolving (`medium` vs `high`).

  **No pagination quirk is declared, and the absence is pinned.** PagerDuty pages by `limit`/`offset`
  and `Pagination` has only `Page` and `Cursor` — `Page` describes a page number incremented by one,
  where `offset` is a row count advanced by `limit`. Declaring it would record something false now,
  and become a **bounded** wrong loop when C-12 compiles quirks into control flow. Bounded is the
  harder failure to notice, not the easier.

  One shared test guard was strengthened rather than worked around: `services.rs` asserted that a
  single-service provider's canonical JSON holds no `"service"` **substring**. PagerDuty is the first
  vendor whose own domain noun is "service" — `GET /services` answers `{"services": [...]}` — so the
  scan was reporting a word collision. It is now a structural walk that finds an IR service key at
  any depth outside a JSON Schema subtree, with each exemption independently pinned by mutation.

### Changed

- **The response-schema ratchet turned: `COVERED_FLOOR` 193 → 220.** Statuspage, Okta and PagerDuty
  each fitted inside the slack alone — which is why each correctly reported eight red tests and left
  the file untouched — and their accumulation crossed it. This is the per-wave-not-per-story case,
  and why the constant is coordinator-owned.

- **`RATIO_FLOOR_PERCENT` 82 → 87, correcting a guard that had stopped guarding.** Its doc specifies
  *"one point under the measurement… there is no room in one point for a whole provider."* It was six
  points under — roughly sixteen operations at this catalogue size, comfortably a whole provider
  arriving with no response shapes and passing. Nothing failed because nothing could: the absolute
  floor has a two-way ratchet and this one has none, so it can only drift. Filed as **C-196**, with a
  recorded preference for deriving it from `COVERED_FLOOR` and deleting the constant rather than
  adding a second ratchet — two numbers describing one measurement drift apart eventually.

### Added

- **A tenant's configuration is substituted into a templated base URL (C-193).** Nine providers'
  hosts carried a `{subdomain}`, `{shop}`, `{domain}`, `{site}`, `{instance}`, `{account_host}`,
  `{space_id}` or `{page_id}` to the wire verbatim. A bound `ConfigStore` port — handed in at
  construction the way `Credentials` already is, never a global and never an environment read —
  now fills them, and substitution is **total or refused**: `Error::MissingConfig` fires before the
  body is evaluated and `Error::UnresolvedEndpoint` is a second lock, so no request leaves with a
  brace in it.

  **The half that is easy to miss is the permission subject.** The pack calls `http.request`'s
  `execute` directly, bypassing `Executor::dispatch`, so `permission_subjects` is the *only* place
  flux's egress allow-list is consulted for the inner call. It previously declared the
  un-substituted template — a subject no allow-list can match. It now declares the substituted host
  on **both** paths, the built one and the malformed-call fallback, pinned by a whole-catalogue
  assertion that no shipped operation's subject contains `{`, with a control that fails if the
  catalogue ever stops carrying a templated connector.

  **Substitution runs over the emitter's string literals, never the finished URL**, and that is the
  security-relevant choice rather than a stylistic one: flux interpolates `fmt` and never `lit`, so
  a brace surviving in a literal is by construction a name nothing fills. Substituting the finished
  URL is one line shorter and would let a *caller's parameter value* be filled with a tenant's
  configuration on its way to the vendor. A test pins that a parameter spelling `{subdomain}` is
  left alone.

  **A refusal nobody asked for:** both ports carry a tenant, and nothing stopped a host pairing
  tenant A's credentials with tenant B's settings — outcome, one tenant's token sent to another
  tenant's host. Now `Error::TenantMismatch`, refused at `project`.

  The story's own measurement was low in both directions: **9 providers / 53 operations**, not 6/38.
  Seven have a templated *host*; two (`contentful`, `statuspage`) have a templated *path* on a host
  that resolves, which is the quieter failure — the request reaches a real server and returns a
  `404` that reads as a missing record rather than as an unconfigured connector.

  Also moved: the Basic user-half no longer reads the process environment. It is a tenant value and
  now comes from the same port.

### Fixed

- **A `--service` scoped build carried another service's `config`, `graphs` and `verify` (C-194).**
  `select_service` narrowed `services`, `operations`, `events` and `channels`, then let
  `..connector.clone()` carry the rest through. Seventeen real crossings across four shipped
  providers: 12 configuration fields and 5 `verify` pointers, in `anthropic`, `contentful`,
  `microsoft_graph` and `postmark`. `graphs` leaked zero times, because **no provider declares one**.

  **None of the seventeen reached a committed artifact, and the number needs its plain reading.**
  Eight of the leaked config fields declare `secret = true`, which sounds far worse than it is: a
  `ConfigField` is `name`/`label`/`help`/`format`/`example`/`binds` and **has no value field**.
  `secret = true` is a claim *about a value a host will later collect*, not a credential. The worst a
  crossing could have published is a form field — a label, its help text, and a credential name the
  catalogue already publishes deliberately. Verified four ways that nothing reaches disk, including
  that the leaked help string appears nowhere outside `providers/anthropic.toml`, which is input.
  The real defect is that a `models`-scoped install would ask an operator for an admin key, eroding
  the operator/connection level split.

  It was invisible because **no test had ever looked at `select_service`'s output beyond
  `operations`**, and an emitted-artifact test structurally cannot cover an IR-only surface. Both
  halves are now pinned: a fixture test for what no shipped provider exercises, and a property test
  over the real catalogue that produced the seventeen.

  `auth`/`default_auth` are deliberately **not** narrowed — `AuthMethod` has no `service` field, so
  it needs a reachability computation rather than a filter, and a test pins that so a later edit
  cannot take it by accident. `..connector.clone()` remains the mechanism and will do this again for
  the next service-partitioned field; `HashDomain::of` already solves the class with exhaustive
  destructuring that fails to compile until someone states the answer.

### Changed

- **The docs now say what a connector actually is, and the charter was amended twice.**

  `vision.md` had defined a connector as *"auth + operations + quirks"* — true of an IR with three
  surfaces. `Connector` has **sixteen fields**, and the three the vision named are not the
  interesting ones any more: `quirks` reaches almost nothing and `auth` reaches neither the module
  nor the manifest. `AGENTS.md` said *"a service has three member kinds"* directly above a five-row
  table. `README.md` advertised v0.3.0 and 19 providers. The roadmap still said *"nothing is
  implemented yet"* against 86 done stories.

  New `docs/designs/connector-surfaces.md` is the single answer to *"what can a connector bring to
  flux?"*, and its most useful content is the negative half: **six surfaces reach no artifact at
  all** — `config`, `graphs`, `verify`, `roles`, `quirks.pagination`, `quirks.rate_limit`. They load,
  they validate, they move `ir_sha256`, and nothing downstream can see them. Two of the six are dead
  for a different reason worth separating: nothing *declares* a graph (the lowering in
  `connector-flux/src/graph.rs` is complete, tested and never called), and hubspot records its
  `rate_limit` non-declaration deliberately.

  **Charter amendment 1 — the "no runtime" non-goal is now "no runtime *for production traffic*"**,
  permitting `crates/connectors-app`: a loopback-bound reference host proving the seams end to end,
  never published, never a production request path. This resolves **C-34** as yes-narrowed — yes to a
  host that proves the seams, **no** to the credential-injecting proxy the story was filed about,
  whose confused-deputy objection stands unamended.

  **Charter amendment 2 — technology adapters stay a non-goal**, with a clarification: capability
  *contracts* span both repos, so a hand-written flux plugin (Vault) and a generated connector
  (1Password) can satisfy one and a host need not know which it holds. This moves nothing into this
  repo; Vault stays a flux plugin.

  Also new: `docs/designs/connector-contracts.md`, which measured the thing that blocks contracts.
  The slot vocabulary fails in **both directions at once** — `get` fills 44 of 48 services (too loose
  to discriminate) while `put` matches **zero operations** and no operation id even contains that
  substring (too narrow to express), because all 13 `PUT` operations are named for their domain verb.
  So a `secret_store` contract is not merely unbuilt, it is unspellable, and **C-23 (operation naming)
  is a hard prerequisite** rather than a nice-to-have.

  The counts in these documents remain **hand-typed**: C-81 (*declared counts are checked*) is still
  `ready`, no mechanism derives them, and they had drifted five times before this pass. They drifted
  again *during* it — two connectors merged between the docs being written and merged.

- **The Okta connector (C-161) — the probe that produced the prefix axis, now shipping on it.**
  Five operations over `Authorization: SSWS <token>`, `okta-user-list` as `verify`, and
  `okta-user-deactivate` as the one `destructive` write.

  This story is the round trip. C-161 first ran while `AuthScheme` was a closed five-variant enum,
  **refused to ship a connector that could not authenticate honestly**, and recorded why at
  `path:line`. That refusal produced C-184, which built the `prefix` field; this pass ships the
  connector on it. The probe's findings are kept rather than deleted — they are the measurement the
  axis rests on — and `no_provider_toml_was_shipped_for_this_probe` was **inverted** rather than
  removed, so the file still records that the refusal stood until the seam existed.

  **Two curated exclusions, both made executable rather than left as prose.** Okta's `q`/`filter`/
  `search` are free-text and SCIM-expression filters, which is the C-30 unencodable-query gap at its
  worst case — a SCIM expression is made of quotes, spaces and punctuation, all interpolated
  verbatim. And `after` is a cursor Okta only ever returns in a `Link` **response header**, which
  this model cannot surface, so exposing the parameter would offer a knob nobody can turn. A test
  fails if a later story adds either back. The cost is real and recorded: all three list operations
  are first-page-only.

  **The host is a bound `{domain}`, not a subdomain label**, because Okta orgs also live at
  `.okta-emea.com`, `.oktapreview.com` and custom domains. The trade is recorded: `format =
  "hostname"` validates the `example`, not the operator's input, so a mistyped host is a malformed
  URL rather than a named error.

- **The Statuspage connector (C-181) — the first shipped connector with a non-empty auth prefix.**
  Five operations over `Authorization: OAuth <key>`, with `statuspage-component-list` as `verify`.
  This is C-184's prefix axis proved end to end: `crates/catalog/src/generated/statuspage.rs` is the
  first committed artifact carrying a `prefix` field with a value in it.

  **`OAuth` here is a literal scheme word, not OAuth2**, and the connector declares no `oauth2` block
  — with a test asserting `oauth2.is_none()` so the trap stays pinned. A connector spelled `bearer`
  would compile clean and fail closed with 401 on every call, which is the trap C-107 recorded for
  Notion and C-161 for Okta.

  **The page id folds into `base_url`**, the way DocuSign's `account_id` already does — so this is
  *not* the C-187 gap, and the story says so. The cost is recorded rather than worked around:
  `GET /v1/pages` becomes unreachable under that base URL, and a multi-page account needs one
  installation per page.

  **What the model cannot say, and does not pretend to.** Creating a Statuspage incident emails and
  texts every subscriber immediately. There is no effects field to declare that — `effects
  ["network"]` is hardcoded at `connector-flux/src/op.rs:616`, which is exactly what C-155 measured
  — and `Risk` has no value meaning externally-visible. Both writes are `risk = "high"`, matching
  `github-issue-create` and `launchdarkly-flag-toggle`, and the asymmetry the scale cannot carry
  lives in each operation's description: **the incident is reversible, the subscriber email is not.**
  No `effects` key was invented, and a test greps the provider file to prove none appears.

  `deliver_notifications` is a **required** body field on both writes, so a caller must make an
  explicit choice about notifying every subscriber rather than inheriting a default.

- **A credential can sit inside a header value it does not wholly occupy (C-184).**
  `AuthScheme::Header` now carries `{ name, prefix }`, so `Authorization: SSWS <token>` is expressible
  without any credential value being authored. Unblocks Okta (C-161, back to `ready`), PagerDuty
  (C-162) and Statuspage (C-181); none of the three connectors ships here — this is the seam only.

  **The axis is `prefix` alone — no `suffix`, no template**, and C-161's own measurement is why. It had
  already recorded PagerDuty's `Token token=<key>` as *"a prefix exactly like `SSWS `, just longer"*, so
  the story's framing that PagerDuty needs text *after* the credential was the one premise that did not
  survive: all three vendors put the credential at the **tail**. A template was rejected for being
  expressive in the wrong direction — it can spell a credential substituted zero times, which is an
  unauthenticated request that every artifact describes as authenticated. A prefix makes that
  unspellable rather than merely refused.

  **A prefix is connector data and the loader keeps it that way**: it refuses a resolution marker
  (`${…}`, `$secret`), a prefix naming a declared credential or its env var, and anything outside
  visible ASCII, space and tab — the last being header injection from a committed artifact. It
  deliberately does *not* consult `CREDENTIAL_VALUE_PREFIXES`, which catches a pasted credential in a
  constant header; a scheme word is that same text where it is correct.

  **Redaction is unchanged, and the reason is the finding worth keeping.** Acquisition can *transform* a
  secret (`base64(user:secret)` does not contain it — hence its second registration); placement only
  *surrounds* it, so `SSWS <token>` scrubs to `SSWS <redacted>` off the registration that already
  exists. Registering the prefixed form would repeat C-159 §2's divergence in the other direction —
  holding a public word while leaving the bare token, the form a 401 body echoes back, unheld.

  **A full build wrote exactly one artifact: `web/public/catalog.json`.** Every `.flux` module, manifest
  and the embedded Rust catalogue are byte-identical, because an empty prefix does not
  serialize and the catalogue's `Header` arm already emitted `prefix: ""` when it was hard-coded. The
  IR hash domain is JSON rather than TOML (`ir.rs:1258`), and `skip_serializing_if` applies there too,
  so `ir_sha256` cannot move either — pinned by `ir_roundtrip.rs:203`. (An earlier draft of this entry
  also claimed `connectors.lock` was byte-identical. **That file is not produced** — `lock.rs:48` says
  writing it is `connector-cli`'s job and the CLI never does — so the claim was vacuous. Filed as
  C-189.) The
  catalog.json diff is purely additive — one `prefix` key per credential (31 `"Bearer "`, 13 `""`, 3
  `"Basic "`). That key is published on purpose: without it, Okta's prefixed `Authorization` and
  LaunchDarkly's raw one flatten to the same two keys.

  The runtime needed no change — `Placement::Header { name, prefix }` has composed `Bearer ` as data
  since the pack landed. The gap was only ever in the half an author writes.

- **The Twilio connector (C-109) — reads only, and `PARTIAL` for two separately-recorded reasons.** Five
  operations over a Basic join with the account SID as username, messages and calls, `account-get` as
  `verify`.

  **The send surface is excluded** because form values interpolate verbatim: C-144 added
  `body_encoding = "form"`, but flux's form encoder (upstream `L-101`) is not in the pinned release, so a
  value carrying `&` or `=` would corrupt the body and could inject a field.

  **The webhook binding is excluded** because `HmacSpec::signed` admits only `{body}` and `{timestamp}`,
  while Twilio signs the request URL plus its form fields sorted and reassembled. So Twilio declares its
  events with **no channel binding**, and a test asserts that absence — declaring a binding whose
  verification cannot be performed would be worse than declaring none. Filed as C-188, the second instance
  of this class after C-141's composite-header finding.

  The account SID is both the Basic username and a path segment on every operation, asked for once. And
  `twilio.webhook_signing_secret` deliberately reuses `TWILIO_AUTH_TOKEN`: Twilio issues one secret serving
  two roles, and the reuse is documented as intentional rather than left looking like a copy-paste error.

- **The Microsoft Graph connector (C-108).** Three services — mail, calendar, files — eight curated
  operations over a bearer token, with the zero-argument `GET /v1.0/me/calendar` as `verify`.

  **All three services share one host *and* one API version**, which is stricter than Google's case and
  makes the finding sharper: the service level earns its place as **the installable unit**, not as a
  routing or versioning mechanism. A test asserts it rather than the header comment claiming it.

  **A provider id must be a valid Rust identifier**, so this ships as `microsoft_graph`, not
  `microsoft-graph`: `catalog.rs::module_ident` requires `^[a-z_][a-z0-9_]*$` because a full build declares
  `mod <id>;`. Caught by the implementor's own gate via `core_catalog`'s full-build simulation. Same family
  as C-171's `box`-is-a-keyword finding — the provider id is the one author-chosen string that must survive
  becoming Rust.

  `POST /me/sendMail` is excluded, blocked on C-185: its `toRecipients` is an array of objects, the
  genuinely-blocked decomposed case. The reply operation ships instead. Binary file download is left out as
  an unexplored response shape rather than guessed at.

- **The DocuSign connector (C-174).** Six operations over an OAuth2 bearer token, envelopes and
  recipients, with `GET /folders` as `verify`.

  **Its two-level per-tenant prefix folds entirely into `base_url`, and that is a better answer than the
  story proposed.** `template_variables` (`config.rs:348-362`) extracts every `{...}` placeholder and the
  validator requires a bound `[[config]]` field per variable with no cap on count
  (`provider.rs:557-573,638-660`) — so the account host and the account id are two independently bound
  variables rather than one pinned value plus a per-call argument. This narrows C-187: a multi-level
  per-tenant *prefix* was always expressible; what cannot be pinned is a path segment **outside** the
  `base_url` template.

  An envelope is a legal signature request, so the declarations were the point: `void` is destructive, and
  it is declared `non_idempotent` deliberately rather than claiming RFC-9110 idempotence on a destructive
  action without vendor evidence that a repeat is safe. **DocuSign's *Create Recipient View* is excluded
  outright** — it returns an embedded signing URL that acts as a bearer token, and excluding the operation
  means there is no hazardous field to warn about, which beats shipping it behind a warning.

### Changed

- **Algolia cannot ship, and C-187 is now load-bearing rather than ergonomic (C-164).** Algolia's
  application id must appear in the hostname *and* as a header. All three possible routes were measured
  against the loader and all three fail: `ConfigField::binds` reaches five destinations and no header;
  the one route that does reach a header forces `secret = true` unconditionally, which would be a false
  declaration for a non-secret application id; and `ParamSet::header` has no connection to `[[config]]`,
  so it pins nothing and only invites a mismatch.

  Both of the story's original probes had already been answered and neither was the blocker — two
  credentials on one request work (C-160), a configured host works (C-163). The blocker is narrower and
  was unpredicted: **one non-secret value cannot reach two request positions.** Cloudflare and Vercel
  shipped with a worse surface; Algolia cannot ship at all.


### Added

- **The Contentful connector (C-177) — two hosts, two credentials, one vendor.** Delivery
  (`cdn.contentful.com`) and management (`api.contentful.com`) are different authorities with different
  tokens; five operations across them, and a test asserts every operation resolves to **its own service's
  token only**.

  It reuses Postmark's `Operation::auth` mechanism rather than inventing a second spelling, and adds a new
  stress: **the first provider whose `base_url` carries two template variables per service**
  (`space_id`, `environment_id`), which is what makes `entries-list` argument-free enough to serve as
  `verify`.

  **A new constraint was measured against the loader, not guessed:** `validate_config` enforces
  `ConfigField` name uniqueness across the **whole connector**, not per service — unlike operations,
  events and channels, which are per-service namespaces. So the two services cannot share a
  `space_id`/`environment_id` pair, and the connector ships four fields where two would express the
  operator's intent, with nothing checking that the duplicated space id matches. Folded into C-187, which
  now tracks three instances of the config surface's reach being narrower than its neighbours'.

- **The Datadog connector (C-160) — the first connector to send two credentials on one request.**
  `DD-API-KEY` and `DD-APPLICATION-KEY` together, four read operations, `monitor-list` as `verify`.

  **It was filed expecting a refusal and shipped instead, because the premise was falsifiable and got
  falsified.** `default_auth` is a `Vec<AuthRequirement>` (an **OR** of alternatives) and each
  `AuthRequirement` holds an **AND**-set of credentials (`auth.rs:272-288`). The capability was designed,
  written up as a worked example in `providers/babelforce.toml`, and never exercised by a shipped
  connector because that vendor deprecated the pair. Confirmed in the emitted artifact:
  `credentials: &[&["datadog.api_key", "datadog.application_key"]]` — one alternative, two credentials,
  never flattened. This also settles C-164 (Algolia), filed on the same wrong premise.

  Two operations were dropped rather than guessed: *submit an event* (v1-vs-v2 body shape unverifiable —
  the vendor's docs render client-side) and *query metrics* (needs the percent-encoding this pipeline
  does not have). Incident Management operations carry no `response_schema` for the same reason
  babelforce carries none: the field-level shape is genuinely unverified.

  **A hazard for flux's `$auth` seam is recorded on the story**, since nothing here can enforce it: an
  AND-set and a set of OR-alternatives are structurally identical in the type, and a host that resolves
  the AND-set as "the first satisfiable credential" would send one header of a required pair.

- **The Webflow connector (C-182), and it completes a taxonomy.** Six operations over a bearer token;
  `site-list` doubles as `verify` and as the site-id discovery step, since `base_url` has no per-tenant
  placeholder to bind.

  **Item creation is excluded, and this is now the third distinct answer to "a payload this pipeline
  cannot type."** Notion excluded a *recursive* union. Miro shipped a *bounded* one as a read-side
  `oneOf`. Webflow's `fieldData` is **unbounded and tenant-defined** — there is no enumerable set of
  shapes to write down at all — so neither prior mechanism applies, and `webflow-collection-get` ships as
  the honest runtime-discovery substitute. Independently, Webflow's create endpoint wraps the item body
  in an array, which is the genuinely-blocked nested case of C-185.

  `webflow-site-publish` carries no `response_schema`: its body is not confidently known, and absence
  beats a guessed placeholder by the coverage ratchet's own stated principle.

- **The Front connector (C-179).** Six operations over a bearer token, prefixed resource ids (`cnv_`,
  `tea_`) declared so a model cannot invent one, and a bounded `GET /conversations?limit=1` as `verify` —
  Front's `/me` identifies the OAuth-granting company rather than a plain token's owner, so it was not a
  verified stand-in for "prove this token works."

  **Pagination is unreachable and every listing operation says so**, asserted by
  `every_listing_operation_documents_that_pagination_is_unreachable`. Front pages with an absolute URL in
  `_pagination.next` — not `_links.next` as this story originally claimed; the implementor checked the
  vendor reference and corrected it. An operation that silently only ever returns the first page is the
  plausible-but-wrong output this pipeline refuses, so the limitation is declared instead.

### Changed

- **C-185's scope was too broad, and C-179 narrowed it by reading the emitter.** A flat, single-level
  array **is** already expressible — Front's `tag_ids` emits as `List<String>`. What is blocked is an
  array a `wire` path must **decompose across nested segments**, which is what SendGrid's
  `personalizations[].to[]` needs. Cloudflare's `files[]` is flat and was wrongly listed as blocked.

  The related wall is C-56, not C-185: Front's optional `to`/`cc`/`bcc` are arrays this pipeline can
  build, but an optional body field cannot be omitted without sending an explicit `null`. Conflating the
  two would send C-185's implementor down the wrong path.

- **The Salesforce connector (C-163) — the first provider whose host comes from configuration.**
  `https://{instance}.my.salesforce.com`, bound by a `[[config]]` field and asserted by a load-bearing
  test. Five operations over an OAuth2 bearer token, with `GET /services/oauth2/userinfo` as `verify`.

  **A configured host is verified rather than inferred:** `Binding::Endpoint { variable }`
  (`config.rs:180-184,240-245`) reaches exactly a `{variable}` in `base_url`. C-169 and C-170 had
  measured the negative half of this — no path segment, no query parameter (C-187) — and this is the
  positive half. It also settles C-92 for tenant-templated providers: `authority` is independent of
  `base_url`, so there is no conflict.

  SOQL is excluded. It needs a `q` query parameter and no query value is percent-encoded, which is the
  `zendesk-ticket-search` defect precisely.

### Fixed

- **Three negative sentinels used `"salesforce"` as a provider that could not exist, and one did.**
  `crates/catalog/src/lib.rs`, `crates/connector-pack/src/lib.rs` and
  `crates/connector-pack/tests/projection.rs` each asserted that an unknown provider is refused, using
  that literal — so shipping the Salesforce connector broke all three at once. `AGENTS.md` had named
  Salesforce as belonging here from the start, so this was scheduled rather than unlucky.

  The sentinel is now a name that is not a company, and each use is **self-checking**: the assertion is
  that the catalogue does not carry it, so it cannot decay into a vacuous pass the way another
  freshly-plausible vendor name would merely defer.

- **The Postmark connector (C-180) — the first provider whose two credentials are partitioned by
  service.** `X-Postmark-Server-Token` for the `server` service, `X-Postmark-Account-Token` for
  `account`; they are never sent together, which is what a service is for. Six operations. Two *named*
  services rather than one named beside an elided default, because the service contract refuses an
  implicit `default` the moment any named service exists.

  **Its `GET /servers` response carries live server tokens in plaintext, and the connector now says so
  where a reader will see it.** The first attempt noted the hazard in a TOML comment and omitted the
  field from `response_schema` — which looks like caution and is the reverse, because `site.rs:680`
  clones the schema into `web/public/catalog.json` and a comment reaches no artifact. Following Zoom's
  `start_url` and Zendesk's `authenticity_token`, `ApiTokens` is now declared with a description that
  names it as account-privileged and not to be logged, echoed, or passed to another tool — and both
  operations' own descriptions repeat it, since that is what a model reads before calling.

- **The Anthropic connector (C-122).** Two services — `models` (catalogue, claiming the
  `llm_catalogue` role) and `admin` (organization, workspaces, API keys) — five read operations, every
  endpoint verified against Anthropic's published reference.

  **It ships the management surface and the model catalogue, and deliberately not inference.**
  `vision.md`'s non-goals exclude replacing flux's native model providers, so `messages.create` is out of
  scope however natural it looks. `anthropic-version` is pinned through `const_headers` in the inline-table
  form (the section-header hazard `providers/github.toml` records) and a test asserts it reaches every
  emitted operation as a literal while appearing in no signature.

  Two credentials, because the Admin API genuinely requires a distinct admin key: folding them into one
  field would ask every operator for organization-admin access merely to list models.

  **Its `api-keys-list` gets the in-band credential question right unprompted** — the response carries
  `partial_key_hint`, and both the operation description and the schema state it is a redacted display
  value and never the key, which is `providers/zoom.toml`'s convention followed without being asked.

### Changed

- **A per-service credential partition is enforced, and now it is written down.** Investigating C-180
  established that `Connector::credential_ref_for` (`ir.rs:1166-1178`) always renders `DEFAULT_SERVICE`
  regardless of a credential's declared service — but `Operation::auth` (`ir.rs:652-669`) overrides
  `default_auth`, so the emitted catalogue carries the correct token per operation. Two providers now
  depend on this; before this run, nothing recorded which of the two mechanisms was load-bearing.

- **The Typeform connector (C-173).** Five operations over a bearer token, cursor-paginated responses,
  and `GET /me` as `verify`.

  It reached the same technique Calendly did, independently: **a JSON Schema `pattern` used as a guard**
  against the unencoded-query gap. `since`/`until` are constrained to Typeform's no-offset UTC shape
  rather than a loose `date-time`, specifically so a `+` timezone offset cannot reach the query string —
  the hazard Calendly excluded two parameters over.

  Its provenance caveat is unusually explicit and worth keeping: the 32-character lowercase-hex cursor
  charset rests on one vendor example and one community statement, not a versioned spec. The consequence
  is named too — a value outside the pattern is rejected client-side by its own schema, so the failure is
  loud rather than a corrupted request. Single-service deliberately: the manifest round-trip tests read
  the default-service manifest, so a multi-service provider with an inbound surface would panic in two
  of them, and this story was not the place to discover that.

- **The Miro connector (C-183), and it narrows a gap Notion recorded.** Six operations: board discovery
  as `verify`, generic item list and get, and sticky-note create/update/delete.

  The story was filed as an experiment — C-107 refused Notion's block model, and the question was
  *which* of its two reasons was load-bearing. **Neither applies here, and both were tested rather than
  assumed.** Recursion does not, because Miro's item union is flat. Untyped-blob-on-write does not,
  because **Miro resolves its type discriminator through the URL** (`/sticky_notes`), not through a body
  field — so the write side never needs to carry a discriminator at all, and a test asserts it doesn't.

  The union therefore survives on the **read** side as a JSON Schema `oneOf` in `response_schema`, and
  the mechanism is worth naming: `response_schema` is raw JSON, unconstrained by `params.body`'s flat
  `BodyNode` model. That is the same asymmetry C-185 describes from the other direction — this pipeline
  can *describe* a shape it cannot *construct*.

  Update and delete are scoped to sticky notes on their type-specific paths rather than the generic
  `/items` endpoints, because the generic ones could not be confirmed and a wrong guess there is exactly
  what the story said to avoid.

- **The Vercel connector (C-170).** Five operations over a bearer token, all five endpoint shapes
  verified against Vercel's published reference rather than recalled.

  The archetype is **an optional parameter with a blast radius**: omit `teamId` and the call lands on
  the personal account instead of the team. Every operation declares it and every list operation's own
  `description` names the fallback, asserted by
  `every_operation_declares_team_id_and_names_the_personal_account_fallback`.

  **`vercel-projects-list` deliberately ships with no `response_schema`** — Vercel documents three
  mutually incompatible top-level shapes for that one endpoint, so declaring one would be a guess
  dressed as a type. Absence is honest; the coverage ratchet permits it precisely because it measures
  the aggregate and leaves the per-operation judgement to the provider file.

- **The Cloudflare connector (C-169).** Five operations over a bearer token: DNS records (list, create,
  delete), a cache purge, and `zone-list` as `verify`.

  **The zone-id question was settled by the schema, not by preference.** A `[[config]]` binding can
  reach `base_url` and never an operation path, so `zone_id` is a required per-call argument on
  everything except `zone-list`, and a test asserts no config field binds a zone. The consequence is
  deliberate: one installed connector can address every zone its token can, rather than being pinned to
  one.

  The two hazard declarations are the point of this connector: a DNS record delete is `destructive`, and
  a cache purge is `high` risk. The purge is **genuinely idempotent and declared `non_idempotent`**
  because the emitter refuses `idempotent` on POST by method — documented rather than absorbed, and now
  filed as C-186 with a second instance from C-175.

- **The SendGrid connector (C-168) — four operations, and deliberately not the send.** Templates,
  bounce suppressions and address validation ship over a bearer key.

  **`sendgrid-mail-send` is excluded, and the reason is a gap wider than SendGrid:** `BodyNode` builds
  nested objects from a dotted `wire` path and **never arrays**, while SendGrid's envelope is
  `personalizations[].to[]` — arrays of objects containing arrays of objects, which SendGrid will not
  accept in bare-object form. Filed as C-185, which also names the four other fleet connectors that
  will hit it independently.

  So this catalogue has an email provider that cannot send email. That is the honest state and it is
  named in the connector's own header, rather than an operation that emits a body SendGrid answers
  `400` to. The mechanically-legal workaround — one array-typed body-root parameter — was rejected on
  Notion's precedent: it decomposes nothing and dresses a guess as a typed field.

- **The Dropbox connector (C-167).** Six operations, and **every one is a `POST` including the reads** —
  Dropbox's v2 API is RPC wearing HTTP. A test asserts exactly that, because it is the property a
  future author is most likely to "fix" by turning a read into a `GET` that returns `405`.

  **A POST-shaped read can be a `verify` operation, and that was measured rather than argued.** The
  configuration contract's prose says a `verify` op *is a read*; the loader's actual check
  (`provider.rs:664-687`) tests the declared `risk`, not the method. So `POST
  /2/users/get_current_account` is a legal `verify`, and shipping one is more honest than refusing on
  the strength of a sentence.

  Content upload and download are excluded: they use a different host and a `Dropbox-API-Arg` header
  carrying JSON, which is a second encoding problem this connector was not chosen for.

- **The LaunchDarkly connector (C-175).** Five operations over a raw, unprefixed `Authorization`
  header, and a test asserts **no emitted operation carries a `Bearer` or `Basic` word** — the failure
  mode here is silent, since either prefix would produce a request LaunchDarkly rejects.

  The flag toggle is declared `high` risk and **`non_idempotent`, which was not a choice**: the emitter
  refuses `idempotency = "idempotent"` on any `PATCH` under RFC 9110 §9.2.2, as a repository-wide rule
  rather than a judgement about this endpoint. Recorded at `path:line` in the connector rather than
  worked around. Its `description` names the live production effect, because toggling a flag changes
  behaviour for real users the moment it returns.

  The toggle's body schema admits **only** a single JSON-Patch replace onto one environment's `on` bit —
  a general patch body would let a model rewrite a flag's targeting rules through an operation whose
  description promises a toggle.

- **The ClickUp connector (C-178).** Six operations over a bare `Authorization` header — no scheme
  word, which needs no new capability: it is `AuthScheme::Header { name: "Authorization" }`, the
  variant C-161 proved loads and round-trips cleanly. ClickUp's raw token and Okta's prefixed one look
  alike and are not: only the second needs C-184.

  **It deliberately does not ship every rung of team → space → folder → list → task**, and a test
  (`the_curated_set_stops_short_of_every_navigation_rung`) pins that rather than leaving it to a future
  author's restraint. Five navigation endpoints would have added five rows and taught nothing.

  Provenance is better than most of this catalogue: all six shapes were verified against ClickUp's
  published reference rather than recalled.

- **The Calendly connector (C-172).** Five read operations over a bearer token, `GET /users/me` as
  `verify`.

  **It was filed expecting a refusal and shipped instead, which is the more useful outcome.** The
  story predicted a collision with the unencoded-query gap, since Calendly identifies a resource by
  its full `https://api.calendly.com/…` URI passed as a *query value*. Measured, that is wrong: a
  Calendly URI's charset — scheme, host, path, hyphens — is structurally disjoint from the emitter's
  four-character danger set (space, `&`, `#`, `=`), so the value survives verbatim. The argument is
  tied down by a schema-level `pattern` that mechanically excludes the danger set, not by prose.

  A path parameter is the **bare uuid, never the full URI**, so the template cannot double-compose a
  URL from a value that is already one. `min_start_time`/`max_start_time` and invitee-email filtering
  are excluded: ISO-8601 offsets and `+`-tagged addresses each carry the `+`-in-query hazard this
  connector was not chosen to demonstrate.

- **The Figma connector (C-176).** Six read operations over `X-Figma-Token`, following Shopify's
  existing `header` scheme rather than inventing a second spelling — no change to the auth model.

  Because Figma's API is read-only, **every operation is uniformly `risk = low` and idempotent, and
  that uniformity is asserted** rather than varied for appearance. A catalogue entry that is honestly
  all-idempotent is useful evidence for the tool-contract surface.

  `figma-image-render-get` returns URLs that expire. The connector says so and deliberately states
  **no duration** — a wrong number would be exactly the plausible-but-incorrect output this pipeline
  refuses. The `ids` query parameter carries a `pattern` restricting it to a charset disjoint from the
  emitter's unencoded-query danger set.

- **The Box connector (C-171).** Six operations over a bearer token, with `GET /users/me` as
  `verify`. Box's root folder id is the literal `"0"` — a sentinel a model guesses wrong unless told,
  so it is declared as a JSON Schema `default` and in prose on every folder-id parameter.

  The download endpoint is **excluded**: it answers `302` to a signed URL, and redirect-following is
  not a declared behaviour of this pipeline, so shipping it would make success depend on a client
  setting nobody declared.

  `box` is a Rust keyword. Traced rather than worked around: `catalog.rs`'s `module_ident` and its
  `RUST_KEYWORDS` table already escape it to `r#box` in the generated index, and the Flux side never
  derives an identifier from a provider id — so nothing needed changing.

### Changed

- **`COVERED_FLOOR` is coordinator-owned, and `AGENTS.md` now says so.** The response-schema ratchet
  is a **ninth** staleness check the eight-red table did not name, and it is red per *wave* rather
  than per *story*: C-166 and C-171 each declared complete response shapes and each saw exactly eight
  red alone, but together they crossed the ratchet's one-tenth slack (105 of 123 against a floor of
  92). Raised to 105. Two implementors raising one constant would collide on one line, which is the
  failure C-104 exists to prevent.


### Added

- **The GitLab connector (C-166).** Seven operations — issues (list, get, create), merge requests,
  a pipeline, branches, and `GET /user` as `verify` — under a bearer personal access token.

  **A project is addressable only by numeric id.** GitLab's other form is a URL-encoded namespace
  path (`group%2Fproject`), and this pipeline does not percent-encode values — the same gap that
  makes `zendesk-ticket-search` non-functional, in the path position rather than the query. Rather
  than ship a parameter a caller must pre-encode by hand, every project-scoped parameter is a JSON
  Schema `integer` and each one's `description` says the path form is unsupported. That is C-106's
  selection rule applied again: an operation ships only if it can address everything it needs.

  Self-managed GitLab (a `{host}` binding) is out of scope, matching Sentry's status.


### Added

- **An operation can declare its request-body encoding (C-144).** A closed `BodyEncoding { Json, Form }`
  on `ParamSet`; `json` remains the default and its serialization is skipped, so the lockfile hash
  domain, every manifest, the catalogue and all 256 artifacts are unchanged — now asserted against the
  committed per-operation renderings rather than assumed.

  Three shapes are refused rather than emitted, each because the alternative is a request a vendor
  answers `200` to and ignores: a nested field under `form`, a braced wire name, and a `body_encoding`
  on an operation that sends no body.

  **`PARTIAL`, for a measured reason.** flux had no form encoder and was not close: `parse`'s `as_type`
  is restricted by flux-lang's own analyzer, so `as: "form"` failed *analysis* rather than runtime, and
  all three percent-encoders in that tree are private Rust unreachable from a Flux program. So form
  values are interpolated verbatim for now — the same class as the already-recorded query-encoding gap,
  in a second request position. Nothing ships as `form`, so nothing is exposed.

  The missing encoder was implemented upstream as flux's `L-101` and is committed there; it reaches this
  repository only when flux publishes, since the flux-lang pin must stay a crates.io release.


### Fixed

- **A credential the redactor will not hold is now refused rather than sent (C-152).** flux's
  `Redactor::add_secret` silently ignores a value under six trimmed characters — correct for flux, since
  over-redacting a common word would corrupt every surface it touches — so a five-character stored
  credential registered *successfully* and travelled unredacted through all four surfaces. C-116 stated
  the resulting property unconditionally. **The code was correct about what it did; the prose was wrong
  about what that meant, and the prose is what a reader relies on.**

  The check **asks the redactor rather than mirroring the threshold**: register the value, then assert
  that scrubbing it changes it. A mirrored `6` would have gone stale on a flux upgrade with nothing
  failing, and asking also covers the empty and all-whitespace cases without naming them. An independent
  review probed both directions empirically — a value the redactor holds always rewrites, and no longer
  registered value can rescue a short one, because stored values are trimmed and ≥6 and so cannot be
  substrings of shorter ones.

  Also: `auth::Assembled` gained a redacting `Debug` matching `Secret`'s; the `view` surface of the
  guarantee test is no longer asserted against an empty string; and every value now passes one door,
  which closes the window where a secret was in memory before registration.


### Fixed

- **A verification field reached the IR and neither consumer (C-151).** C-141's `timestamp_format` was
  published in the loader and the JSON schema but silently dropped from the manifest and `catalog.json`,
  because `ManifestHmac` and `HmacEntry` enumerate `HmacSpec`'s fields **by hand**. Both now carry it.

  The fix that matters is not the field — it is that the hand-enumeration stopped being a place a field
  can go missing. The authoritative list is now **derived** from `provider::accepted_keys()`, which reads
  the field names out of `deny_unknown_fields`' own error, so a field added to `HmacSpec` fails a test
  with **no edit to any test file**: first at the every-field fixture, then at whichever projection
  forgot it.

  Neither projection could consume `HmacSpec` directly, and the reasons are recorded beside both types:
  the manifest's field order is load-bearing (TOML places a nested table after its parent's scalars, and
  `HmacSpec` declares `secret`/`tolerance` *after* `timestamp`, so flattening would emit a manifest that
  reparses wrongly), and `catalog.json` publishes every key always while `HmacSpec` skips its `None`s.

  Both artifacts publish the **effective** spelling rather than passing the declaration through, so a
  host reads the answer instead of having to know the IR's default.


### Fixed

- **The integration test harness leaked its fixtures into a shared tmpfs (C-150).** `Fixture::new`
  rooted every fixture at `std::env::temp_dir()` with a `{label}-{pid}-{counter}` name — and `/tmp` here
  is a 32 GB tmpfs, so a pid plus a process-local counter does not separate two agents running the same
  binary. **Two agents reproduced it independently in one wave**, one measuring it take down `wiring`,
  `no_network`, `service_units` and `site_catalog`.

  **This is what made the integration gate untrustworthy twice**, and once cost a good merge that was
  reverted before the cause was measured. Fixtures now live under the build's own `target/`, follow
  `CARGO_TARGET_DIR`, carry a run-scoped name, and are removed on every path. Verified over **20 full
  workspace runs** under two concurrent cold builds on the real disk, zero fixtures surviving.

  The two spellings of this fix — `artifact.rs` from C-143 and the harness here — now derive their root
  the same way, differing only in directory name so the two harnesses stay distinguishable.

- **`AGENTS.md` and `README.md` were three releases stale**, claiming 17 providers and 237 artifacts
  against a build that reports 19 and 256. And `AGENTS.md`'s Validation gate omitted `--no-fail-fast`
  while the same file argues for it two sections earlier — the exact omission that once made it claim a
  new provider leaves three tests red when it leaves eight.

### Changed

- **The babelforce IVR epic's premise was refuted by its own inventory (C-130).** The epic proposed
  exposing babelforce's IVR *atomics* — `audioplayer`, `read`, `switchnode`, `dial`, `recording`, `acd` —
  as operations, since the vendor's call modules are compositions of them.

  Written from the Go source before any TOML, the inventory found **the atomics have no wire identity**:
  `parse_settings.go` maps *call-module* names onto them and the internal `v2.*` identifiers appear in no
  wire document, so the composition-vs-primitive split the epic wanted has already happened *inside*
  babelforce, behind its API. There is no `audioplayer` to address — only `promptPlayer`. The one
  endpoint carrying a `module` field is an unmounted CRUD resource this repo already excludes as account
  provisioning, and `dial` places no call: it writes configuration.

  **A connector cannot publish what a vendor does not expose.** What landed instead: the inventory, a
  fence test proving no operation is named after a call module (verified to have teeth by adding one),
  and babelforce's first per-provider contract test — it had none. The story is re-scoped onto the six
  endpoints that *are* mounted at `/api/v3`, two of which are text-to-speech.


### Fixed

- **A signature scheme that verified forgeries (C-141).** `signed = "{timestamp}"` with a selector and a
  tolerance **loaded cleanly** and signed a body-independent string — so one captured signature verified
  any forged payload for the whole window. Reachable with no typo at all, unlike the unterminated-brace
  bug C-60 fixed. The failing-first test *demonstrates* the forgery rather than asserting the refusal.

  Reworked once, and the rework matters: `parse_tolerance` scaled with `*`, unchecked. In debug that
  panicked inside `provider::load`; in **release** `i64::MAX * 60` wrapped to `Ok(-60)` — a negative
  window that satisfies both bounds and therefore **loaded**. That is the same defect the story exists to
  close, reintroduced for the overflow class. Now `checked_mul`, verified in both profiles, and the test
  asserts the *property* — any accepted window falls in `1..=MAX` — rather than the two spellings found.

  Also: `tolerance` is parsed rather than accepted as any string, a body-sourced verification timestamp
  is refused (honouring it would require parsing before verifying), and `HmacSpec` gained a timestamp
  *format* axis so the verifier reads the spelling instead of sniffing it.

### Added

- **A measured floor under response-shape coverage (C-126).** Re-measured on entry at **29 of 110**, not
  the design's 16 of 97 — that figure predated Stripe and Notion. Now **92 of 110 (83%)**, with 63 new
  schemas across 13 providers, each citing the vendor reference it came from and nothing fetched.

  The floor is the deliverable. **Two** floors, because a count cannot see the regression that actually
  happened between the design's measurement and this story — operations *arriving* without shapes — so a
  ratio floor sits beside it. A third guard stops a floor nobody raises from quietly ceasing to measure,
  and a fourth refuses `{}` or `true` or any schema with no members, so absence stays absence by
  enforcement rather than by an author remembering.

  Eighteen are deliberately absent, each with its reason recorded — including nine babelforce operations
  whose only authoritative reference cannot be vendored, because the response examples are where
  credential-shaped values live.


### Added

- **A connector can now authenticate (C-116).** A `CredentialStore` port is bound when the pack is
  constructed — never looked up globally — over C-91's `SecretStore` and C-90's `CredentialRef`
  addressing, which had no consumer until now. Auth is assembled **in Rust**: the `Bearer` prefix, the
  basic-auth base64, query placement, honouring the source × acquisition × placement axes.

  That is what takes flux's `$auth` seam off the critical path. The whole-value `{"$secret"}` marker
  never needs to grow prefix or encode support, because the pack builds the header value itself.

  **The secret is registered with the redactor before the request is constructed**, so a failure
  between construction and dispatch cannot surface it. An independent review reproduced the proof by
  removing only that registration and watching the test go red, confirmed the four `ToolResult`
  surfaces are the complete set, and established that `permission_subjects` returning the
  *unauthenticated* URL is **necessary** rather than merely tidy: flux consults it before `execute`, so
  the redactor is still empty at that moment.

  A missing credential names the `CredentialRef` that was not found and sends nothing.

  Follow-ups from the review are filed as C-152 — most importantly that flux's `Redactor` silently
  drops values under six characters, so the guarantee as documented is stronger than the one that holds.


### Fixed

- **A test that reported `ok` when it skipped (C-149).** The live Vault leg printed `ok / 1 passed`
  when no Vault was offered, with the reason captured and invisible — while its own module doc claimed
  *"there is no third path where it reports success without having talked to anything."* A reader of a
  green run could not tell whether the HTTP transport had been exercised.

  It is now reported as `ignored` with a message naming what is **UNEXERCISED**, and the claim is held
  by a guard test that re-execs the binary and reads libtest's own report — because the claim is about
  what a reader sees, so nothing short of the real output can prove it.

  Three smaller gaps from the same review closed beside it: `Secret::into_inner` became
  `expose_secret_owned` so the type's "one search for `expose_secret`" audit is *true*, and is now
  asserted by reading the source rather than promised in a comment; `StoreError::Layout` was wired to a
  caller instead of being a typed error nothing could raise; and a KV v1 test was renamed to what it
  actually proves, with the real 404 case added beside it.


### Fixed

- **The artifact tests leaked their fixtures into a shared tmpfs, and went flaky under load (C-143).**
  They wrote to `std::env::temp_dir()`, keyed on a label and a process id; `/tmp` here is a 32 GB
  tmpfs, and 55 leaked fixture directories were sitting in it. Under a wave of concurrent builds,
  tmpfs pressure was enough to fail a write.

  Fixtures now live under the per-worktree build tree, follow `CARGO_TARGET_DIR`, carry a name no run
  repeats, and are removed by a `Drop` guard even when a test panics. The three original tests each
  lost exactly one cleanup line and still assert the same properties. Verified over 10 full workspace
  runs under concurrent build load: 10/10 green, zero fixtures surviving.

  **This cost real time twice before it was measured** — both times the first hypothesis was "the
  merge broke it", and in one case a good merge was reverted before the cause was found. A flaky
  integration gate is worse than a missing one, because it teaches a reader to distrust a red gate.

  The wider half is filed as C-150: `tests/common/mod.rs` has the identical bug in the harness *every
  integration binary* uses, and two agents reproduced it independently in the same wave.


## [0.5.0] — 2026-07-30

### Added

- **Every operation publishes one composed `input_schema` (C-125).** Path, query, header and body
  parameters plus `body_schema` become a single object schema with `properties` and `required`,
  derived and never authored, published per operation in `catalog.json`.

  **The merge rule `ir.rs` left unstated is now stated — as a refusal.** An operation declaring both
  named body fields *and* a free-form `body_schema` no longer loads. Refusal rather than a merge
  because there is no rule to write down: "the body is these fields" and "the body *is* this schema"
  describe the same bytes two ways, and every possible merge is a decision no vendor document
  supports whose failure mode is a request the vendor answers `200` and ignores. The refusal moved
  from the emitter to the **loader**, making it an invariant of the IR rather than of one back-end.

  **Two derivations are held together by a test rather than collapsed**, because they genuinely
  answer different questions. `connector-pack` keys by Flux *symbol* — babelforce's `time.start` is
  `time_start` in a declaration — and the name→symbol mapping lives a dependency edge downstream of
  the IR. And flux has no optional composite-op parameter, so the pack's `required` is necessarily
  *every* parameter, while the composed schema states what the **vendor** requires. Collapsing them
  would have published as optional a parameter the pack's own request builder rejects. An agreement
  test over all 105 shipped operations asserts they describe the same parameter set modulo the symbol
  mapping, with a proper-subset guard so the divergence stays tested rather than assumed away.

- **The Notion connector (C-107)** — the nineteenth provider, 256 artifacts. Five operations, with
  `Notion-Version: 2022-06-28` emitted as a **literal** on every request.

  That header is why the first attempt refused to ship. The emitter used to chain every header
  parameter in unfiltered, so a required constant emitted as a caller-supplied argument and the
  connector would have returned 400 on every call — while `every_shipped_provider_compiles` stayed
  green, because the module parsed, formatted and round-tripped perfectly. The gate could not see it.
  C-55 fixed the emitter and this is the connector that proves it.

  Scoped honestly: a Notion block is a ~30-way *recursive* discriminated union and this repo's
  `JsonSchema` has no `$ref`, so page **content** is out of scope and `notion-page-get` says it
  returns properties rather than text. The filter/sort DSL and the property `PATCH` are excluded for
  the same reason — their keys are tenant-defined.

- **A connector operation can now reach a vendor, with the network gate mirrored (C-115).** Each Tool
  builds `{method, url, headers, body}` and delegates to flux's own `http.request`, so flux keeps
  every byte of egress.

  The safety property is the story. Delegating directly **bypasses `Executor::dispatch`**, so the
  inner call never consults `http.request`'s own `permission_subjects` or its `NetworkFetch` intent —
  and both have trait defaults returning empty. A Tool that omitted them would compile, register,
  execute, reach the vendor, and never be gated. An independent review probed **1470
  (operation, params) pairs** and established the strong property: `permission_subjects(p)` *equals*
  `vec![build_request(p).url]` whenever a request builds, so the declared subject cannot drift from
  what is reached.

  The request is evaluated from the operation's **own emitted Flux** rather than re-lowered from the
  IR, so the pack's request is the module's request by construction. The node set is closed and
  anything unmodelled refuses — a partly-evaluated request is not a degraded request, it is a
  different call, and the vendor answers it.

- **Events and channel bindings reach the manifest and the catalogue (C-83).** Verification publishes
  as a **total** three-valued `kind` plus a `verified` flag with no `skip_serializing_if`, so C-82's
  "a deliberately-unverifiable surface stays loud" is structural: a consumer tells it from a verified
  one by reading a value, never by noticing a missing key. Nothing reaches the `.flux` module, and the
  emitter refuses rather than dressing an event up as a pollable op. The site renders the inbound
  surface.

- **A provider can pin a vendor-constant request header (C-55).** `const_headers` emits the value as a
  literal instead of a caller-overridable argument. A `const`-pinned `params.header` — which silently
  dropped the constraint and shipped a required argument any caller could set to anything — is now
  **refused** rather than reinterpreted.

  A constant header can never carry a credential: nine spellings are refused, including a declared
  env-var name, a credential name, `${ENV}`, and CRLF injection.

  `providers/github.toml` declares its `Accept: application/vnd.github+json` and the SCHEMA GAP note
  it carried since C-52 is gone. This also unblocks Notion, whose required `Notion-Version` header is
  why C-107 was parked.

- **`connector-secrets` — a secret store trait and a Vault KV v2 implementation (C-91).** A **host
  library, outside the compile path**: `connector-cli` must not depend on it, and that is now asserted
  rather than assumed.

  The fence is the valuable part. It parses `Cargo.lock` rather than asking `cargo metadata`, because
  the lock records **optional** dependencies — so the edge trips the test even when added behind a
  feature flag, which is how the invariant would realistically be broken. An independent review
  verified it four ways: at the merge base, through an optional dependency, through a
  dev-dependency, and through a **real transitive edge** (`connector-cli → connector-flux →
  connector-secrets`) rather than only a synthetic graph. A default `cargo test --workspace` produces
  **zero** reqwest/hyper/rustls artifacts, so `no_network.rs` keeps meaning what it says.

  `Secret` has no `Serialize`, `Display`, `Deref`, `AsRef` or `Hash`, and a `compile_fail` doctest
  pins the first — confirmed by the reviewer to fail for the right reason rather than on a mistyped
  path. Every Vault semantic is tested offline against a scripted transport, and the review recorded
  plainly what that does and does not prove: the store's URL construction, envelope parsing and status
  mapping — nothing about Vault itself.

- **The Stripe connector (C-106)** — the eighteenth provider. Eight operations selected from roughly
  450, graded by what they do to money: the refund is `destructive`, capture and cancel are `high`,
  and all three are `conditional` **earned rather than asserted** — each declares a *required*
  `Idempotency-Key` header, stricter than Stripe itself, because leaving it optional would tell flux a
  retry is sound while permitting the request that makes it unsound.

  The webhook binding is **deliberately unshipped**. Stripe's `Stripe-Signature` is a `t=…,v1=…` list
  that `HmacSpec` cannot address, so `verification = "none"` would present an unverified payments
  endpoint as trusted, and an `hmac` block naming the header whole would *read* as verification while
  comparing a digest to a key/value list. The four `[[events]]` ship; a test fails the moment a
  binding appears without C-141.

  It also shipped **without** the canonical `POST /v1/refunds`, using the legacy charge-nested form
  instead, and with capture and refund restricted to full amounts — all three because of C-144.

- **A flow graph lowers to one composite Flux op (C-95).** Operation, Select, Template, Object,
  Literal, Gate, Approval, Retry and Throttle lower through `flux_lang::ast`; Trigger, Schedule and
  Endpoint are boundary declarations that reach no statement. Cycles, region-crossing edges, unbound
  region outputs and a `Select` wired to an operation's response are all refusals rather than
  degraded output.

  It found an upstream defect and refused rather than working around it: **flux-lang 0.39's two
  formatters disagree about durations.** `format::fmt_duration` never emits a bare number, so every
  `throttle` and every `retry` with a delay produces text `format_cst` declines to re-print — though
  both spellings parse to the same AST. Emitting a module flux's own formatter cannot format would
  have been the alternative, so `throttle` is pinned as a refusal with the three steps to undo it
  recorded.

- **A service can declare the roles it implements (C-120).** `[[services]] roles = [...]` as a
  **closed, checkable** set: an unknown role name is refused rather than ignored, because a typo'd
  capability that silently means "no capability" is the failure the mechanism exists to prevent. A
  provider's roles are derived as the union of its services' and are never authored — roles attach to
  a *service* because a vendor's model-listing surface and its chat surface are different
  capabilities.

  Two holes an independent review demonstrated with probes were closed before merge. A `default`
  service entry was accepted *alongside* named services, which repealed the rule that a provider
  declaring named services has no implicit `default` — an operation omitting `service` became legal
  again. And a role slot could be filled by **any member kind, including an event**, which would
  publish a live-listing capability nothing can call, since an event is emitted into no module.

- **The Tool pack's declaration half (C-114).** `crates/connector-pack` projects a catalogue
  operation onto a flux `ToolSpec`, so a host can register a provider's operations into a
  `ToolRegistry` and resolve them by dotted name (`zendesk.ticket.comment.add`) — the spelling flux's
  reference flow uses and one a composite declaration cannot have. 97 operations across 17 providers
  register and resolve.

  Two findings the work turned up. `ToolSpec::access` cannot be left empty: flux **refuses the
  registration** of a declared network effect with no carrying access kind, so the pack derives access
  from effects and re-runs flux's own checker at projection time. An independent review reproduced
  that refusal verbatim and confirmed the derivation picks the narrowest carrier rather than
  over-granting. And the projection reads the *embedded Flux declaration* rather than the catalogue's
  flat columns, which makes the pack's answer the module's answer by construction — removing the
  drift C-117 exists to guard, for the declaration half, instead of testing for it.

- **Verification conformance against real vendor vectors (C-60).** A parameterized matrix over
  GitHub, Slack, Zendesk and Stripe, with the HMAC primitive pinned separately to RFC 4231 so the two
  vendor-published triples count as independent evidence rather than the implementation agreeing with
  itself.

- **A tool contract is now readable.** The core explorer rendered `Risk`, `Idempotency`, `Effects`,
  `Access` and `Group` as bare text and dumped the input schema through an unhighlighted
  `JSON.stringify`. The safety fields are now chips whose tone is **derived from the value** — so a
  risk level cannot read calm on one page and alarming on another — and the schema is syntax
  highlighted with a JSON/YAML toggle and a copy button.

  An unrecognised value stays neutral rather than being guessed at: a wrong colour on a safety field
  would read as an assurance nobody made.

  The highlighter is hand-rolled and about forty lines, deliberately. Shiki is a *build-time*
  dependency in VitePress and this content is read from the catalogue at **runtime**, so the built
  pipeline does not apply; a client-side highlighter would cost more bytes than the catalogue. Tokens
  render as elements rather than through `v-html`. Every colour is a VitePress token, so light and
  dark both work and a theme change carries automatically.

### Changed

- **The explorer components are no longer welded to VitePress (C-142).** Six of fourteen imported
  from `vitepress`, and between them they used exactly two symbols — `withBase` to prefix an href,
  and `inBrowser` for a `typeof window` guard. No `useData`, no `useRouter`, no theme internals. So
  detaching them was a **link port and a tier boundary, not a rewrite**: components now take a path
  resolver through `provide`/`inject` with an identity default, and the site supplies `withBase`.

  The three tiers are written down — presentational (props only), catalogue-aware, and page (owns
  routing and state) — and a test enforces the import allow-list mechanically, so a component that
  reaches for its own data fails the suite rather than review.

  "Identical" was verified rather than asserted: the merge base and the branch were built separately
  and compared page by page across every `core/**` and `operations/**` page, with zero differing
  files once Vue's scoped-style ids and chunk hashes were normalised.

  **No npm package was extracted.** `web/package.json` stays `private: true` — publishing a component
  library is a distributed artifact with its own versioning and consumers, and that decision waits
  for a second consumer to shape it.

- **Whole-catalogue artifacts are coordinator-owned (C-104).** The provider index is generated on a
  full build only, so provider stories no longer collide on a hand-maintained list. The real class is
  **four** artifacts, not the two the story assumed — enumerated by differencing a full plan against a
  scoped one.

  The rework corrected two documented claims that were measurably wrong, and both mattered because
  they are the instruction implementors follow: a new provider leaves **eight** tests red, not three,
  and a changed provider three, not one. Both undercounts came from plain `cargo test --workspace`
  stopping at the first failing binary — the trap `AGENTS.md` documents elsewhere. Both gates now say
  `--no-fail-fast`.

- **`AGENTS.md`'s "does not depend on the flux runtime" is now scoped to the compiler crates**, since
  `connector-pack` links `flux-runtime`/`flux-spec` by necessity. This repository still constructs no
  runtime.

- **flux-lang 0.37 → 0.39, and every generated module is rewritten in the new canonical syntax.**
  Flux's L-93 changed what canonical source looks like: local bindings lose the `$` sigil, object
  fields pun when the field and symbol names agree, and calls take direct named arguments. So
  `$payload = { channel: $channel, text: $text }` / `http.request({ method: "POST", url: $url })`
  becomes `payload = { channel: $channel, text }` / `http.request(method: "POST", url)`.

  The change is syntactic — no operation's method, host, body shape, or credential handling moved.
  117 of 236 artifacts were regenerated, and the build is a fixed point again. The compatibility
  claim is measured rather than assumed: every provider's `…_emits_a_module_that_parses_analyzes_and_is_canonical`
  test requires the emitted module to parse with no errors, be a **fixed point of flux's own
  formatter**, and load as a program with exactly one exposed op. All pass under 0.39.

  The sigil survives exactly where a bare name would collide with a Flux keyword — `$channel`,
  `$include` — which is why some fields pun and their neighbours do not.

### Fixed

- **`web/README.md` still carried the reasoning that shipped an unstyled site.** It claimed the
  committed CNAME serves the site from a custom domain "so `config.mts` sets `base: '/'`" — the exact
  inference that broke production, and one the config and its test have contradicted since. Rewritten
  to say where the site is actually served from and what evidence would justify changing it.

- **A signature scheme that verified forgeries (C-60).** `signed_placeholders` silently swallowed an
  unterminated brace, so `signed = "v0:{timestamp}:{body"` — one missing character, a plausible typo —
  passed every loader check and produced a signed string **containing no body at all**. A signature
  captured from one delivery would then verify any forged payload for the whole tolerance window. The
  fragment now comes back as a placeholder no host can fill, so the loader's existing refusal catches
  it.

- **The explorer set a floor under its own width (C-100, follow-up).** Three symptoms — 193px of
  horizontal overflow on a phone, a filter bar that always wrapped to two rows, and a provider grid
  stuck at three columns — turned out to be one cause: a flex or grid item's automatic minimum size
  is its `min-content`, so a `<select>` (as wide as its widest option), a row (as wide as its longest
  request path) and a card header (274px) each refused to shrink and pushed their container instead.
  Measured after: overflow 193px → 0, filter bar 2 rows → 1 from 1280px up, provider grid 3 → 4
  columns from 1440px up.

  The earlier reasoning that 320px was "the smallest round number above the 314px floor" is kept in
  the story as the thing that was wrong rather than deleted: it identified the cause correctly and
  drew the wrong conclusion, because the floor was never a fact about the card — it was one missing
  declaration.

- **A sigil-matching test would have gone vacuous under the upgrade.** Nine assertions checked the
  *absence* of an emitted symbol by its old spelling — `!module.contains("$sep")`, which pins that no
  operation assembles a query string. With the sigil gone those became true no matter what the
  emitter did, so the query-string guard would have passed while asserting nothing. They now match
  the binding (`sep = `). The same class was fixed in the `$payload` / `body: $body` negative checks,
  and the dotted-symbol guard now matches the interpolation form `{time.start}` rather than a
  substring the vendor's own wire name legitimately contains.

- **The published site rendered unstyled.** `web/.vitepress/config.mts` had been set to
  `base = '/'` on the strength of the committed `web/public/CNAME`, but GitHub never accepted that
  custom domain — the Pages API still reports `"cname": null` and serves
  `https://codewandler.github.io/flux-connectors/`, so every bundled asset resolved a level too high
  and 404'd. Restored the project-pages prefix.

  The test that was supposed to guard this asserted the aspiration (`base === '/'`, CNAME contents)
  rather than anything falsifiable, so it locked the breakage in instead of catching it. It now
  asserts the built HTML's own asset URLs sit under the deployed base, and was checked to fail when
  the base is wrong.

## [0.4.0] — 2026-07-30

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
