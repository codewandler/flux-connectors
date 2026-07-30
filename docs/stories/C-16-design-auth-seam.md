---
id: C-16
title: Design the $auth seam and file its stories on flux's board
pillar: Bridge
status: ready
priority: 2
design: docs/designs/auth-seam.md
epic: connectors-v1
areas: [flux-bridge]
note: **critical path** · ships in ../flux, longest lead time
---

# Design the $auth seam and file its stories on flux's board

## Goal
Settle the design for scheme-aware credential injection in flux's `http.request`, and get the
implementation stories onto `../flux`'s board early — it ships on a different repo's release cadence
and blocks milestone 1's finish.

## Acceptance
- [x] [docs/designs/auth-seam.md](../designs/auth-seam.md) reviewed and its open question resolved
      **with flux**: does flux want a separate connector-manifest registry, or should connector auth
      fold into the existing plugin manifest registry?
      → Resolved **from flux's source**, not from a maintainer conversation (see Progress). The
      answer is *neither, as posed*: flux has no file-based capability manifest at all, and the
      plugin registry has no file-ingestion path to fold into. The design was changed so it no
      longer depends on either; the residual trust-model decision is filed as draft **F-8 / C-273**.
- [ ] Implementation stories filed on `../flux`'s board covering: the `{"$auth": {...}}` header
      marker, `AuthScheme` reuse from `flux-plugin-protocol`, deny-by-default purpose resolution,
      redactor registration of the composed value, `http_hosts` scoping, and `Query`-scheme injection
      as its own story.
- [ ] Each filed story names its failing-first test.
      → Every draft names one (F-8/C-273 excepted and explicitly justified: a decision story with no
      behavioral change). Not tickable until the stories are actually filed.
- [x] This repo records which flux release the seam is expected in, so `C-15` knows what to wait for.
      → See Notes: **unscheduled** as of flux `v0.37.0`.

## Progress
- **2026-07-30 — design hardened against flux source; handoff drafts written. Filing on flux's board
  is still outstanding.**
- Verified every claim in the design against `/home/timo/projects/flux` at commit `bcfab0ad` plus
  that checkout's uncommitted working-tree changes (read-only; nothing written there). Four cited
  files were dirty at read time — `flux-cli/src/execution.rs`, `flux-codegate/src/lib.rs`,
  `flux-plugin-protocol/src/lib.rs`, `flux-plugin/src/host.rs` — so their line numbers track the
  working tree. Re-grep by symbol if a line number does not land.
- **Confirmed as drafted:** the `$secret` marker is whole-value, headers-only, no
  prefix/encode (`flux-web/src/http.rs:234`, `:275`, sole call site `:171`); the four `AuthScheme`
  variants and `AuthMethod`'s `purpose`/`env`/`user_env`/`scheme`/`oauth2`
  (`flux-plugin-protocol/src/lib.rs:344-354`, `:422-443`, plus an undocumented `description`);
  the layer numbers (`flux-codegate/src/lib.rs:36` = L0, `:51` = L5); the redactor call
  (`flux-web/src/http.rs:248`); and `WebOptions` as the right carrier
  (`flux-web/src/lib.rs:75`, `allowed_secrets` at `:97`).
- **Corrected — three things the draft got wrong or oversold:**
  1. *"Reuse the plugin host's injection logic."* Only the **types** are reusable. `AuthInjection`
     is private (`flux-plugin/src/host.rs:755`), `resolve_auth` is a private method (`:644`), and
     the Basic base64 is inline inside a `match` arm (`:1253-1261`). Reuse requires extracting a
     pure L0 function first — now draft **F-1 / C-266**.
  2. *"flux-web may depend on flux-plugin-protocol."* True but beside the point: flux-web **already
     has** `AuthScheme`/`AuthMethod` in scope, because it already depends on `flux-plugin` and
     `flux-plugin/src/lib.rs:35` is `pub use flux_plugin_protocol::*;`. **No new dependency at all.**
  3. *"Connector manifests in `~/.flux/connectors/`."* The premise was wrong — see below. The
     design no longer depends on them; purposes travel in flux's operator config instead.
- **Open question resolved with evidence.** flux has **no file-based capability manifest of any
  kind**: a `PluginManifest` is fetched by spawning the binary and sending a `manifest` frame
  (`flux-plugin/src/host/loading.rs:186-188`), and what is on disk in `~/.flux/plugins`
  (`flux-cli/src/execution.rs:544-546`) is a `PluginDescriptor` (`loading.rs:693-726`) carrying
  transport + `sha256` only, never capabilities. So `~/.flux/connectors/` would be flux's first
  capability grant with no binary-hash anchor — `spawn_verified`'s drift refusal
  (`loading.rs:91-100`, D-48) has no analogue for a TOML file. There is also no "connector" concept
  to fold into (`flux-credentials/src/lib.rs:49` literally says "flux doesn't use connectors").
- **Strengthened the redaction requirement.** `Redactor::redact` matches by **exact substring**
  (`flux-secret/src/lib.rs:204-215`), so registering the raw token provably does *not* redact a
  `Basic` header. Also found: `add_secret` silently drops values under 6 chars (`:198`).
- **Adjacent bug found in flux (not fixed here):** the plugin host composes a Basic credential at
  `flux-plugin/src/host.rs:1253-1261` and never registers it with the redactor — the shipped
  `plugins/zendesk` path. Filed as draft **F-7 / C-272**.
- **Naming hazard:** flux already has a *done* `request-auth-seam` (D-64/D-68) for **inbound**
  bearer→principal auth. All drafts say "outbound" so reviewers do not conflate them.
- **Second pass — multi-scheme, the three real providers, and JWT forward-compat folded in:**
  - **Multi-scheme is first-class.** Adopted OpenAPI's `security` semantics verbatim (AND within a
    requirement object, OR across objects, `[]` = explicitly no auth, absent = inherit).
    **Confirmed one `$auth` marker per header carries an AND set**: the header loop at
    `flux-web/src/http.rs:169-181` calls `resolve_header_value` once per header with no shared
    state, so this needs no extra flux design — but F-2/C-267 now carries a test proving it, because
    "one credential per request" is exactly the assumption that gets introduced by accident later.
    Two limits recorded: two schemes cannot both target `Authorization` (permanent, acceptable —
    codegen rejects such a set), and a `Query` scheme has no header to hang a marker on, so
    F-6/C-271 now also adds a request-level `auth: [{purpose}]` array.
  - **OR is resolved at build time, so flux needs no OR support at all** — a real simplification.
    Which alternative codegen chose is recorded *here* (C-7 lockfile, so C-13 diffs it and C-14 can
    drift-check it), never in flux; flux's purpose map needs only the union of declared purposes.
  - **All three providers are blocked on this seam** — zendesk `Basic <email>/token`, freshdesk
    `Basic <api_key>:X`, babelforce SSO-issued `Bearer`. The design now says so plainly instead of
    implying a partial-credit path.
  - **Withdrawn:** babelforce's `X-Auth-Access-Id`/`X-Auth-Access-Token` raw header pair is being
    deprecated and scrubbed from the API. It is retracted from the design and must not be modeled or
    emitted. babelforce is Bearer-only.
  - **New critical-path gap found (F-9 / C-274):** `AuthMethod.user_env`
    (`flux-plugin-protocol/src/lib.rs:436`) resolves an env var **verbatim**, so neither zendesk's
    `<email>/token` suffix nor freshdesk's literal `X` password can be expressed. `Basic` would ship
    implemented-but-unusable for two of three providers. Fix mirrors the existing
    `EndpointSpec.template` precedent (`:512-519`).
  - **JWT forward-compat answered.** The *header* shape is a non-issue — `AuthScheme::Bearer` covers
    a JWT with zero reshaping, and flux already decodes a JWT's `exp` for refresh scheduling
    (`flux-credentials/src/lib.rs:126-127`, used at `:431`). The *acquisition* path splits: a
    token-endpoint-issued JWT needs **no protocol change**; a self-signed/RFC-7523 assertion fits
    nothing — no `JwtBearer` grant, nowhere to declare a signing key, and no signing dependency in
    flux. Separately, `OAuth2Spec` has **no client secret** at all (only `client_id` at `:404`), so a
    confidential-client `client_credentials` grant is inexpressible today — likely to bite babelforce
    before JWT does. Both filed as F-10 / C-275, and both hinge on two questions for babelforce:
    public or confidential client, and token-issued or self-signed JWT.
- **Third pass — reconciled with [unified-auth.md](../designs/unified-auth.md)** (read from
  `main` at `652849a`; new §9 in the design):
  - **`AuthScheme` is not being replaced** — confirmed and stated explicitly. This seam reuses it;
    the only protocol changes proposed are additive (`AuthMethod` Basic halves, the OAuth2 gaps).
  - **Three of the four presets do not round-trip exactly — the headline finding.**
    `Bearer` and `Header{name}` are exact (`flux-plugin/src/host.rs:1248-1252`, `:1262-1264`).
    **`Basic` fails twice:** flux's user half is `user_env` resolved **verbatim** (`resolve_user`,
    `flux-plugin/src/host.rs:630-640`), so it is a join of *a bare env value* and a secret, not a
    general `basic_join` — zendesk's `{EMAIL}/token` and freshdesk's literal `X` are inexpressible;
    and flux **assumes the user half is not a secret**
    (`flux-plugin-protocol/src/lib.rs:433-435`, and `resolve_user` never calls `register_secret`),
    which for **freshdesk — where the user half *is* the API key** — is a silent credential-leak
    shape, not merely an expressiveness complaint. **`Query{name}` fails minorly:** `append_pair`
    (`flux-plugin/src/host.rs:1230`) is form-urlencoded (space → `+`), so a `%20`-encoding
    placement is not byte-identical; recommend pinning the unified model's `query` placement to
    form-urlencoding. Consequence: unified-auth's acceptance criterion *"the four presets round-trip
    exactly"* is **false today**, becomes true for `Basic` after F-9/C-274, and for `Query` only if
    the encoding is pinned. Written that conformance test now would fail honestly — which is right.
  - **Prefix confirmed implicit** in `Bearer`/`Basic` (`format!("Bearer {token}")` at `:1250`,
    `format!("Basic {encoded}")` at `:1258`), so a purpose→`AuthMethod`→scheme seam gets it free.
    **And `resolve_header_value` permits composed values:** it returns `Result<String>`
    (`flux-web/src/http.rs:234`) — *a header value*, not "an env var's value" — and the caller just
    does `HeaderValue::from_str` (`:175`). No caller changes. Bonus: `from_str` rejects control
    characters, so a newline-bearing credential errors instead of injecting a header — keep it.
  - **Effectful acquisition: machinery reusable, plumbing plugin-bound** (the requested finding).
    Reusable as-is, L1, no plugin concept: `resolve_stored_bearer_with_client_factory`
    (`flux-credentials/src/lib.rs:648-670`), `CredentialStore`/`FileCredentialStore` (`:700-720`),
    `OAuthToken` (`:87-96`), refresh buffer (`:661-664`), `jwt_expiry_ms` (`:126-127`).
    Plugin-bound: the store key is **`plugin:<caller>:<purpose>`** (`flux-plugin/src/host.rs:381`,
    `:426`; CLI `flux-cli/src/auth_cmd.rs:171`, `:385`) — a connector would collide with plugins;
    `resolve_purpose` is a private `SystemHostCaps` method (`:358`); and **`flux auth login`/`auth
    set` cannot work for a connector at all** — both require `plugins_dir()` + `load_descriptor` +
    `spawn_and_load_manifest` (`flux-cli/src/auth_cmd.rs:136-140`, `:367-371`), and a connector has
    no binary to spawn. **Verdict: feasible with real reuse, not a rewrite** — needs a resolver
    parameterized over (auth methods, host allowlist, key namespace, secret sink). Filed F-11/C-276.
    Not needed for the first cut: all three providers need only pure acquisition.
  - **Two disagreements flagged rather than silently reconciled** (design §9.5): (a) unified-auth
    puts the auth declaration in a *connector manifest*, this design puts it in flux operator config
    — compatible on the point that matters (declaration lives outside the generated Flux), only the
    file differs; (b) **a genuine conflict on OR-selection timing** — unified-auth selects the first
    set whose sources *resolve* (host-side, needs OR in the host), this design selects at build time
    (which is what buys "flux needs no OR support"). unified-auth's rule is *not* unsafe — checking
    configuredness is not an auth attempt. Recommended first cut: build-time selection on
    expressibility with a recorded preference order. Not blocking: none of the three providers has
    an OR alternative set.
- **Deliverables:** [auth-seam.md](../designs/auth-seam.md) rewritten with cited/corrected claims
  and a new §9 reconciling it with [unified-auth.md](../designs/unified-auth.md);
  [auth-seam-flux-stories.md](../designs/auth-seam-flux-stories.md) holds eleven paste-ready story
  drafts (F-1…F-11, provisional ids C-266…C-276).
- **Next agent:** the only thing left is a human pasting the drafts into
  `../flux/docs/stories/` and running flux's `/track:board`. Writing to that repo was explicitly out
  of scope for this run (it holds unrelated uncommitted work).

## Notes
- Do **not** implement the flux change from this repo; file the stories and let flux's own workflow
  run. This story is done when the design is settled and the work is queued there.
- The rejected fallback (operator stores a pre-composed `Authorization` value) is documented in the
  design as an emergency option only.
- Key flux files: `crates/flux-web/src/http.rs:234` (`resolve_header_value`),
  `crates/flux-plugin-protocol/src/lib.rs:344` (`AuthScheme`).

### Which flux release is the seam expected in? — **UNSCHEDULED** (for `C-15`)

- **Answer: unscheduled.** As of flux **`v0.37.0`** (`../flux/Cargo.toml:49`) there is **no story on
  flux's board** for the outbound `$auth` marker, **no roadmap entry**, and nothing in flux's
  `[Unreleased]` CHANGELOG section. `grep -rn '\$auth' ../flux/docs/` returns only the *plugin*
  `auth_purpose` mechanism, which is a different path.
- **`C-15` must not plan against a date.** flux's `docs/roadmap.md` "Next" section carries roughly
  ten epics that are already in-progress or designed and filed ahead of this. Treat the live
  end-to-end run as gated on an event (the seam merging in flux), not on a version number.
- **What would change this:** a human pasting
  [auth-seam-flux-stories.md](../designs/auth-seam-flux-stories.md) into `../flux/docs/stories/` and
  flux's own board scheduling F-1…F-6 (+ F-9, which is critical path for zendesk and freshdesk).
  Until then the expected release is genuinely unknown, and this repo should not assert one.
- **Meanwhile nothing else here is blocked.** The spec crate, codegen, golden tests and the parse +
  analyze gate all run offline against `flux_lang` and need no seam. Only C-15's live call does.

### Do not conflate with flux's *other* auth seam

flux already has a **done** `request-auth-seam` (`../flux/docs/designs/request-auth-seam.md`, stories
D-64 and D-68) — that is **inbound** per-request bearer→principal resolution for flux-server, shipped
and unrelated. Every draft in the handoff says "outbound" for exactly this reason.
