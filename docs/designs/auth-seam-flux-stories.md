# Handoff: ready-to-paste flux stories for the outbound `$auth` seam

> **This file is a handoff artifact, not a tracked backlog.** Nothing in it is a story on
> *this* repo's board, and `/track:board` must never pick these up. Each block below is a complete
> story file destined for **`../flux`**'s `docs/stories/`. A human copies a block verbatim into
> `/home/timo/projects/flux/docs/stories/<id>-<slug>.md` and runs flux's own `/track:board`.
>
> Source design: [auth-seam.md](auth-seam.md) · Parent story:
> [C-16](../stories/C-16-design-auth-seam.md)

## Before you paste

- **IDs are provisional.** The highest `C-` id in flux at the time of writing is **C-265**, so these
  claim **C-266 … C-276**. flux's fleet allocates ids concurrently — re-check with
  `ls ../flux/docs/stories | grep -oP '^C-\d+'` and renumber the block (and its cross-references)
  if that range is taken.
- **Do not call this "the auth seam" on flux's board.** flux already has a *done*
  `request-auth-seam` (`docs/designs/request-auth-seam.md`, stories D-64/D-68) for **inbound**
  bearer→principal resolution. Every title below says **outbound** for that reason.
- **A design doc must exist in flux** before these land, since each sets
  `design: docs/designs/outbound-auth-marker.md`. Either port
  [auth-seam.md](auth-seam.md) into flux under that name, or drop the `design:` line from each
  block. Do not leave a `design:` pointing at a file that does not exist in flux.
- **Vocabulary.** These drafts say **credential** where an earlier revision said "purpose": an
  AND-group of credentials is a *mechanism*, and alternatives are alternative mechanisms.
  **flux's own `AuthMethod.purpose` field is NOT renamed** — our `credential` name resolves to it.
  Every flux identifier (`AuthMethod.purpose`, `auth_purpose`, `resolve_purpose`, the
  `plugin:<name>:<purpose>` store key) is quoted verbatim below and must stay that way.
- **Layer facts these stories rely on** (verified in flux at `bcfab0ad` + working tree):
  `flux-plugin-protocol` is L0 and `flux-web` is L5 (`crates/flux-codegate/src/lib.rs:36`, `:51`);
  flux-web already reaches `AuthScheme`/`AuthMethod` today via its existing `flux-plugin` dep, since
  `crates/flux-plugin/src/lib.rs:35` is `pub use flux_plugin_protocol::*;`.

## Sequencing

```
F-1 (C-266) extract apply_scheme → L0
      ↓
F-2 (C-267) $auth marker + WebOptions.auth_credentials ──┐
      ↓                                               │  must ship in the SAME release
F-3 (C-268) deny-by-default, proven ──────────────────┘
      ↓
F-4 (C-269) redactor registers the composed value
      ↓
F-5 (C-270) http_hosts scoping          F-6 (C-271) Query-scheme injection
      ↓
F-9 (C-274) AuthMethod composes Basic halves from env + literal
```

**F-9 is on the critical path for two of our three providers** (zendesk, freshdesk) — it is not a
nice-to-have. See F-9's own Goal.

`F-7 (C-272)`, `F-8 (C-273)`, `F-10 (C-275)` and `F-11 (C-276)` are **non-blocking**: an adjacent bug
found in flux's plugin host, the manifest trust-model decision the design removed from the critical
path, the JWT/confidential-client contingency that becomes urgent only once babelforce answers a
question, and the work that would let *effectful* acquisition (OAuth2/session) run host-side for a
connector. The three in-scope providers need only pure acquisition, so F-11 is filed for shape, not
for schedule.

**Companion design:** these drafts must stay consistent with this repo's
[unified-auth.md](unified-auth.md) (source × acquisition × placement), whose presets are flux's four
`AuthScheme` variants. **F-9 / C-274 is what makes that preset claim true** — see
[auth-seam.md](auth-seam.md) §9.2, which reports three round-trip failures against it.

**Safety constraint, stated once:** F-2 introduces a header marker that resolves credentials.
It must carry its minimal refusal path in its *first* commit and must not reach a release without
F-3. F-3 is filed separately because it is where the full refusal envelope is *proven*, not because
the refusal may be deferred.

---

## F-1 → `C-266-outbound-auth-scheme-application-to-l0.md`

```markdown
---
id: C-266
title: Extract AuthScheme application into flux-plugin-protocol (L0) so one impl composes credentials
pillar: Core
status: ready
priority: 3
epic: outbound-auth-marker
design: docs/designs/outbound-auth-marker.md
note: "prerequisite for the outbound $auth marker — today the Basic base64 composition is inline and private in flux-plugin"
---

# Extract AuthScheme application into flux-plugin-protocol (L0)

## Goal
Turn credential composition into one shared, pure, public function so a second consumer (flux-web's
outbound `$auth` marker) cannot fork a second base64 implementation. Today the logic is private and
welded to the plugin host: `AuthInjection` is a private enum (`crates/flux-plugin/src/host.rs:755`),
`resolve_auth` is a private method on `SystemHostCaps` (`:644`), and the actual composition is inline
inside one arm of `HostCapabilities::handle` — Bearer at `:1248-1252`, the Basic base64 at
`:1253-1261`, custom Header at `:1262-1264`, and the `Query` URL mutation earlier at `:1229-1231`.

## Acceptance
- [ ] `flux-plugin-protocol` (L0) exposes a pure, public `AuthScheme` application:
      `pub enum AuthInjection { Bearer(String), Basic{..}, Header{..}, Query{..} }` plus
      `pub fn apply_scheme(scheme: &AuthScheme, secret: &str, user: Option<&str>) -> Result<AuthInjection, String>`
      and a `pub fn authorization_header_value(inj: &AuthInjection) -> Option<String>` that performs
      the `Bearer ` prefixing and the `Basic base64(user:secret)` composition. No IO, no env reads.
- [ ] Failing-first test (in `flux-plugin-protocol`):
      `apply_scheme_basic_composes_base64_of_user_colon_secret` — asserts
      `authorization_header_value(apply_scheme(&AuthScheme::Basic, "tok", Some("me@x.com")))`
      is exactly `Basic bWVAeC5jb206dG9r`. It fails to compile/link before the extraction because
      no such public function exists.
- [ ] `flux-plugin`'s `http.do` path calls the extracted function instead of composing inline; the
      inline base64 at `crates/flux-plugin/src/host.rs:1253-1261` is deleted, not duplicated.
- [ ] The existing plugin-host auth tests still pass unchanged — in particular the `resolve_auth`
      table around `crates/flux-plugin/src/host.rs:3696-3735`. Behavior is byte-identical; this is a
      pure move.
- [ ] `cargo run -p flux-codegate` (or the repo's layering check) stays green: the new code is L0 and
      depends on nothing.

## Progress
- (not started)

## Notes
- `AuthScheme` is at `crates/flux-plugin-protocol/src/lib.rs:344-354`; `AuthMethod` at `:422-443`.
- `Query` deliberately produces no header — it mutates the URL. Keep that asymmetry explicit in the
  returned type rather than hiding it, exactly as `crates/flux-plugin/src/host.rs:1247` does today.
- Do **not** move `resolve_auth` itself: it reads plugin manifest state and the credential store.
  Only the pure scheme→bytes step moves.
```

---

## F-2 → `C-267-outbound-auth-header-marker.md`

```markdown
---
id: C-267
title: "`http.request` accepts the outbound `{\"$auth\": {\"credential\": \"…\"}}` header marker"
pillar: Core
status: ready
priority: 3
epic: outbound-auth-marker
design: docs/designs/outbound-auth-marker.md
note: "unblocks flux-connectors: Bearer/Basic are unreachable today because $secret is whole-value replacement only"
---

# `http.request` accepts the outbound `$auth` header marker

## Goal
Let a caller name a **credential** in a header value and have the host compose it per its declared
`AuthScheme`, so `Authorization: Bearer <tok>` and
`Authorization: Basic base64(user:tok)` become reachable from Flux. Today `resolve_header_value`
(`crates/flux-web/src/http.rs:234`) supports only `{"$secret": "ENV"}`, which is whole-value
replacement (`as_secret_ref`, `:275`, requires an object of exactly one key) with no prefixing or
encoding — and flux-lang cannot compose it either, since the `expr` built-in whitelist has no
`base64` (`crates/flux-lang/src/expr.rs:136-139`).

## Acceptance
- [ ] `WebOptions` (`crates/flux-web/src/lib.rs:75`) gains
      `auth_credentials: Option<Vec<AuthMethod>>` alongside `allowed_secrets` (`:97`), carrying the
      operator-declared credential→method map. Our `credential` name is matched against flux's
      existing `AuthMethod.purpose` field (`crates/flux-plugin-protocol/src/lib.rs:424`) — that
      field is **not** renamed. `None` = no credentials declared = every `$auth` reference refused.
- [ ] `HttpRequestTool` resolves it once at construction, mirroring `allowed_secrets`
      (field at `crates/flux-web/src/http.rs:46`, `new` at `:50`).
- [ ] `resolve_header_value` recognizes an object of exactly one key `$auth` whose value is an object
      with a string `credential`; a malformed `$auth` shape is a caller error, never a silent
      passthrough to the string branch.
- [ ] `Bearer`, `Basic` and `Header` schemes compose via `flux_plugin_protocol::apply_scheme`
      (C-266). `Query` is explicitly rejected with "use C-271" until that story lands — it must not
      silently emit a header.
- [ ] Failing-first test: `auth_marker_composes_bearer_header_from_declared_credential` — a
      `HttpRequestTool` built with one `AuthMethod::bearer("api_token", vec!["TEST_TOK"])` sends
      `Authorization: Bearer <value of TEST_TOK>`. Before the change the request fails with
      `header values must be strings or a secret reference {"$secret": "ENV"}`
      (`crates/flux-web/src/http.rs:251-256`).
- [ ] Second test: `auth_marker_composes_basic_header_from_user_env_and_secret` — a
      `AuthMethod::basic` credential produces `Authorization: Basic <base64(user:secret)>`.
- [ ] **Several credentials on one request (AND) work.** Test:
      `two_auth_markers_on_different_headers_both_resolve` — a request carrying
      `{"X-Api-Key": {"$auth":{credential:"a"}}, "X-Account-Id": {"$auth":{credential:"b"}}}` sends both
      resolved values. This should need no extra code — the header loop at
      `crates/flux-web/src/http.rs:169-181` already calls `resolve_header_value` once per header
      with no shared state — but it must be **proven**, because "one credential per request" is
      exactly the kind of assumption that gets introduced accidentally later.
- [ ] An undeclared credential is refused **before any env var is read** (the minimum refusal path;
      C-268 proves the full envelope). This must be in the first commit of this story.
- [ ] The op's JSON schema description (`crates/flux-web/src/http.rs:88`, `:99`) documents the new
      marker alongside `$secret`.
- [ ] The exhaustive `WebOptions` struct literal at `crates/flux-cli/src/execution.rs:1529-1541` is
      updated. The four `WebOptions::default()` sites need no change
      (`crates/flux-cli/src/catalog_coherence.rs:149`, `crates/flux-lsp/src/catalog.rs:57`,
      `crates/flux-cli/src/plugin_cmd.rs:1389`, `crates/flux-cli/tests/website_contract.rs:333`).

## Progress
- (not started)

## Notes
- **Requirement-set semantics do not reach flux.** flux-connectors resolves OpenAPI's OR
  alternatives at build time and emits exactly one alternative's markers, so this story needs no
  alternative-selection logic. AND is just "several markers". See the design's §7.
- Two `$auth` markers cannot both target `Authorization` (a Flux object has no duplicate keys). That
  is a permanent, acceptable limit; rejecting such a requirement set is **codegen's** job in
  flux-connectors, not this story's.
- **Do not** add a second `$auth` definition to flux-lang. `$secret` exists twice on purpose
  (`crates/flux-lang/src/program.rs:295` and the inlined copy at
  `crates/flux-web/src/http.rs:273-274`, whose comment explains the deliberate avoidance of an L0
  language dep). `$auth` has no language-level meaning — define it in flux-web only.
- Object keys survive Flux round-tripping: `fmt_obj_key`
  (`crates/flux-lang/src/format.rs:479-485`) JSON-quotes a non-identifier key and the parser
  recovers it (`crates/flux-lang/src/parse.rs:1861`).
- No new dependency is required: `flux_plugin::AuthScheme`/`AuthMethod` are already in scope in
  flux-web via `crates/flux-plugin/src/lib.rs:35`. Taking the direct L0 dep on
  `flux-plugin-protocol` is legal and cleaner; either is fine.
- Ship in the same release as C-268.
```

---

## F-3 → `C-268-outbound-auth-deny-by-default.md`

```markdown
---
id: C-268
title: Deny-by-default credential resolution for the outbound `$auth` marker
pillar: Core
status: ready
priority: 3
epic: outbound-auth-marker
design: docs/designs/outbound-auth-marker.md
note: "the C-76 refusal envelope, applied to credentials — must not reach a release without C-267"
---

# Deny-by-default credential resolution for the outbound `$auth` marker

## Goal
Make the `$auth` marker's refusal path as provably fail-closed as the `$secret` allowlist C-76
established, so a prompt-injected model naming an arbitrary credential gets nothing — and gets it
before any credential value is read from the environment or the credential store.

## Acceptance
- [ ] `auth_credentials: None` and `Some(vec![])` are both **deny-all**, mirroring `allowed_secrets`'
      documented contract (`crates/flux-web/src/lib.rs:90-97`).
- [ ] Refusal happens strictly before any env read, credential-store read, or `user_env` lookup —
      no value is materialized on the refusal path.
- [ ] The error text matches the shape C-76 established at
      `crates/flux-web/src/http.rs:236-242`: it names the refused credential, states it is not
      declared, and tells the operator where to declare it. It must **not** echo any env-var value.
- [ ] Failing-first test: `auth_ref_to_undeclared_credential_is_refused` — direct mirror of
      `secret_ref_to_non_allowlisted_env_var_is_refused` (`crates/flux-web/src/http.rs:571`). Set an
      env var that a *declared* credential would resolve, then reference a *different*, undeclared
      credential; assert the call errors and that the env value appears nowhere in the error.
- [ ] Second test: `auth_credentials_empty_vec_is_explicit_deny_all` — a tool built with
      `Some(vec![])` refuses a credential name that a populated map would accept.
- [ ] Third test: `declared_credential_with_unset_env_is_a_clean_error` — mirrors
      `missing_secret_header_env_is_a_clean_error` (`crates/flux-web/src/http.rs:545`): a declared
      credential whose `env` keys are all unset produces a clean, value-free error, not a panic and not
      an empty `Authorization: Bearer ` header.

## Progress
- (not started)

## Notes
- Precedent story: `docs/stories/C-76-http-request-secret-exfil.md`.
- The plugin host's analogous refusal is
  `format!("no auth method declared for purpose `{p}`")` at
  `crates/flux-plugin/src/host.rs:657` — that is flux's existing string and stays verbatim on the
  plugin path. The `$auth` path should say **credential**, since that is the word its caller used.
- An empty resolved credential must be treated as *absent*, not as a valid empty token.
```

---

## F-4 → `C-269-outbound-auth-redact-composed-value.md`

```markdown
---
id: C-269
title: Register the **composed** credential with the redactor, not just the raw token
pillar: Core
status: ready
priority: 3
epic: outbound-auth-marker
design: docs/designs/outbound-auth-marker.md
note: "the Redactor matches by exact substring, so registering the raw token provably does NOT redact a Basic header"
---

# Register the composed credential with the redactor

## Goal
Guarantee the value that actually travels on the wire is scrubbed from model-visible output. The
`$secret` path already registers what it injects (`ctx.redactor.add_secret(resolved.clone())`,
`crates/flux-web/src/http.rs:248`) — trivially correct there, because the injected value *is* the raw
env value. For `Basic` it is not: `Redactor::redact` (`crates/flux-secret/src/lib.rs:204-215`)
replaces registered values by **exact substring**, and `base64("user:token")` shares no substring
with `token`; the `Basic dXNl…` form also matches no entry in `SECRET_PREFIXES`
(`redact_patterns`, `:232`). Registering only the raw token leaves the real credential in the clear.

## Acceptance
- [ ] Every scheme registers the **composed** header value with `ctx.redactor.add_secret(...)`
      before the request is dispatched.
- [ ] The raw secret is registered **as well**, not instead — an upstream error body can echo the
      bare token.
- [ ] Failing-first test: `basic_auth_composed_header_is_registered_with_the_redactor` — resolve a
      `Basic` credential, then assert `redactor.redact(&composed_authorization_value)` no longer
      contains the base64 payload. Before the change it contains it verbatim, since only the raw
      token was registered.
- [ ] Second test: `bearer_auth_composed_header_is_registered_with_the_redactor` — same shape for
      `Bearer <tok>`.
- [ ] The existing `secret_header_is_resolved_and_seeded_into_the_redactor`
      (`crates/flux-web/src/http.rs:521`) still passes unchanged.

## Progress
- (not started)

## Notes
- **Known limit, do not paper over it:** `Redactor::add_secret` silently drops any value shorter than
  6 characters (`crates/flux-secret/src/lib.rs:198`). A very short vendor key is unredactable by
  construction. That is pre-existing flux behavior; this story must not claim otherwise, and docs
  must not promise otherwise.
- Related, filed separately: C-272 — flux's *plugin* host has the same bug today and composes a
  Basic header at `crates/flux-plugin/src/host.rs:1253-1261` without registering it.
```

---

## F-5 → `C-270-outbound-auth-http-hosts-scoping.md`

```markdown
---
id: C-270
title: Scope an outbound `$auth` credential to declared hosts, using one shared host matcher
pillar: Core
status: ready
priority: 3
epic: outbound-auth-marker
design: docs/designs/outbound-auth-marker.md
note: "the single most important control in the design — without it a credential can be sent to any host"
---

# Scope an outbound `$auth` credential to declared hosts

## Goal
Stop a resolved credential from reaching a host its operator never authorized. Without
this, a generated (or injected) call can name `zendesk.api_token` and point the request at an
attacker's URL — the SSRF guard (`crates/flux-web/src/http.rs:157`) blocks private ranges but
happily allows any public host.

## Acceptance
- [ ] Each declared credential carries an `http_hosts` allowlist; a request whose guarded URL host
      matches none of them is refused **before dispatch and before the credential is resolved**.
- [ ] The check uses **one shared matcher**, not a third copy. `host_matches` exists twice today and
      is private both times — `crates/flux-plugin/src/host.rs:1840` and
      `crates/flux-system/src/net.rs:409`. Make the **flux-system** one public (flux-web already
      depends on flux-system) and have `flux-plugin` call it; delete its private copy.
- [ ] Wildcard semantics match the plugin path exactly, including `*` and leading-label wildcards
      such as `*.zendesk.com` (`plugins/zendesk/src/main.rs:131` is the reference shape).
- [ ] Failing-first test: `auth_credential_is_refused_for_a_host_outside_its_http_hosts` — declare a
      credential scoped to `*.zendesk.com`, issue a request to `https://evil.example.com`, assert the
      call is refused and no `Authorization` header was built.
- [ ] Second test: `auth_credential_is_allowed_for_a_matching_wildcard_host` — the same credential against
      `https://acme.zendesk.com` succeeds.
- [ ] Third test: `shared_host_matcher_is_used_by_both_plugin_and_web` — a table test on the now-
      public matcher covering `*`, `*.zendesk.com`, exact match, case-insensitivity, and bracketed
      IPv6 literals (the trimming at `crates/flux-system/src/net.rs:410-414`).
- [ ] The check runs on the **guarded, post-redirect-resolution** URL, so a redirect cannot move a
      credential to an unscoped host.

## Progress
- (not started)

## Notes
- flux's plugin model scopes hosts in two places — per-endpoint `EndpointSpec.http_hosts`
  (`crates/flux-plugin-protocol/src/lib.rs:504`) and manifest-wide
  `PluginCapabilities.http_hosts` (`:565`) — with env-resolved endpoint hosts additionally admitted
  by `endpoint_allows_host` (`crates/flux-plugin/src/host.rs:603-628`). The `$auth` path needs only
  the per-credential list; do not import the endpoint machinery.
- Making `host_matches` public is a small L2 public-API addition. Flag it in the changelog.
```

---

## F-6 → `C-271-outbound-auth-query-scheme-injection.md`

```markdown
---
id: C-271
title: "`Query`-scheme injection for the outbound `$auth` marker (URL parameter, not a header)"
pillar: Core
status: ready
priority: 4
epic: outbound-auth-marker
design: docs/designs/outbound-auth-marker.md
note: "separate story because the injection point is before the SSRF guard, not in header assembly"
---

# `Query`-scheme injection for the outbound `$auth` marker

## Goal
Support the fourth `AuthScheme` variant — `Query { name }`
(`crates/flux-plugin-protocol/src/lib.rs:353`) — for older SaaS APIs that authenticate with
`?api_key=<token>`. This cannot ride along with the header work: it mutates the URL rather than the
header map, and the mutated URL must be the one the SSRF guard vets.

## Acceptance
- [ ] A `$auth` reference to a `Query`-scheme credential appends `?<name>=<secret>` to the request URL.
- [ ] The parameter is appended **before** `guard_url_scoped_pinned`
      (`crates/flux-web/src/http.rs:157`), so the guarded and pinned URL is the one actually sent.
      A design that appends afterwards is wrong and must fail review.
- [ ] `Query` emits **no** `Authorization` header — mirroring the plugin host, which handles `Query`
      before request construction (`crates/flux-plugin/src/host.rs:1229-1231`) and explicitly skips
      it in the header match (`:1247`).
- [ ] The composed URL is **not** registered with the redactor as a whole; the *token value* is, so
      a logged URL has the parameter value scrubbed rather than the whole URL replaced.
- [ ] Deny-by-default (C-268) and `http_hosts` scoping (C-270) apply identically to `Query`
      credentials — a test asserts each.
- [ ] The `$auth`-rejects-`Query` guard added by C-267 is removed.
- [ ] **A request-level `auth: [{credential: "…"}]` array is added to `http.request`**, because a
      `Query` credential has no header to hang a marker on and `http.request` has no `query`
      parameter (`crates/flux-web/src/http.rs:88-110`). This is what makes a *mechanism* (an AND
      group) that mixes a header scheme and a `Query` scheme expressible at all. Credentials named
      there are applied by scheme; a header-scheme credential named there is also valid (it just
      injects its header).
- [ ] Test: `query_and_header_auth_can_be_required_together` — one request carrying a `Query`
      credential in `auth` and a `Header` credential as a header marker sends both.
- [ ] Failing-first test: `query_scheme_auth_appends_parameter_to_the_guarded_url` — declare a
      `Query { name: "api_key" }` credential, issue a request, assert the sent URL carries
      `api_key=<secret>` and that no `Authorization` header is present. Before the change the call is
      refused by C-267's explicit `Query`-not-supported error.
- [ ] Second test: `query_scheme_parameter_is_present_in_the_url_the_ssrf_guard_saw` — proves the
      ordering, not just the outcome.

## Progress
- (not started)

## Notes
- Percent-encoding: use `url::Url::query_pairs_mut().append_pair(..)` as the plugin host does
  (`crates/flux-plugin/src/host.rs:1230`), so encoding behavior is identical across both paths.
- A URL that already carries the same parameter name should be an error, not a silent duplicate.
- **Deliberate two-spelling design, flagged for your review.** The header marker keeps the injection
  site visible at the call site, which is what makes generated Flux reviewable; the request-level
  `auth` array covers what a header cannot express. You may prefer to collapse both into the array
  and drop the marker — a cleaner protocol and a worse diff. flux-connectors will follow whichever
  you choose; say so on this story.
```

---

## F-7 → `C-272-plugin-host-basic-auth-not-redacted.md` (adjacent, non-blocking)

```markdown
---
id: C-272
title: Plugin host never registers the composed Basic credential with the redactor
pillar: Core
status: ready
priority: 3
epic: outbound-auth-marker
design: docs/designs/outbound-auth-marker.md
note: "pre-existing bug found while designing the outbound $auth seam — affects flux-plugin-zendesk today"
---

# Plugin host never registers the composed Basic credential with the redactor

## Goal
Close a live redaction gap in the **existing** plugin `http.do` path, independent of the outbound
`$auth` work. `resolve_purpose` registers the *raw* secret with the redactor
(`crates/flux-plugin/src/host.rs:414`, `:432`, `:443`), but the `Basic` arm then composes
`base64(user:secret)` at `:1253-1261` and never registers the result. Because `Redactor::redact`
matches by exact substring (`crates/flux-secret/src/lib.rs:204-215`), the value that actually travels
on the wire is not scrubbed. The full `register_secret` census in that file — `:414`, `:432`, `:443`,
`:908`, `:965`, `:1046`, `:1145`, `:1162` — contains no composed-credential registration.

## Acceptance
- [ ] The composed `Authorization` value is registered with the redactor for both `Bearer` and
      `Basic` before the request is sent.
- [ ] Failing-first test: `plugin_http_do_basic_auth_composed_header_is_redacted` — drive `http.do`
      with a `Basic` auth method through the existing test harness (the `register_secret`-capturing
      sink at `crates/flux-plugin/src/host.rs:4841` is the hook), then assert the captured composed
      value redacts. It fails today.
- [ ] No behavior change on the wire — this adds a registration, nothing else.

## Progress
- (not started)

## Notes
- Real-world impact: `plugins/zendesk` declares `AuthMethod::basic` (`plugins/zendesk/src/main.rs:135`),
  so this is the shipped Zendesk path, not a hypothetical.
- If C-266 lands first, the fix is one call at the extracted composition site and covers both hosts.
```

---

## F-8 → `C-273-connector-manifest-trust-decision.md` (decision, non-blocking)

```markdown
---
id: C-273
title: Decide whether flux accepts a file-based capability manifest, and with what integrity anchor
pillar: Core
status: backlog
epic: outbound-auth-marker
design: docs/designs/outbound-auth-marker.md
note: "maintainer decision, NOT a blocker — the outbound $auth seam carries credentials in operator config instead"
---

# Decide whether flux accepts a file-based capability manifest

## Goal
Settle a trust-model question the flux-connectors work surfaced, on flux's own terms and timeline.
flux-connectors originally proposed installing `~/.flux/connectors/<provider>.connector.toml`
declaring credentials and egress hosts. Verification showed this is a larger ask than it looks,
so the outbound `$auth` seam (C-267) was redesigned to **not depend on it** — credentials travel in
flux's operator config. This story exists so the question is answered deliberately rather than by
default.

## Why it is not a small change
- **flux has no file-based capability manifest today.** A `PluginManifest` is obtained by spawning
  the plugin binary and sending a `manifest` request frame
  (`crates/flux-plugin/src/host/loading.rs:186-188`). Capabilities are Rust literals inside the
  binary — e.g. `plugins/zendesk/src/main.rs:131`, `:135`.
- **What is on disk carries no capabilities.** `~/.flux/plugins`
  (`crates/flux-cli/src/execution.rs:544-546`) holds a `PluginDescriptor`
  (`crates/flux-plugin/src/host/loading.rs:693-726`): `program`, `args`, `version`, `sha256`,
  `git_url`, `git_commit` — transport and integrity only.
- **The integrity anchor has no analogue.** `spawn_verified`
  (`crates/flux-plugin/src/host/loading.rs:91-100`) refuses to spawn on sha256 drift (D-48). A TOML
  file granting credential access and egress hosts can be edited to widen the grant with no signal.
- **There is no "connector" concept to fold into.** `grep -rni connector crates/` returns only
  TUI tree-drawing connectors and hyper's Slack connector;
  `crates/flux-credentials/src/lib.rs:49` even records "flux doesn't use connectors".

## Acceptance
- [ ] A recorded decision, one of: (a) no file-based manifests — connector credentials stay in operator
      config indefinitely; (b) accept them with a named integrity anchor (signature, pack-index
      entry, or an install-time recorded hash mirroring `PluginDescriptor.sha256`); or (c) require
      connectors to ship as plugins.
- [ ] If (b): the anchor is specified concretely enough to implement, and the install path shows the
      operator what a manifest declares before granting it.
- [ ] The decision is written into flux's design doc and communicated back to `flux-connectors`
      (story C-16 there).

## Progress
- (not started)

## Notes
- **No behavioral change and therefore no failing-first test** — this is a decision story. It must
  not be closed by writing code.
- Counter-argument a maintainer will raise, and the answer flux-connectors gives: flux already ships
  SaaS plugins (`plugins/zendesk`, `jira`, `confluence`, `slack`, `opsgenie`, `huggingface`), so
  "just write a plugin" is a working answer today. flux-connectors' position is that a hand-written
  plugin per vendor does not scale to a generated catalogue and re-implements pagination and error
  mapping each time. Both positions are legitimate; that is why this is a decision, not a task.
```

---

## F-9 → `C-274-authmethod-basic-halves-from-env-plus-literal.md` (**critical path**)

```markdown
---
id: C-274
title: "`AuthMethod` must compose a Basic half from env + literal, not a bare env value"
pillar: Core
status: ready
priority: 3
epic: outbound-auth-marker
design: docs/designs/outbound-auth-marker.md
note: "without this, Basic ships but is unusable for Zendesk and Freshdesk — the two providers that need it"
---

# `AuthMethod` must compose a Basic half from env + literal

## Goal
Make `AuthScheme::Basic` usable for the API-token forms real vendors actually use. Today
`AuthMethod.user_env` (`crates/flux-plugin-protocol/src/lib.rs:436`) resolves an env var
**verbatim** into the user half of `base64(user:secret)`. Neither provider that needs Basic fits
that shape:

- **Zendesk:** `base64("<email>/token" : "<api_token>")` — the user half is an env value **plus a
  literal `/token` suffix**.
- **Freshdesk:** `base64("<api_key>" : "X")` — the API key is the **user** half and the password
  half is the **literal `X`**.

The workaround — tell operators to bake `/token` into `ZENDESK_USER`, or set a variable to the
literal `X` — is the pre-composed-credential mistake one level down: it stores a value that is not
the thing it is named after, and nothing can validate it. It is explicitly rejected by this design.

## Acceptance
- [ ] `AuthMethod` can describe each Basic half as a **template over declared env**, e.g.
      `user: "{ZENDESK_EMAIL}/token"`, `secret: "{ZENDESK_API_TOKEN}"` and
      `user: "{FRESHDESK_API_KEY}"`, `secret: "X"`. Composition happens host-side; the operator sets
      only the real credential.
- [ ] The mechanism mirrors the existing precedent rather than inventing a second one:
      `EndpointSpec.template` (`crates/flux-plugin-protocol/src/lib.rs:512-519`) already composes a
      URL host-side from declared placeholders. Reuse its substitution semantics.
- [ ] A placeholder naming an env key that is **not declared** on the method is refused — a template
      must not become a way to read arbitrary environment.
- [ ] Existing `user_env` manifests keep working unchanged (serde defaults; `AuthMethod::basic`
      (`:457`) still compiles and behaves identically).
- [ ] Failing-first test: `basic_user_half_composes_env_value_with_literal_suffix` — a method
      declaring `user: "{EMAIL}/token"` with `EMAIL=me@acme.com` and secret `tok` produces exactly
      `Basic bWVAYWNtZS5jb20vdG9rZW46dG9r`. It fails today because there is no template field.
- [ ] Second test: `basic_password_half_can_be_a_literal` — freshdesk's `X` form composes correctly
      and the literal `X` is never read from the environment.
- [ ] Third test: `basic_template_placeholder_for_undeclared_env_key_is_refused`.
- [ ] **A Basic half can be declared secret-bearing, and is then redactor-registered.** Today
      `user_env` is documented as "config (not a gated secret)"
      (`crates/flux-plugin-protocol/src/lib.rs:433-435`) and `resolve_user`
      (`crates/flux-plugin/src/host.rs:630-640`) accordingly **never calls `register_secret`**. For
      **freshdesk the user half *is* the API key**, so mapping it onto today's model routes a secret
      through the non-secret config path and leaves it unregistered with the redactor.
- [ ] Test: `secret_bearing_basic_user_half_is_registered_with_the_redactor` — a method declaring
      its user half secret-bearing has that value redacted. It fails today.

## Progress
- (not started)

## Notes
- **This is critical path, not polish.** Two of the three providers flux-connectors plans to ship
  (zendesk, freshdesk) are blocked on it *after* the `$auth` marker lands. Sequencing it as "later"
  means `Basic` is implemented and unusable.
- **Framed another way:** flux's `Basic` variant is documented as
  `base64(<user_env>:<secret>)`, i.e. a *join*. It is really "join of a bare env value and a secret",
  which is a narrower thing. This story makes the documented claim true. The secret-half point above
  is the security-relevant half of that gap and should not be dropped if the story is trimmed.
- The composed value must still be redactor-registered per C-269 — the composition changes, the
  redaction requirement does not.
- The third provider, babelforce, is `Bearer` and unaffected by this story.
```

---

## F-10 → `C-275-oauth2spec-jwt-and-confidential-client-gaps.md` (contingency, non-blocking)

```markdown
---
id: C-275
title: "`OAuth2Spec` cannot express a confidential client or a self-signed JWT assertion"
pillar: Core
status: backlog
epic: outbound-auth-marker
design: docs/designs/outbound-auth-marker.md
note: "filed early on purpose — cheaper to learn now than after three connectors ship"
---

# `OAuth2Spec` cannot express a confidential client or a self-signed JWT assertion

## Goal
Record two concrete gaps in flux's OAuth2 model, found while designing the outbound `$auth` seam for
a provider (babelforce) that authenticates with an SSO-issued Bearer today and plans to add JWT.
Neither blocks the `$auth` seam. Both block *that provider*, and which one bites depends on an
answer we do not have yet.

## What is already fine
A JWT is carried as `Authorization: Bearer <jwt>`, so `AuthScheme::Bearer`
(`crates/flux-plugin-protocol/src/lib.rs:347`) expresses the header shape with **zero** change. flux
also already decodes a JWT's `exp` claim (`jwt_expiry_ms`, `crates/flux-credentials/src/lib.rs:126-127`)
and uses it as the expiry when a token response omits `expires_in` (`:431`), with staleness and
refresh handled by `resolve_stored_bearer_with_client_factory` (`:648-670`). **If the JWT is issued
by a token endpoint, nothing needs to change at all.**

## Gap 1 — no client secret
`OAuth2Spec` (`crates/flux-plugin-protocol/src/lib.rs:390-414`) carries `client_id` (`:404`) and no
secret. `grep client_secret` across `crates/` finds it only in flux-providers' Bedrock code and in
flux-auth's **inbound** introspection (`crates/flux-auth/src/introspect.rs:48`) — a different
subsystem. The plugin OAuth path is public-client/PKCE shaped, so a `client_credentials`
(`OAuthGrant::ClientCredentials`, `:369`) grant against a **confidential** client cannot be declared.

## Gap 2 — no JWT-assertion grant
If a client **signs its own** JWT (RFC 7523 `jwt-bearer`, or a self-signed JWT used directly — the
Google/Apple service-account pattern), none of the four `OAuthGrant` variants (`:361-370`) expresses
it, and `OAuth2Spec` has nowhere to declare a signing key, algorithm, issuer, audience, subject, or
claim template. flux also has no JWT **signing** dependency today (it only decodes).

## Acceptance
- [ ] `OAuth2Spec` gains `client_secret_env: Vec<String>` — env-keyed, never a literal, resolved
      host-side like `user_env`. Failing-first test:
      `client_credentials_grant_sends_secret_resolved_from_declared_env`.
- [ ] A decision is recorded on Gap 2: add `OAuthGrant::JwtBearer` + a
      `JwtAssertionSpec { key_env, algorithm, issuer, audience, subject, ttl_secs }` (which brings a
      signing dependency), or decline and require such providers to be plugins.
- [ ] If Gap 2 is accepted, it is split into its own implementation story with its own
      failing-first test — this story does not implement it.

## Progress
- (not started)

## Notes
- **Open question owned by flux-connectors, not flux:** ask babelforce (a) whether their SSO client
  is public or confidential, and (b) whether their planned JWT is token-endpoint-issued or
  self-signed. (a) determines whether Gap 1 blocks them; (b) determines whether Gap 2 is ever
  needed. Neither answer is available at the time of writing.
- Gap 1 is cheap and likely to bite first. Gap 2 is a real protocol addition and may never be
  needed — hence `status: backlog` rather than `ready`.
```

---

## F-11 → `C-276-credential-resolver-usable-outside-the-plugin-registry.md`

```markdown
---
id: C-276
title: Make OAuth2 credential resolution usable outside the plugin registry
pillar: Core
status: backlog
epic: outbound-auth-marker
design: docs/designs/outbound-auth-marker.md
note: "the token machinery is already reusable; the store-key namespace and the login CLI are what are plugin-bound"
---

# Make OAuth2 credential resolution usable outside the plugin registry

> Naming: flux's `AuthMethod.purpose` field and `resolve_purpose` function are quoted verbatim
> throughout and are **not** renamed by this story. "Credential" is the caller-facing word the
> outbound `$auth` marker uses; it resolves to flux's `purpose`.

## Goal
Let a non-plugin caller (the outbound `$auth` seam) use flux's existing OAuth2 token machinery, so
**effectful acquisition runs in the host and is never emitted into generated Flux** — the property
that keeps a token out of a bound Flux symbol and therefore out of model-visible state. The good
news from the audit is that the hard part is already reusable; only the plumbing is plugin-shaped.

## Already reusable — no change needed
Free functions in `flux-credentials` (L1) with no plugin concept anywhere:
`resolve_stored_bearer_with_client_factory` (`crates/flux-credentials/src/lib.rs:648-670`) with its
store-first / stale-check / refresh loop and injectable transport; `CredentialStore` +
`FileCredentialStore` (`:700-720`); `OAuthToken` (`:87-96`, `Debug` redacts at `:98-102`); the
refresh buffer (`:661-664`); and `jwt_expiry_ms` (`:126-127`), already used at `:431`.

## What is plugin-bound
1. **The store-key namespace is `plugin:<caller>:<purpose>`** — `crates/flux-plugin/src/host.rs:381`,
   `:426`, written by the CLI at `crates/flux-cli/src/auth_cmd.rs:171` and `:385`.
2. **`resolve_purpose` is a private method on `SystemHostCaps`**
   (`crates/flux-plugin/src/host.rs:358`) reading `self.auth` / `self.grants.secrets` /
   `self.caller` / `self.secret_sink` / `self.cred_store`.
3. **`flux auth login` and `flux auth set` require an installed plugin** — both begin with
   `plugins_dir()` + `load_descriptor` + `spawn_and_load_manifest`
   (`crates/flux-cli/src/auth_cmd.rs:136-140`, `:367-371`). There is no path through that code that
   does not spawn a binary.
4. **`OAuth2Spec.endpoint` names a plugin-manifest `EndpointSpec`**
   (`crates/flux-plugin-protocol/src/lib.rs:391-394`), egress-gated against the plugin's hosts
   (`crates/flux-plugin/src/host.rs:380`).

## Acceptance
- [ ] A credential resolver parameterized over *(auth methods, host allowlist, store-key namespace,
      secret sink, credential store)* rather than over `SystemHostCaps`. `flux-plugin` calls it with
      `plugin:` and its manifest; a connector caller calls it with `connector:` and its own host
      list (C-270).
- [ ] **Namespace separation is enforced, not conventional.** A connector credential can never read a
      credential stored under a `plugin:` key, and vice versa.
- [ ] Failing-first test: `connector_namespace_cannot_read_a_plugin_stored_credential` — store a
      token under `plugin:acme:api_token`, resolve the same name in the connector namespace,
      assert it is not found. It cannot even be written today, since there is no second namespace.
- [ ] Second test: `resolver_is_callable_without_a_plugin_descriptor` — resolve an OAuth2 credential
      with no `~/.flux/plugins` entry present at all.
- [ ] A login path exists that does not require a plugin descriptor. Whether that is
      `flux auth login --connector <name>` or a generalized subject argument is this story's design
      choice; the constraint is that no binary is spawned.
- [ ] The existing plugin behavior is unchanged — same keys, same precedence, same tests.

## Progress
- (not started)

## Notes
- **Not needed for the first cut.** The three in-scope providers (zendesk, freshdesk, babelforce)
  need only the *pure* acquisitions — `static` and `basic_join` — on their non-OAuth paths. This
  story is what unblocks effectful acquisition later, and it is filed now so the shape is known
  before someone re-implements token refresh in a connector.
- Item 4 above is where this story meets C-270: a connector's `http_hosts` is what egress-gates its
  token endpoint.
- If flux prefers connectors to remain env-only forever, say so on this story and close it — that is
  a legitimate answer, and it bounds flux-connectors' roadmap usefully.
```
