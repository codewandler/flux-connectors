# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
