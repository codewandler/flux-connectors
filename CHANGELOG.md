# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

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
