# Design: the `$auth` seam for `http.request`

**Status:** reviewed — verified against flux source · **Pillar:** Bridge · **Stories:**
[C-16](../stories/C-16-design-auth-seam.md) · **Handoff:**
[auth-seam-flux-stories.md](auth-seam-flux-stories.md) · **Companion:**
[unified-auth.md](unified-auth.md) (the connector-side credential model this seam must agree with —
see §9, which reports **three preset round-trip failures**)

> This design describes a change to **`../flux`**, not to this repository. It is recorded here because
> flux-connectors is the consumer that motivates it. The implementation stories belong on flux's own
> board and must ship in a flux release before this repo's milestone 1 can go green.

### Provenance of the citations in this document

Every `path:line` below was read in `/home/timo/projects/flux`. The reading began at commit
`bcfab0ad` plus that checkout's uncommitted working-tree changes; **flux then cut `v0.38.0` mid-review**,
committing exactly those changes. **Every anchor was therefore re-verified afterwards against
`v0.38.0`, and the line numbers below describe that tree.** Five moved in the cut and are corrected
here: `resolve_user` `:630`→`:631`, `resolve_auth` `:644`→`:645`, the
"no auth method declared for purpose" error `:657`→`:656`, `host_matches` `:1840`→`:1898` (all
`crates/flux-plugin/src/host.rs`), and `PluginHost::manifest` `:186`→`:187`
(`crates/flux-plugin/src/host/loading.rs`). Every other citation was unchanged.

Symbol names are stable and line numbers are not; re-grep by symbol rather than trusting a number if
it does not land. The seam is still unfiled on flux's board after the cut — see the C-16 story's
Notes.

### Naming — do not reuse "auth seam" on flux's board

flux already has a **`request-auth-seam`**: `docs/designs/request-auth-seam.md`, stories D-64 and
D-68, both `status: done`. That is *inbound* per-request bearer→principal resolution for
flux-server. It is unrelated to this work. Stories filed on flux's board for the change below must
say **"outbound `$auth` header marker"** or reviewers will conflate the two.

## Why

A generated connector calls the vendor API through flux's `http.request` op
(`crates/flux-web/src/http.rs`). Credentials reach that op today through one mechanism: a header
value may be the marker `{"$secret": "ENV_NAME"}`, which `resolve_header_value`
(`crates/flux-web/src/http.rs:234`) replaces with the environment variable's value.

**Confirmed — the marker is a whole-value replacement, headers only, with no prefix or encode
capability.**

- *Whole value:* `as_secret_ref` (`crates/flux-web/src/http.rs:275`) matches only an object of
  **exactly one** key named `$secret` whose value is a string; anything else falls through to
  `Value::String` passthrough or a caller error (`:251-256`).
- *Headers only:* the sole call site is `crates/flux-web/src/http.rs:171`, inside the
  `params["headers"]` loop. The URL, query string and body never see it.
- *No composition:* the function returns the raw env value (`:250`). There is no prefixing,
  concatenation, or encoding anywhere on the path.

Flux-Lang cannot close this gap from the language side either — **confirmed**: the `expr` built-in
whitelist is `round/abs/min/max/len/lower/upper/trim/replace/repeat/reverse/contains/concat/sum/any/
all/has/join/split/first/last` (`crates/flux-lang/src/expr.rs:136-139`). No `base64`. And string
interpolation over a secret would defeat redaction by materializing the token into a bound symbol.

The consequences for connectors:

| Vendor auth shape | Example | Works today? |
|---|---|---|
| Raw value in a custom header | Anthropic `x-api-key: <key>`, GitLab `PRIVATE-TOKEN: <tok>` | **yes** |
| `Authorization: Bearer <token>` | OpenAI, OpenRouter, most modern SaaS | no — needs the `Bearer ` prefix |
| `Authorization: Basic base64(user:token)` | Zendesk, Freshdesk | no — needs base64 of a joined pair |
| `?api_key=<token>` query parameter | older SaaS APIs | no — the marker is headers-only |

### Every provider we plan to ship is blocked on this seam

The "yes" row above is real but **no provider on our roadmap uses it**. The three concrete providers
are:

| Provider | Scheme | Shape | Executable today? |
|---|---|---|---|
| **zendesk** | `Basic` | `base64("<email>/token" : "<api_token>")` — the `/token` suffix on the *user* half is Zendesk's API-token form | **no** |
| **freshdesk** | `Basic` | `base64("<api_key>" : "X")` — the API key is the *user* half, the password is the literal `X` | **no** |
| **babelforce** | `Bearer` | SSO-issued token, `Authorization: Bearer <token>`; **JWT planned** (see §8) | **no** |

So the honest statement is the strong one: **all three providers are blocked on this seam for live
calls.** Bearer needs a `Bearer ` prefix; Basic needs a base64-joined pair; the whole-value
`{"$secret": "ENV"}` marker produces neither. There is no provider we can ship end-to-end without
this change, and no partial-credit path where some providers work and others wait.

Two things follow for the connector IR (this repo's C-2/C-5, not flux's problem — noted so codegen
gets them right):

- **Basic is not "username and password".** Zendesk puts a literal `/token` suffix inside the user
  half; freshdesk puts a literal `X` in the password half. The IR must carry both halves as
  *composable* values, not assume `user_env` holds a bare username. flux's `AuthMethod.user_env`
  (`crates/flux-plugin-protocol/src/lib.rs:436`) resolves an env var verbatim, so the `/token`
  suffix and the literal `X` must come from the connector's own declaration, not from the operator
  being told to type them into an env var. This is a **gap** — see §7.5.

> **Withdrawn:** an earlier reading of this design treated babelforce as already executable via its
> `X-Auth-Access-Id` + `X-Auth-Access-Token` raw apiKey header pair. **That is retracted.** Those
> headers are being deprecated and scrubbed from the babelforce API. They must not be modeled in the
> IR, emitted by codegen, or cited anywhere as a working path. babelforce is Bearer-only, and
> therefore blocked like the other two.

The rejected workaround was to have operators store a **pre-composed** header value
(`ZENDESK_AUTH="Basic dXNlcjp0b2s="`). It unblocks everything with zero flux changes, but it pushes
credential assembly onto the operator, stores a value that is neither the username nor the token in
a form anything else can validate, and gives the host no idea what scheme it is applying. We chose
the correct end state instead.

### A second `$secret` definition exists at L0

`flux-lang` owns its own copy of the marker: `SECRET_KEY = "$secret"`
(`crates/flux-lang/src/program.rs:295`) and `as_secret_ref` (`:308`). flux-web deliberately inlines
the predicate rather than take an L0 language dependency for one function — the comment at
`crates/flux-web/src/http.rs:273-274` says so explicitly. **Any `$auth` marker therefore has two
possible homes, and the stories must pick one deliberately.** The recommendation is: define
`$auth` in flux-web only, because unlike `$secret` it carries no language-level meaning (flux-lang's
copy exists for settings-block secret references, not for header values).

## Approach

### 1. A second marker: `{"$auth": {"credential": "<name>"}}`

Extend `resolve_header_value` to recognize a second marker alongside `$secret`. Where `$secret` names
an *environment variable*, `$auth` names a **credential** — a declared credential slot whose scheme
the host knows.

> **Naming.** Our word is **credential**, not "purpose". An AND-group of credentials is a
> *mechanism*; alternatives are alternative mechanisms. Where this design maps our marker onto flux's
> struct, **our `credential` name resolves to flux's `AuthMethod.purpose` field**
> (`crates/flux-plugin-protocol/src/lib.rs:424`). We are **not** proposing to rename flux's field —
> keeping that mapping visible is deliberate. Flux's own identifiers (`AuthMethod.purpose`,
> `auth_purpose`, `resolve_purpose`, the `plugin:<name>:<purpose>` store key) are quoted verbatim
> throughout and are never renamed.

Object keys survive this fine in Flux text — **confirmed**: `fmt_obj_key`
(`crates/flux-lang/src/format.rs:479-485`) emits a JSON-quoted key when `is_ident_key` is false, and
the parser recovers it losslessly (`crates/flux-lang/src/parse.rs:1861` round-trips a `"$secret"`
key). So codegen can emit:

```flux
http.request({url: $url, method: "GET", headers: {Authorization: {"$auth": {credential: "zendesk.api_token"}}}})
```

### 2. Reuse `flux_plugin_protocol::AuthScheme` — do not invent a second vocabulary

**Confirmed exactly as drafted.** flux models these four shapes for plugins at
`crates/flux-plugin-protocol/src/lib.rs:344-354`:

```rust
pub enum AuthScheme {
    #[default]
    Bearer,                    // Authorization: Bearer <secret>            (:347)
    Basic,                     // Authorization: Basic base64(<user>:<secret>) (:349)
    Header { name: String },   // <name>: <secret>                          (:351)
    Query  { name: String },   // ?<name>=<secret>                          (:353)
}
```

`AuthMethod` (`:422`) carries `purpose` (`:424`), `env` (`:427`), `description` (`:429`),
`scheme` (`:432`), `user_env` (`:436`) and an optional `oauth2` block (`:442`) — the draft's list
plus `description`. Convenience constructors `AuthMethod::bearer` (`:447`), `::basic` (`:457`) and a
header form (`:467`) already exist.

**Layering — the draft was right about the layers but wrong about what follows from them.**

- The layer map confirms the claim: `flux-plugin-protocol => 0`
  (`crates/flux-codegate/src/lib.rs:36`) and `flux-web => 5` (`:51`). A downward dependency is legal.
- **But flux-web needs no new dependency at all.** It already declares `flux-plugin` (L4) in
  `crates/flux-web/Cargo.toml` — for the `EgressAudit` seam, used at
  `crates/flux-web/src/http.rs:41` — and `crates/flux-plugin/src/lib.rs:35` is
  `pub use flux_plugin_protocol::*;`. So `flux_plugin::AuthScheme` and `flux_plugin::AuthMethod` are
  **already in scope in flux-web today**. Taking the direct L0 dep is still the cleaner spelling and
  is legal; it is a style choice, not a blocker.

**Correction — the *types* are reusable; the *injection logic* is not.** The draft claimed the
connector path could use "the same injection logic the plugin host uses". It cannot, as written:

- `AuthInjection` is a **private** enum — `crates/flux-plugin/src/host.rs:755`, no `pub`.
- `resolve_auth` is a private method on `SystemHostCaps` (`crates/flux-plugin/src/host.rs:645`),
  reached through the plugin's own manifest state (`self.auth`, `self.grants`).
- The actual composition is inline inside one arm of a very large `match command` in
  `HostCapabilities::handle`: Bearer at `crates/flux-plugin/src/host.rs:1248-1252`, the Basic
  base64 at `:1253-1261`, custom Header at `:1262-1264`, and the `Query` URL mutation earlier at
  `:1229-1231`.

Reusing it means **extracting a pure, public
`fn apply_scheme(scheme: &AuthScheme, secret: &str, user: Option<&str>) -> Injection` into
`flux-plugin-protocol` (L0)** and having both flux-plugin and flux-web call it. That extraction is a
strict improvement — it removes a duplicated base64 composition before a second one is written — and
it is filed as its own story (**F-1 / C-266** in the handoff) rather than smuggled into the marker
work.

The payoff of the shared vocabulary stands: OAuth2 connectors come almost free — `OAuth2Spec` and the
`flux auth login` grants already exist for plugins.

### 3. Where credentials come from — the draft's premise was wrong

The draft asserted that flux-connectors emits `<provider>.connector.toml`, installed to
`~/.flux/connectors/`, which flux loads and resolves into `WebOptions`. Verification changed the
picture materially. See §"Risks & open questions" below for the resolved answer; the short version:

> **flux has no file-based capability manifest of any kind.** Every capability grant in flux today
> is declared in Rust inside a plugin binary and delivered over the wire from a spawned subprocess.
> A `~/.flux/connectors/*.toml` that grants credential access would be flux's **first** capability
> grant with no binary-hash anchor. That is a trust-model change, not a new file format, and it is
> the thing flux's maintainers will actually push back on.

**The design therefore no longer depends on connector manifests.** The credential map reaches flux-web
through **flux's own operator config**, exactly as `allowed_secrets` does today (§6). A connector
manifest becomes an optional later convenience, tracked separately, not a precondition for the seam.

If a manifest is later adopted, it should mirror the plugin schema faithfully — and note that flux
scopes hosts in **two** places, not one as the draft's example showed: per-endpoint
`EndpointSpec.http_hosts` (`crates/flux-plugin-protocol/src/lib.rs:504`) *and* manifest-wide
`PluginCapabilities.http_hosts` (`:565`), with env-resolved endpoint hosts additionally admitted by
`endpoint_allows_host` (`crates/flux-plugin/src/host.rs:603-628`).

### 4. Fail closed, and redact

Both behaviors have precedent in the `$secret` path and must be matched exactly:

- **Deny-by-default.** A credential absent from the resolved map is refused *before any value is read*,
  with the same shape of error `allowed_secrets` produces today
  (`crates/flux-web/src/http.rs:236-242`, the C-76 precedent — flux story
  `docs/stories/C-76-http-request-secret-exfil.md`). The existing refusal test to mirror is
  `secret_ref_to_non_allowlisted_env_var_is_refused` (`crates/flux-web/src/http.rs:571`).

- **Redaction — confirmed, and the requirement is stronger than the draft stated.** The call is
  `ctx.redactor.add_secret(resolved.clone())` at `crates/flux-web/src/http.rs:248`.
  `Redactor::add_secret` (`crates/flux-secret/src/lib.rs:195-201`) stores the value; `redact`
  (`:204-215`) replaces registered values by **exact substring**, then applies prefix-shaped token
  heuristics (`redact_patterns`, `:232`).

  It follows that **registering the raw token provably does not redact a Basic header**:
  `base64("user:token")` shares no substring with `token`, and `Basic dXNlcjp0b2s=` starts with no
  entry in `SECRET_PREFIXES`. Registering the *composed* value is not a nicety — it is the only
  thing that works. Two further facts the stories must respect:
  - `add_secret` **silently drops any value shorter than 6 characters** (`:198`), so a short token
    is never redacted; the composed Basic value comfortably clears the bar, a bare short API key
    does not.
  - Register the composed value **in addition to**, not instead of, the raw secret — the raw token
    can still appear in an upstream error body.

  **Adjacent gap found in flux (reported, not fixed here):** the plugin host's own Basic path
  composes the base64 at `crates/flux-plugin/src/host.rs:1253-1261` and never registers it. The
  `register_secret` calls in that file are at `:414`, `:432`, `:443` (raw secret, inside
  `resolve_purpose`), `:908`, `:965`, `:1046`, `:1145`, `:1162` — none is the composed `Basic`
  header. So `flux-plugin-zendesk`'s Basic credential is redacted only in raw form today. Fixing
  that is a flux story in its own right and is included in the handoff (**F-7 / C-272**).

- **Host scoping.** The request URL must be checked against the connector's declared hosts before
  dispatch, so a connector cannot send its Zendesk credential to an attacker-chosen host. This is
  the single most important control in this design.

  **New constraint found:** flux-web has **no public host-pattern matcher**. `host_matches` exists
  twice and is private both times — `crates/flux-plugin/src/host.rs:1898` and
  `crates/flux-system/src/net.rs:409`. Writing a third copy inside flux-web is precisely the
  "second URL guard" flux's invariants forbid. The `http_hosts` story must therefore **make the
  flux-system copy public and have flux-plugin call it**, then use it from flux-web. flux-web
  already depends on flux-system (`crates/flux-web/Cargo.toml`), so no layering question arises.

### 5. `Query` needs a second injection point

**Confirmed by the plugin host's own shape.** The three header schemes resolve inside header
assembly; `Query` does not — it mutates the URL, and in flux-plugin it is deliberately handled
*before* the request is built (`crates/flux-plugin/src/host.rs:1226-1231`) and then explicitly
skipped in the header match (`:1247`). In flux-web the equivalent point is *before*
`guard_url_scoped_pinned` (`crates/flux-web/src/http.rs:157`), because the appended parameter must
be part of the URL that the SSRF guard vets and pins. That ordering constraint is the reason this is
genuinely a separate change to `HttpRequestTool::execute` and its own story on flux's board.

### 6. Carrying the credential map: `WebOptions`

**Confirmed as the right place.** `WebOptions` is at `crates/flux-web/src/lib.rs:75`, and
`allowed_secrets: Option<Vec<String>>` at `:97` is the exact precedent — a security-boundary
allowlist, `None` meaning "fall back to an env var", `Some(vec![])` an explicit deny-all.
`HttpRequestTool` resolves it once at construction (field at `crates/flux-web/src/http.rs:46`,
`new` at `:50`).

Two mechanical facts the story must handle:

- `WebOptions` derives `Default` (`crates/flux-web/src/lib.rs:74`), and four of the five
  construction sites use it: `crates/flux-cli/src/catalog_coherence.rs:149`,
  `crates/flux-lsp/src/catalog.rs:57`, `crates/flux-cli/src/plugin_cmd.rs:1389`,
  `crates/flux-cli/tests/website_contract.rs:333`. Those are unaffected by a new field.
- The **real wiring site** is the exhaustive struct literal at
  `crates/flux-cli/src/execution.rs:1529-1541`, which lists every field including
  `allowed_secrets: None`. A new field must be added there, and that is also where the operator
  config is read.

So the credential map is `auth_credentials: Option<Vec<AuthMethod>>` (or a
`BTreeMap<String, AuthMethod>` keyed by our `credential` name, which is flux's `AuthMethod.purpose`)
alongside `allowed_secrets`, populated in `execution.rs` from flux's config — same operator, same
trust anchor, no new artifact kind.

### 7. Multi-scheme: mechanisms, and what a credential must be able to describe

A provider may support several schemes, and one operation may require zero, one, or several of them.
This is a **first-class requirement**, not an edge case, and the design must answer it explicitly.

The model is OpenAPI's, adopted verbatim so ingest is a translation rather than an interpretation:
`security` is a **list of requirement objects**; within one object **all** listed schemes must be
satisfied (**AND**); across objects **any one** suffices (**OR**); `security: []` means the
operation needs **no** auth, and that must stay distinguishable from *unspecified* (which inherits
the document-level default).

In this repo's vocabulary an **AND-group is a mechanism**, and its members are **credentials**. That
is what makes babelforce's two-header case read correctly — `credentials = ["access_id",
"access_token"]` is two credentials in *one* mechanism, not two mechanisms — and it makes the OR case
read as *alternative mechanisms*, each composed of credentials. "Requirement object" below is
OpenAPI's word for the same thing as our *mechanism*.

#### 7.1 AND — confirmed: one `$auth` marker per header works, with two stated limits

**Confirmed against the code.** `resolve_header_value` is called once per header inside the
`params["headers"]` loop (`crates/flux-web/src/http.rs:169-181`). Each call is independent: there is
no shared state, no "already resolved a credential" flag, and nothing that assumes at most one
marker per request. So this needs **no additional design at all**:

```flux
http.request({url: $url, method: "GET", headers: {
  "X-Api-Key":    {"$auth": {credential: "acme.api_key"}},
  "X-Account-Id": {"$auth": {credential: "acme.account"}}
}})
```

Two limits must be written down rather than discovered later:

1. **Two schemes cannot both target `Authorization`.** A Flux object cannot carry a duplicate key,
   and duplicate `Authorization` headers are not meaningfully interpretable by servers anyway. This
   is a permanent limitation and an acceptable one — no real API demands two `Authorization`
   schemes. Codegen must **reject** such a requirement set at build time with a clear error rather
   than silently emitting one of them.
2. **A `Query` scheme is not a header and cannot ride the header marker.** An AND set that mixes,
   say, a `Header` scheme and a `Query` scheme has no complete spelling in the header marker alone.
   `http.request` has no `query` parameter to hang a marker on (its params are `url`, `method`,
   `headers`, `body`, `timeout` — `crates/flux-web/src/http.rs:88-110`), so this needs a
   **request-level** spelling: an `auth: [{credential: "…"}]` array on the request, applied by the
   host according to each credential's declared scheme. That lands with the `Query` story
   (**F-6 / C-271**),
   not with the header marker.

   This is deliberately **two spellings**, and the reason is worth stating: the header marker keeps
   the injection site visible at the call site, which is what makes generated Flux reviewable — you
   can see *which header* carries the credential. A reviewer may reasonably prefer collapsing both
   into the request-level `auth` array and dropping the marker; that would be a cleaner protocol and
   a worse diff. The recommendation is to keep both, and the decision belongs to flux.

#### 7.2 OR — resolved at build time, so flux never sees it

The host must **not** choose among alternatives at runtime. Choosing would mean trying credentials
until one is accepted, and a failed attempt has already put a credential on the wire. So:

> **flux-connectors' codegen selects exactly one requirement object at build time.** The emitted
> Flux contains that object's markers and nothing else. OR is a compile-time concept in this repo;
> **flux needs no OR support whatsoever.**

That is a real simplification and none of the flux stories should carry alternative-selection logic.

#### 7.3 Does anything need to record which alternative was selected? Yes — here, not in flux

flux's credential map needs only the **union** of credentials the operator declared; it never needs to know
which alternative codegen picked, because by the time a request is dispatched the choice is already
baked into the emitted Flux.

But the choice must be recorded **in this repo**, for three reasons that map onto existing stories:

- **Reviewability** — the selection is a security-relevant decision (it determines which credential
  a generated op carries). It belongs in the provenance lockfile (C-7) so it is diffable.
- **Diff (C-13)** — a build that silently switches from an OAuth alternative to an API-key
  alternative must show up as a diff, not as an identical-looking regeneration.
- **Drift (C-14)** — if the vendor edits its `security` block and removes the alternative we chose,
  the drift check must be able to notice. It can only do that if the chosen alternative was recorded.

Recommended record: the selected mechanism, the credentials it resolved to, and *why* it was
selected (the selection rule, e.g. "first object all of whose schemes are supported").

#### 7.4 Zero auth: `[]` must not collapse into "unspecified"

The IR must distinguish them, because they mean opposite things:

- `Some(vec![])` — explicitly **no** auth. Emit **no** `$auth` marker. This is correct and silent.
- `None` — unspecified; inherit the document-level default.
- `None` **and no document default** — the operation is unauthenticated *by omission*. That is
  almost always a spec bug, and the build should **warn**, not silently emit an unauthenticated
  call.

So the IR field is `Option<Vec<RequirementSet>>`, never `Vec<RequirementSet>` — a bare `Vec` makes
`[]` and "absent" the same value and loses the distinction irrecoverably. (This is a flux-connectors
IR concern for C-2/C-5; recorded here because getting it wrong is invisible until a connector ships
an unauthenticated op.)

#### 7.5 Gap: `Basic` halves are literals-plus-secrets, not two env vars

Neither zendesk nor freshdesk fits `AuthMethod.user_env` as it stands. `user_env`
(`crates/flux-plugin-protocol/src/lib.rs:436`) resolves an env var **verbatim** into the user half.
But:

- zendesk needs `base64("<email>/token" : "<api_token>")` — the user half is *env value + a literal
  suffix*.
- freshdesk needs `base64("<api_key>" : "X")` — the user half is the secret and the password half is
  a *literal*.

Telling operators to bake `/token` into `ZENDESK_USER` or to set a variable to the literal `X` is
exactly the pre-composed-credential mistake this design rejects, one level down: it stores a value
that is not the thing it is named after, and nothing can validate it.

**Required:** the auth method must be able to describe each half as `env-key + optional literal
suffix`, or more generally as a small template over declared env — e.g.
`user: "{ZENDESK_EMAIL}/token"`, `secret: "{ZENDESK_API_TOKEN}"` and
`user: "{FRESHDESK_API_KEY}"`, `secret: "X"`. flux already has precedent for exactly this shape:
`EndpointSpec.template` (`crates/flux-plugin-protocol/src/lib.rs:512-519`) composes a URL host-side
from declared config placeholders. The same mechanism applied to Basic halves solves both providers
and keeps composition host-side.

This is a **change to `AuthMethod`**, so it is a flux story — filed as **F-9 / C-274** — and it is
on the critical path for zendesk and freshdesk, i.e. for two of our three providers. Without it,
`Basic` support technically exists and is still unusable for the providers we ship.

### 8. Forward compatibility: JWT (babelforce)

Babelforce is Bearer today and intends to add JWT. The question is whether flux's existing types
absorb that without reshaping.

**The header shape is a non-issue — confirmed.** A JWT is carried as `Authorization: Bearer <jwt>`.
`AuthScheme::Bearer` (`crates/flux-plugin-protocol/src/lib.rs:347`) expresses it with **zero**
change. The interesting question is entirely about *acquisition and refresh*.

**What flux already has, and it is more than expected:**

- `OAuth2Spec` (`crates/flux-plugin-protocol/src/lib.rs:390-414`): `endpoint`, `authorize_path`,
  `token_path`, `client_id`, `scopes`, `grants`, `redirect`.
- `OAuthGrant` (`:361-370`): `AuthorizationCode`, `Password`, `RefreshToken`, `ClientCredentials`.
- `OAuthToken` (`crates/flux-credentials/src/lib.rs:87-96`): `access`, `refresh`, `expires_at_ms`.
- **flux already understands JWTs specifically**: `jwt_expiry_ms` decodes the `exp` claim
  (`crates/flux-credentials/src/lib.rs:126-127`) and is used as the expiry when a token response
  omits `expires_in` (`:431`). Staleness and refresh-with-buffer are handled by
  `resolve_stored_bearer_with_client_factory` (`:648-670`).

**Verdict, split by which kind of JWT babelforce means — this is the question to ask them now:**

- **(a) The SSO/token endpoint issues us a JWT.** Standard authorization-code or client-credentials
  flow, JWT happens to be the token format. **No protocol change at all.** Declare an `OAuth2Spec`,
  and flux's existing store already reads the `exp` claim to schedule refresh. This also covers
  babelforce's *current* SSO-issued Bearer.
- **(b) We sign a JWT ourselves** (RFC 7523 `jwt-bearer` assertion, or a self-signed JWT used
  directly — the Google/Apple service-account pattern). **This does not fit, and nothing in flux
  comes close.** `OAuthGrant` has no `JwtBearer` variant, there is no `private_key_jwt` client
  authentication, and `OAuth2Spec` has nowhere to declare a signing key, algorithm, issuer,
  audience, subject, or claim template. It would need a fifth grant plus a
  `JwtAssertionSpec { key_env, algorithm, issuer, audience, subject, ttl_secs }` — **and a JWT
  signing dependency flux does not currently have.** That is a real, maintainer-visible protocol
  addition.

**One more gap found either way: there is no client secret.** `OAuth2Spec` carries `client_id`
(`:404`) and nothing else — a `grep client_secret` across `crates/` finds it only in flux-providers'
Bedrock code and in flux-auth's *inbound* introspection (`crates/flux-auth/src/introspect.rs:48`), a
different subsystem entirely. The plugin OAuth path is public-client/PKCE shaped. **If babelforce's
SSO is a confidential client, `client_credentials` cannot be expressed today**, and `OAuth2Spec`
needs a `client_secret_env: Vec<String>` (env-keyed, never a literal, resolved host-side like
`user_env`). This is likely to bite before JWT does.

**Recommendation:** ask babelforce which of (a) or (b) their JWT plan is, and whether their SSO
client is public or confidential, **before** any provider ships. (a) costs nothing; (b) is a flux
protocol change that should be filed early rather than discovered after three providers are live.
Both the `client_secret_env` gap and the (b) contingency are captured in draft **F-10 / C-275**.

### 9. Agreement with [unified-auth.md](unified-auth.md)

[unified-auth.md](unified-auth.md) is the connector-side credential model: **source × acquisition ×
placement**, with flux's four `AuthScheme` variants as *presets* of it. This design is the flux-side
seam that model targets. They must agree, so this section states the agreement precisely and reports
where verification found it does **not** hold.

#### 9.1 This design does not replace `AuthScheme` — confirmed

Nothing here proposes a rival vocabulary. §2 reuses `flux_plugin_protocol::AuthScheme` and
`AuthMethod` as-is; §5 extends the *same* enum's `Query` variant to a second injection point; the
only proposed change to the protocol types is `AuthMethod`'s Basic halves (§7.5) and the OAuth2 gaps
(§8) — both additive, neither a reshaping. The unified model maps **onto** flux's vocabulary; flux
remains the authority for what reaches the wire.

#### 9.2 Do the four presets round-trip exactly? **Three fail. This is the significant finding.**

Checked one preset at a time against the code that actually composes the value.

| Preset | flux's implementation | Round-trips? |
|---|---|---|
| `Bearer` = static + `header{"Authorization","Bearer "}` | `format!("Bearer {token}")`, `crates/flux-plugin/src/host.rs:1248-1252` | **yes, exactly** |
| `Header{name}` = static + `header{name,""}` | `insert_http_header(&mut headers, &name, &value)`, `:1262-1264` | **yes, exactly** |
| `Basic` = basic_join + `header{"Authorization","Basic "}` | `STANDARD.encode(format!("{user}:{secret}"))` then `format!("Basic {encoded}")`, `:1253-1261` | **no — three ways** |
| `Query{name}` = static + `query{name}` | `url.query_pairs_mut().append_pair(name, value)`, `:1229-1231` | **no — encoding differs** |

**Failure 1 (major) — `basic_join`'s user half is env-verbatim, so it cannot express a *join*.**
The unified model's `basic_join { user_source }` treats the user half as a value produced by a
source. flux's is `user_env` (`crates/flux-plugin-protocol/src/lib.rs:436`), resolved by
`resolve_user` (`crates/flux-plugin/src/host.rs:631-641`), which returns the **first set env var
verbatim** — no composition, no template. So the preset round-trips **only when the user half is a
bare env value**, which is true for neither provider that needs Basic: zendesk needs
`{EMAIL}/token`, freshdesk needs a literal `X` password. This is the same gap as §7.5, and stating
it as a round-trip failure is the sharper framing: **the `Basic` preset is not a faithful preset of
`basic_join`; it is a preset of a narrower thing.** F-9 / C-274 is what makes the claim true.

**Failure 2 (security-relevant) — flux's Basic model assumes the user half is *not* a secret.**
`user_env` is documented as "config (not a gated secret), so they resolve directly from declared env
like an endpoint" (`crates/flux-plugin-protocol/src/lib.rs:433-435`), and `resolve_user`
(`crates/flux-plugin/src/host.rs:631-641`) accordingly **never calls `register_secret`**. The
unified model's `basic_join { user_source }` carries no such assumption. For **freshdesk the user
half *is* the API key** — so mapping freshdesk onto the preset as it stands puts the secret through
the non-secret config path and leaves it unregistered with the redactor. F-9 / C-274 must therefore
also let a half be declared secret-bearing, not just templated. **This one is worth flagging loudly:
it is a silent credential-leak shape, not an expressiveness complaint.**

**Failure 3 (minor but real) — `Query` placement encodes differently.**
`append_pair` applies `application/x-www-form-urlencoded` serialization, which encodes a space as
`+`. A generic `query{name}` placement that percent-encodes to `%20` does not produce byte-identical
URLs. Harmless for the token alphabets in scope, but "the presets round-trip exactly" is false as
written unless the unified model's `query` placement is *defined* as form-urlencoded. Recommend
defining it that way — matching flux is free, diverging is not.

**Consequence for unified-auth's Acceptance.** Its criterion *"The four flux `AuthScheme` presets
round-trip exactly, proving the model is a superset"* **does not hold today** — it holds for
`Bearer` and `Header{name}`, and becomes true for `Basic` only after F-9 / C-274, and for
`Query{name}` only if the encoding is pinned. That conformance test will fail honestly if written
now, which is the right outcome: it is the test that would have caught this.

#### 9.3 `prefix` — confirmed implicit, and `resolve_header_value` permits composed values

Both halves of the request check out.

- **Prefix handling is already implicit in the presets.** `Bearer` prefixes at
  `crates/flux-plugin/src/host.rs:1250` (`format!("Bearer {token}")`) and `Basic` at `:1258`
  (`format!("Basic {encoded}")`). A seam that resolves a credential to an `AuthMethod` and applies its
  scheme gets prefixing for free — there is no separate prefix step to build for the preset cases.
  Turning the prefix into *data* (so `Token `/`GenieKey ` cost nothing) is a change to `AuthScheme`
  and is **not** proposed here; the presets cover every provider in scope, and §9.1's discipline
  says grow the enum only when a real provider demands it.
- **Nothing in `resolve_header_value`'s contract prevents emitting a composed value.** Its signature
  is `fn resolve_header_value(val: &Value, ctx: &ToolContext, allowed: &[String]) -> Result<String>`
  (`crates/flux-web/src/http.rs:234`) — it returns *a header value*, not "an env var's value". The
  `$secret` branch happens to return the raw env value (`:250`), but nothing depends on that: the
  caller simply does `HeaderValue::from_str(&resolved)` (`:175`). Returning `Bearer <tok>` or
  `Basic <b64>` is entirely within the existing contract, and no caller changes.
  - One incidental property worth keeping: `HeaderValue::from_str` rejects control characters, so a
    credential containing a newline errors instead of injecting a header (`:175-179`). That is a
    fail-closed behavior the composed path inherits for free — do not replace it with a lossy
    sanitizer.

#### 9.4 Effectful acquisition — the machinery is reusable, the plumbing is plugin-bound

The design position (effectful acquisition runs in the host, never in generated Flux) is **supported
by what flux already has**, but the reuse is partial. Verified split:

**Reusable as-is — free functions in `flux-credentials` (L1) with no plugin concept anywhere:**

- `resolve_stored_bearer_with_client_factory` (`crates/flux-credentials/src/lib.rs:648-670`) — the
  whole store-first / stale-check / refresh-grant loop, with an injectable transport so the caller's
  own egress guard binds the connection.
- `CredentialStore` + `FileCredentialStore` over `~/.flux/credentials.toml` (`:700-720`).
- `OAuthToken { access, refresh, expires_at_ms, account_id }` (`:87-96`), with a `Debug` impl that
  redacts (`:98-102`).
- Expiry: the refresh buffer (`:661-664`) and `jwt_expiry_ms` (`:126-127`), which already reads a
  JWT's `exp` when the token response omits `expires_in` (`:431`).

**Plugin-bound — would need a seam before a connector can use it:**

1. **The credential-store key namespace is `plugin:<caller>:<purpose>`** —
   `crates/flux-plugin/src/host.rs:381` and `:426`, and the CLI writes the same key at
   `crates/flux-cli/src/auth_cmd.rs:171` and `:385`. A connector must get its own namespace
   (`connector:<name>:<credential>`) or it will collide with, and be indistinguishable from, a
   plugin's credentials. **This is the single most concrete blocker.** (`<purpose>` here is flux's
   own key format, quoted verbatim; the connector namespace is ours to name.)
2. **`resolve_purpose` is a private method on `SystemHostCaps`** (`crates/flux-plugin/src/host.rs:358`)
   reading plugin state — `self.auth`, `self.grants.secrets`, `self.caller`, `self.secret_sink`,
   `self.cred_store`. It is not callable from flux-web at any layer.
3. **`flux auth login` / `flux auth set` are written against the plugin store.** Both start with
   `plugins_dir()` + `load_descriptor` + `spawn_and_load_manifest`
   (`crates/flux-cli/src/auth_cmd.rs:136-140` and `:367-371`). A connector credential has no plugin
   descriptor and no binary to spawn, so **`flux auth login <connector>` cannot work today** — there
   is no path through that code that does not require an installed plugin.
4. **`OAuth2Spec.endpoint` names an `EndpointSpec` from the plugin manifest**
   (`crates/flux-plugin-protocol/src/lib.rs:391-394`), and the token URL is gated by
   `ensure_http_host_allowed` against that manifest's hosts (`crates/flux-plugin/src/host.rs:380`).
   A connector needs its own host list — which is F-5 / C-270's `http_hosts`, so the two stories
   meet here.

**Verdict:** effectful acquisition for connectors is **feasible with real reuse and is not a rewrite**,
but it is not free either. It needs a credential resolver parameterized over *(auth methods, host
allowlist, store-key namespace, secret sink)* rather than over `SystemHostCaps`, plus a login path
that does not assume a plugin descriptor. That is bounded, nameable work — filed as **F-11 / C-276**.
Until it lands, connectors can use only the **pure** acquisitions (`static`, `basic_join`), which is
exactly what the three in-scope providers need for their non-OAuth paths, and matches unified-auth's
own "ship the presets first" scope discipline.

#### 9.5 Two places the two designs currently disagree — flagged, not silently reconciled

1. **Where the auth declaration lives.** unified-auth says "the connector manifest, not the Flux
   module, carries the auth declaration". §3 of this design removed the connector manifest from the
   critical path (evidence: flux has no file-based capability manifest at all) and puts credentials in
   flux's **operator config** instead. **These are compatible on the point that matters** — the
   argument unified-auth is making is that the declaration lives *outside the generated Flux*, and
   operator config satisfies that identically. Only the file changes, not the boundary. If the
   manifest decision (F-8 / C-273) later goes the other way, this section is where the two docs
   re-converge; neither should be edited without the other.
2. **When an OR alternative is selected — a genuine conflict.** unified-auth says "choose the first
   mechanism whose credentials are all *configured* (their sources resolve)". §7.2 of this design
   says codegen selects at **build time**, which is what lets us claim flux needs no OR support at
   all. Both cannot be true: *configuredness* is only knowable where configuration lives, i.e.
   host-side, so a configuredness rule forces the OR structure to reach the host.
   - To be fair to unified-auth: its rule is **not** unsafe — checking whether a source resolves is
     not an authentication attempt, so it does not put a credential on the wire. My §7.2 objection
     (trying credentials until one works) does not apply to it.
   - **Recommendation for the first cut:** select at build time on *expressibility* plus a recorded,
     deterministic preference order, emit one alternative, and keep "flux needs no OR support" as a
     real simplification. Defer configuredness-based selection; adopting it later is a distinct
     change that must carry the OR structure into the host.
   - **This is not blocking:** none of zendesk, freshdesk or babelforce presents an OR alternative
     set. The decision can be made deliberately rather than under pressure — but it must be made in
     *one* of the two documents, not assumed differently by each.

## Alternatives considered

- **Pre-composed header value in one env var.** Zero flux changes and unblocks every header scheme
  today. Rejected: it makes the operator assemble credentials by hand, stores a value in a form
  nothing can validate, and leaves the host blind to the scheme. Kept on file as an emergency
  fallback only.
- **A `base64` builtin in flux-lang's `expr` whitelist.** Would let generated Flux compose Basic auth
  itself. Rejected: it forces the raw token into a bound symbol, defeating redaction, and it puts
  credential assembly in model-visible code.
- **A dedicated `connector.request` op in flux.** Cleaner call sites, but it is a second HTTP egress
  path — flux's invariants explicitly forbid hand-rolling a second URL guard. Reusing `http.request`
  keeps one guarded path.
- **A `~/.flux/connectors/` manifest registry as a precondition.** *Now rejected as a precondition*
  (it remains a reasonable later convenience). It would make flux accept its first capability grant
  with no binary-hash anchor — see below. Config-carried credentials achieve the same deny-by-default
  guarantee with flux's existing trust model.
- **Write connectors as typed plugins instead.** This is flux's *current* answer, and it works: flux
  ships `plugins/zendesk`, `plugins/jira`, `plugins/confluence`, `plugins/slack`,
  `plugins/opsgenie`, `plugins/huggingface`. It is rejected for flux-connectors' charter reasons
  (see this repo's `AGENTS.md` — services are generated, technologies are hand-written), but a flux
  maintainer *will* raise it, and the answer must be ready: a plugin per SaaS vendor does not scale
  to a generated catalogue, and it re-implements HTTP + pagination + error mapping by hand each
  time.

## Risks & open questions

### RESOLVED — "Does flux want connector manifests at all?"

The story asked this to be resolved with flux. The *facts* are now settled from source; only the
*decision* needs a maintainer, and it is a smaller decision than before because the design no longer
depends on the answer.

**Evidence.**

1. **flux has no "connector" concept.** `grep -rni connector` across `crates/` returns only
   TUI/render tree-drawing connectors (`crates/flux-tools/src/render.rs:44`,
   `crates/flux-tui/src/plan.rs:97`), the `Role::Connector` token kind
   (`crates/flux-lang/src/render.rs:24`), and hyper's `SlackClientHyperConnector`
   (`crates/flux-channels/src/adapters/slack.rs:56`). Nothing load-bearing.
   `crates/flux-credentials/src/lib.rs:49` even states "flux doesn't use connectors".
   So `~/.flux/connectors/` **would** be a new installable artifact kind, as the draft said.

2. **The sharper fact the draft missed: flux has no file-based *capability manifest* at all.** A
   `PluginManifest` is obtained by **spawning the plugin binary and sending a `manifest` request
   frame** — `PluginHost::manifest`, `crates/flux-plugin/src/host/loading.rs:187-189`. What lives on
   disk in `~/.flux/plugins` (`crates/flux-cli/src/execution.rs:544-546`) is a `PluginDescriptor`
   (`crates/flux-plugin/src/host/loading.rs:693-726`): `program`, `args`, `pinned`, `version`,
   `sha256`, `source`, `previous`, `git_url`, `git_commit` — **transport and integrity only, never
   capabilities**. Capabilities are Rust literals inside the binary, e.g.
   `plugins/zendesk/src/main.rs:131` (`http_hosts: vec!["*.zendesk.com".into()]`) and `:135`
   (`AuthMethod::basic(...)`).

3. **Therefore "fold it into the existing plugin manifest registry" is not available as stated.**
   That registry has no file-ingestion path, and a connector has no binary to spawn. Folding in
   would mean *building* the file path, which is the same work as a new registry.

4. **And it would break flux's integrity story.** `spawn_verified`
   (`crates/flux-plugin/src/host/loading.rs:91-100`) refuses to spawn on sha256 drift (D-48). A TOML
   file in `~/.flux/connectors/` granting credential access and egress hosts has no analogue of that
   control — edit the file, widen the grant, no signal. **This, not the file format, is the real
   objection**, and any proposal that ignores it will be rejected.

**Answer recorded.** Connector manifests are **not** a precondition for the `$auth` seam and are
removed from its critical path. The credential map reaches `WebOptions` from flux's operator config
(§6), which inherits `allowed_secrets`' trust model exactly. The remaining decision — whether flux
eventually accepts a file-based manifest kind (and with what integrity anchor: signature, pack-index
entry, or `flux connector install` recording a hash the way `PluginDescriptor` does) — **is a flux
maintainer call, because it changes flux's trust model.** It is filed as a separate, non-blocking
flux story (**F-8 / C-273** in the handoff) and this repo does not wait on it.

### Still open / carried

- **Cross-repo sequencing.** This is the critical path for milestone 1, and it lands in a different
  repository on a different release cadence. Mitigation: file the flux stories now and keep this
  repo's work (spec crate, codegen, golden tests) fully unblocked — none of it needs the seam until
  the live end-to-end run in C-15.
- **No flux release is scheduled for this.** See the C-16 story's `## Notes`; the seam is
  **unscheduled** as of flux `v0.38.0`.
- **Manifest trust.** If manifests are ever adopted, installing one is a trust decision equal to
  installing a plugin, and it needs the integrity anchor described above.
- **Token refresh for OAuth2 connectors** is out of scope for the first cut; the schema can name
  `oauth2`, but implement only the static schemes initially. Note this is *not* true of babelforce:
  its SSO-issued Bearer is the shipping path, so refresh matters sooner than "first cut" implies.
- **The `Basic` composition gap (§7.5) is on the critical path and was not in the original scope.**
  Two of three providers need it. If flux declines to change `AuthMethod`, zendesk and freshdesk are
  blocked even after the `$auth` marker ships. This is the second-highest risk in the design after
  cross-repo sequencing.
- **Babelforce's JWT shape is unknown (§8).** If it turns out to be a self-signed assertion rather
  than a token-endpoint-issued JWT, flux needs a new grant *and* a signing dependency. Ask before
  three providers ship, not after.
- **`OAuth2Spec` has no client secret (§8).** If babelforce's SSO client is confidential, this
  blocks babelforce independently of everything else in this design.
- **`Redactor` minimum length.** `add_secret` drops values under 6 characters
  (`crates/flux-secret/src/lib.rs:198`). Short vendor keys are unredactable by construction; this is
  a pre-existing flux property, not introduced here, but connector docs should not promise otherwise.

## Acceptance / done

- `http.request` accepts `{"$auth": {"credential": "<name>"}}` as a header value and injects per
  `AuthScheme`, with `Bearer`, `Basic`, and `Header` covered.
- An undeclared credential is refused before any value is read — proven by a failing-first
  test mirroring `secret_ref_to_non_allowlisted_env_var_is_refused`
  (`crates/flux-web/src/http.rs:571`).
- The **composed** value is registered with the redactor; a test asserts the on-the-wire
  `Authorization` value does not survive `Redactor::redact`.
- A request whose URL falls outside the connector's declared hosts is refused, using one shared
  host-matcher rather than a third copy.
- `Query`-scheme injection lands as a follow-up story with its own test, appending the parameter
  *before* the SSRF guard runs.
- **Several credentials on one request work**: two `$auth` markers on two different headers both
  resolve and both are injected, proven by a test. Codegen rejects a requirement set that would need
  two `Authorization` headers.
- **`AuthMethod` can express a Basic half as `env + literal`**, so zendesk's `<email>/token` and
  freshdesk's `<api_key>:X` compose host-side with no pre-composed env var (F-9 / C-274). Without
  this, `Basic` is implemented but unusable for two of our three providers.
- flux-connectors can generate a Zendesk op that authenticates against the live API with no
  pre-composed credential anywhere — and the same for Freshdesk (`Basic`) and babelforce (`Bearer`).
