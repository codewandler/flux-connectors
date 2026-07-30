# Design: the verified-webhook seam — flux-side signature verification

**Status:** reviewed — verified against flux source · **Pillar:** Bridge · **Epic:** `inbound-events` ·
**Story:** [C-64](../stories/C-64-design-verified-webhook-seam.md) · **Parent design:**
[inbound-events.md](inbound-events.md) (§"The flux-side seam") · **Companion:**
[channel-bindings.md](channel-bindings.md) (C-82 — the connector-side declaration this consumes) ·
**Handoff:** [inbound-events-flux-stories.md](inbound-events-flux-stories.md)

> This design describes a change to **`../flux`**, not to this repository. It is recorded here because
> flux-connectors is the consumer that motivates it, exactly as
> [auth-seam.md](auth-seam.md) records the outbound `$auth` seam. The implementation stories are filed
> on flux's own board.

### Provenance of the citations in this document

Every `path:line` below was read in `/home/timo/projects/flux` at **`v0.40.0-4-g2abd0a13`** (commit
`2abd0a13`, workspace version `0.40.0`). Symbol names are stable and line numbers are not; **re-grep
by symbol** rather than trusting a number if it does not land. Claims marked *inferred* were not read
off source and must be treated as recommendations, not findings.

### Naming — never call this "the inbound auth seam"

flux already has a **`request-auth-seam`** (`docs/designs/request-auth-seam.md`, stories D-64 and
D-68, both `done`): *inbound bearer → principal* resolution for flux-server. It is unrelated. This
work is **webhook signature verification**, and a story titled "inbound auth" on flux's board reads
as a duplicate of shipped work and gets closed. The same trap caught
[C-16](../stories/C-16-design-auth-seam.md) from the outbound side.

## Why

`HmacSpec` (`crates/connector-spec/src/inbound.rs`) already models the whole vendor matrix — digest,
encoding, header, prefix, signed template, timestamp selector, secret name, tolerance. The connector
side is designed and being built (C-59, C-60). **None of it can run**, because flux's webhook channel
has no place to run it.

Verified in flux: `channel webhook` authenticates with an optional **static bearer token** and
performs no signature verification of any kind.

- `WebhookSettings` is `{ addr, path, async, token }` — `crates/flux-channels/src/config.rs:18-32`.
- The only check in the request path is `constant_time_eq` against that token —
  `crates/flux-channels/src/adapters/webhook.rs:88-97`.
- A `grep -ni "hmac\|signature\|sha256"` over `crates/flux-channels/src/` and
  `crates/flux-server/src/` returns **nothing**.

A vendor that signs its payloads and cannot send a custom `Authorization` header — which is every
vendor in the matrix — therefore has **no authenticated route into flux at all**. The operator's only
options today are an unauthenticated public endpoint or no webhook.

## 1. Where verification happens in flux's request path

### The path as it stands

```
build_channels(&decls)                       crates/flux-channels/src/adapters/mod.rs:36
  └─ "webhook" | "http" => WebhookChannel::from_decl(d)                             :48
       (an unknown kind is a hard error)                                            :63
  └─ WebhookChannel::from_decl                crates/flux-channels/src/adapters/webhook.rs:31
       · deserializes WebhookSettings                                                     :32
       · refuses a non-loopback bind with no `token`                                   :40-46
  └─ WebhookChannel::router → Router::new().route(&self.path, post(handle))            :63-73
  └─ async fn handle(State(state), headers: HeaderMap, Json(body): Json<Value>)         :83-87
       · bearer check                                                                  :88-97
       · async branch → 202 Accepted, spawn deliver                                    :99-108
       · sync branch  → deliver, reply with the journeys' results                     :110-119
```

### The finding: there is no point inside `handle` at which the raw bytes exist

`Json(body): Json<Value>` at `webhook.rs:86` is an **axum extractor**. It consumes the request body
and deserializes it *before the handler body begins to run*. So "verify the raw body, before
parsing" is not a line that can be inserted into `handle` — the parse has already happened by the
time control reaches `webhook.rs:88`.

Two consequences follow, and the second is a pre-existing property worth naming:

1. **Changing `handle`'s signature is the structural change.** Everything else in this design layers
   on it. The shape is `body: axum::body::Bytes` as the final extractor, verification over `&body`,
   then `serde_json::from_slice::<Value>(&body)`.
2. **The existing bearer check already runs after the body is parsed.** An unauthenticated request's
   body is deserialized at `webhook.rs:86` before `webhook.rs:88` rejects it. That is not a signature
   bypass — the token is not computed over the body — but it is the exact ordering the HMAC path must
   not copy, and moving to `Bytes` fixes it for free by putting *both* checks ahead of the decode.

### The ordering rule

```
1. read headers                     (HeaderMap — already the first extractor, webhook.rs:85)
2. read the raw body as bytes       (never a Value, never a String round-trip)
3. resolve {timestamp} from a HEADER (see §2 — a body selector is not readable here)
4. build the signed string from the template over the raw bytes
5. constant-time compare, then check tolerance
6. ── only past this line ── serde_json::from_slice::<Value>(&body)
7. resolve the discriminator and delivery id (these MAY read the parsed body)
8. deliver
```

Step 6 is the whole design. Verifying a re-serialized body fails on byte-identical-but-reordered
JSON, and any normalize-then-verify step is a bypass rather than a convenience. The rule is stated
once, in this order, so a reviewer can check a diff against it mechanically.

**Step 7 is the reason the split is clean.** `Selector` (`crates/connector-spec/src/inbound.rs:100`)
admits `FieldSource::Header` and `FieldSource::Body`. The pre-parse/post-parse line falls exactly
between verification and routing: the timestamp is verification input and must be header-borne; the
discriminator and delivery id are routing inputs read after the decode, so a body path is fine for
them. See §2 for the constraint this places back on the connector side.

### What moves into the handler when `Json` leaves

`Json<Value>` does three things today that vanish with `Bytes`, and each must be reproduced
deliberately rather than silently dropped:

| behaviour today | with `Bytes` |
|---|---|
| rejects a non-`application/json` content type before the handler | the handler decides — and must decide **after** verification |
| rejects malformed JSON with `400` before the handler | `serde_json::from_slice` at step 6, `400` on error |
| inherits axum's `DefaultBodyLimit` | `Bytes` inherits the same default |

The content-type row is the one with an edge. Vendors do not all send `application/json` — Slack's
slash-command surface is form-encoded — and a 415 emitted *before* verification tells an
unauthenticated caller something about the endpoint. Rejecting on content type is fine; doing it
before the signature check is a small oracle. Put it after.

### Body limit — a new security parameter

`WebhookChannel::router` (`webhook.rs:63-73`) attaches **no** `DefaultBodyLimit` layer; it relies on
axum's built-in default. flux-server, by contrast, applies an explicit
`DefaultBodyLimit::max(limits.max_body_bytes)` over its whole surface, outermost, as C-189 —
`crates/flux-server/src/lib.rs:913` and `:1120`.

Verification changes what that limit means. Today the channel's cost per unauthenticated request is a
JSON parse; afterwards it is a JSON parse *plus an HMAC over the whole body*, and the HMAC runs
before anything has authenticated the caller. The limit is therefore the bound on work an anonymous
caller can cause, and the webhook channel should adopt the same explicit, configurable limit
flux-server already has rather than inheriting a default nobody chose. **Recommendation, not a
finding** — I did not measure the cost.

### Why not a tower layer

A `middleware::from_fn` layer in `router()` would also sit ahead of the handler, and it is the
tempting spelling. Rejected: a layer that wants the raw body must buffer the request stream itself
and hand the buffered copy downstream, which puts **two** places in the channel that materialise the
body. That is the seam along which a "normalize then verify" bypass grows. The channel has exactly
one route (`webhook.rs:71`), so an in-handler check is both sufficient and un-bypassable, and it
keeps one buffering site.

## 2. How `HmacSpec` reaches it

### It is program text, not a manifest

The credential-manifest question that dominated [auth-seam.md](auth-seam.md) §3 does not arise here.
A webhook's verification parameters reach flux the same way every other channel setting does: **the
operator writes them in their `.flux` program**, and flux-connectors' job is to *generate the block
to paste* (or, later, for a product's setup flow to write). Same trust anchor as `token` today, no
new artifact kind, no new capability grant.

This works **today, with no language change**, and that is verified rather than assumed:

- `lower_channel` puts every declaration attribute that is not `kind` straight into the settings bag —
  `crates/flux-lang/src/cst_decode.rs:1598-1612`.
- A settings value may be a record literal — `parse_setting_record`,
  `crates/flux-lang/src/cst_decode.rs:2120` dispatching to `:2171` — nested to
  `MAX_SETTING_DEPTH = 256` (`:2083`).
- `secret "NAME"` is recognised at **every** nesting depth, because it is a case of
  `parse_setting_prefix` itself (`crates/flux-lang/src/cst_decode.rs:2127-2130`), which recurses.

So this parses now:

```flux
channel gh
  kind "webhook"
  addr "0.0.0.0:8790"
  path "/hooks/github"
  verify {
    scheme: "hmac",
    algorithm: "sha256",
    encoding: "hex",
    header: "X-Hub-Signature-256",
    prefix: "sha256=",
    signed: "{body}",
    secret: secret "GITHUB_WEBHOOK_SECRET"
  }
```

### `HmacSpec` field by field

| `HmacSpec` (`crates/connector-spec/src/inbound.rs`) | `verify` key | notes |
|---|---|---|
| `algorithm: Digest` (`:130`) | `algorithm` | `sha256`, plus `sha1` for GitHub's legacy header |
| `encoding: Encoding` (`:132`) | `encoding` | `hex` \| `base64` |
| `header: String` (`:134`) | `header` | the header carrying the signature |
| `prefix: Option<String>` (`:137`) | `prefix` | literal, e.g. `sha256=`, `v0=` |
| `signed: String` (`:143`) | `signed` | template over `{body}` / `{timestamp}` |
| `timestamp: Option<Selector>` (`:151`) | `timestamp` | **header-only on the flux side — see below** |
| `secret: String` (`:157`) | `secret` | a *credential name* here; a `secret "ENV"` reference there |
| `tolerance: Option<String>` (`:164`) | `tolerance` | `5m`, `300s`; mandatory with `{timestamp}` |

The one field that does **not** cross unchanged is `secret`. In the connector IR it is the name of an
`AuthScheme::Signing` credential of that connector — a name, deliberately never a value. In the flux
program it is a `secret "ENV_NAME"` reference. The connector's generated snippet therefore emits the
*credential name* as the suggested env var and the operator binds it; flux never learns the
connector's credential vocabulary, and the connector never learns the operator's env.

### Nested record or flat prefix?

flux's own precedent points the other way and should be acknowledged rather than ignored:
`A2aSettings` spells its whole principal-auth sub-configuration as **flat prefixed keys** —
`introspect_url`, `introspect_secret`, `introspect_client_id`, `introspect_account_claim`, … —
`crates/flux-channels/src/config.rs:58-83`, not as a nested `introspect { … }`.

A nested `verify { … }` is still the right call here, for two reasons that do not apply to
`introspect_*`:

1. **`verify` is tri-state and must be legible as one thing** (§3). `verify "none"` versus a `verify`
   record versus absent is a single decision; eight flat `verify_*` keys make "did the author decide?"
   a question about which subset is present.
2. **The block is machine-generated and pasted whole.** A connector emits the record; a reviewer
   diffs the record. Flat keys interleave with `addr`/`path`/`token` and the unit of review is lost.

### Constraint discovered: `Selector` is wider than a verify-before-parse host can honour

`Selector` (`crates/connector-spec/src/inbound.rs:100-105`) admits `FieldSource::Body` (`:74`), and
`HmacSpec::timestamp` is an `Option<Selector>` (`:151`). **A body selector cannot be honoured for the
timestamp**, by construction: the timestamp is an input to the comparison that decides whether the
body may be parsed at all, so reading it out of the body means parsing before verifying — the exact
inversion this design exists to prevent.

This is not a hypothetical narrowing. No vendor in the matrix needs it: GitHub has no timestamp,
Slack and Zendesk use headers, Stripe's is a component of its signature header. So:

- **flux side:** a `verify` block whose `timestamp.source` is `body` is a **load error**, not a
  runtime failure. It is filed as acceptance on the foundation story.
- **connector side (a finding, not a change made here):** the IR permits a declaration flux must
  refuse. The cleanest fix is a loader rule in `connector-spec` — `HmacSpec::timestamp` must be
  `FieldSource::Header` — so the refusal happens at build time in the repository that owns the
  declaration, rather than at load time in the operator's runtime. **I did not make this change**;
  `crates/connector-spec/src/inbound.rs` belongs to C-59/C-60. It is recorded in C-64's story so it
  reaches the right owner.

`discriminator` and `delivery_id` are unaffected — both are read at step 7, after the decode, so
`FieldSource::Body` is legitimate for them and no narrowing applies.

### Gap: Stripe's composite header is not expressible in `prefix` + `Selector`

`docs/designs/inbound-events.md:58` and `crates/connector-spec/src/inbound.rs:118` both record Stripe
as `Stripe-Signature`, hex, signed `{timestamp}.{body}`, with a tolerance. The header's actual value
is a comma-separated list of `key=value` pairs:

```
Stripe-Signature: t=1614556800,v1=5257a869e7…,v0=6ffbb59b2300…
```

Three things follow that the current shape cannot say:

1. **The digest is not the whole header value, nor a literal prefix of it.** `prefix: Option<String>`
   (`inbound.rs:137`) strips a leading literal such as `sha256=`; it cannot select the `v1` element of
   a list whose order and membership Stripe does not promise.
2. **The timestamp is a component of that same header**, not a header of its own.
   `Selector { source: Header, name }` (`inbound.rs:100-105`) names a header; it has no way to say
   "the `t` component of it". The doc comment at `inbound.rs:145` already says "Stripe's `t=`
   component" — the *intent* is recorded, the *grammar* is not there.
3. **Stripe sends more than one `v1` during a secret rotation**, and a verifier must accept if **any**
   candidate matches. A scheme modelled as "one header value, one digest" silently fails every request
   during a rotation window.

This is a **connector-side modelling gap surfaced by writing the flux side**, and it is the strongest
argument in this document for designing the seam before the conformance matrix ships. Options, none
chosen here because the choice belongs to C-59/C-60:

- a `Structured`/`Pairs` encoding variant that says the header is a `k=v` list, with the digest under
  a named key and the timestamp under another — expressive, one new enum variant, and it makes
  `Selector` able to address a component;
- a per-vendor `HmacLayout` enum with a `Stripe` variant — smaller, and exactly the per-vendor code
  path the "one parameterized algorithm" finding exists to avoid;
- declare Stripe unverifiable for now. Not acceptable: Stripe is the flagship case in the design's own
  table.

Whichever lands, **flux's side must accept a set of candidate digests, not one**, so rotation works.
That is filed as acceptance on the scheme-matrix story regardless of how the declaration is spelled.

## 3. What a verification failure does

### Fail closed, and the assertion is the delivery count

A missing, malformed, mismatched or stale signature yields `401` and **no delivery** — no journey, no
trigger, no agent, no model call. The test that proves it asserts the **recording deliverer's
delivery count is zero**, not that the response status was an error. `crates/flux-channels/tests/e2e.rs:16-28`
already has the pattern: a `Tee` deliverer wrapping the real `AppDeliverer` and recording every run.
A status-code assertion would pass against a handler that returns 401 *and* delivers, which is the
defect worth testing for.

### The `async` branch is where fail-closed is easiest to get wrong

`webhook.rs:99-108` returns `202 Accepted` and spawns the delivery. Verification must sit **before**
that branch, not inside the spawned task — a verification failure discovered after the 202 has no way
to report itself and no way to stop the delivery it already scheduled. In the ordering of §1 this is
automatic (steps 1–5 precede step 8), but it is the one place where an implementation that "adds
verification to the delivery path" instead of to the request path would look correct and be wrong.

### The response body says nothing

The existing bearer rejection replies with the fixed literal `"unauthorized"`
(`webhook.rs:95`). The verification rejection matches it exactly: **one fixed string for every
failure mode.** Not "signature mismatch" versus "timestamp too old" versus "missing header" — a
caller that can distinguish those has a probe for how far its forgery got, and the operator's logs
are where that distinction belongs.

Two existing paths in the same file are the counterexamples to copy nothing from:

- `webhook.rs:118` returns `(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())` — a delivery error's
  text goes straight into the response body, to the vendor, un-redacted.
- `webhook.rs:104` is `eprintln!("webhook `{label}`: async delivery failed: {e}")` — straight to
  stderr, bypassing the redactor entirely.

Neither leaks the HMAC secret today (neither error is built from it). Both are the shape a
verification diagnostic must not take. See §4.

### An unknown scheme is a load error

`build_channels` treats an unknown channel kind as a hard error
(`crates/flux-channels/src/adapters/mod.rs:63`) and an unresolved `{"$secret":…}` marker as a hard
error (`first_unresolved_secret` at `:23-32`, the refusal at `:39-45`, the test at `:75`). A `verify`
block naming an unknown scheme, or a timestamped
`signed` template with no `tolerance`, or a `timestamp` selector sourced from the body, joins that
list. The channel refuses to build; it does not start and fail per-request. The precedent is already
there — a channel that cannot honour its own declaration must not bind a port.

### `verification = "none"` must stay loud on this side too

C-82 makes this a connector-side invariant: a webhook binding's `verification` is **tri-state** —
unset is a loader error, `VerificationScheme::None` is a deliberate statement, an `HmacSpec` is
verified (`crates/connector-spec/src/inbound.rs:323-334`). The flux side must not normalise that away,
and there are two distinct places it could:

1. **In the declaration.** `verify` absent and `verify "none"` must not be the same thing. The
   recommendation is that a webhook channel bound to a non-loopback address must state one or the
   other, mirroring the existing rule that a non-loopback bind requires a `token`
   (`webhook.rs:40-46`) — same reasoning, one step further: the host auto-approves tools, so an open
   endpoint with no stated verification decision is a remote-trigger surface.
2. **In what a flow can see.** This is the harder half, and it is where the seam currently has
   nothing. `flux_app::Event` is `{ label, payload }` and nothing else
   (`crates/flux-app/src/bus.rs:115-118`). There is no id, no source, no received-at, and **no
   "this was signature-verified" flag**. So a program written against a signed GitHub webhook behaves
   identically if an operator later points an unverified transport at the same trigger label — the
   flow cannot tell, and neither can a reviewer reading the flow.

The second is also where the delivery id has to go, and it collides with an existing behaviour:
`seed_payload` binds the whole payload to `$input` **and every top-level field to its own symbol**
(`crates/flux-app/src/app.rs:1988-1996`). So writing `delivery_id` or `verified` into the payload puts
envelope data in the message body where a vendor field of the same name silently shadows it. This
repository already filed the problem as [C-85](../stories/C-85-delivery-envelope.md); the flux-side
story is filed alongside the rest of this seam.

## 4. How the secret is supplied without reaching a log or an error

**The mechanism already exists and needs nothing new.** This is the part of the design that is
smallest, and it is smallest because flux got it right for `token` and the same path carries an HMAC
key unchanged. Verified end to end:

1. **Declaration.** `secret: secret "GITHUB_WEBHOOK_SECRET"` inside the nested record lowers to a
   `{"$secret": "GITHUB_WEBHOOK_SECRET"}` marker — `crates/flux-lang/src/cst_decode.rs:2127-2130`,
   reached at any depth because `parse_setting_prefix` recurses (`:2096`). A plaintext literal is
   never written in the program.
2. **Resolution + redaction, in one step.** `resolve_secrets` walks `program.channels`
   (`crates/flux-app/src/secrets.rs:24-26`) and `resolve_in` recurses through objects and arrays
   (`:47-58`); for each marker it reads the env var, **calls `redactor.add_secret(resolved.clone())`
   at `:43`**, and only then substitutes the value. A missing env var is a hard error (`:38-42`), not
   an empty secret.
3. **One redactor, shared.** `app_cmd.rs:437-438` constructs a single `flux_secret::Redactor` and
   passes it to `resolve_secrets`; `App::with_sub_agents`'s contract is to be handed *that same one*
   (`crates/flux-app/src/app.rs:216-218`), and `crates/flux-app/src/app.rs:2462-2463` is the test
   asserting a journey's tool output comes back scrubbed. So the secret is registered **before the
   channel is built**, and every output path that goes through the redactor is covered from the start.
4. **No marker survives.** `build_channels` refuses any unresolved `{"$secret":…}` marker anywhere in
   a channel's settings — `first_unresolved_secret`,
   `crates/flux-channels/src/adapters/mod.rs:23-32`, refused at `:39-45`, test at `:75`. A
   deserialization that quietly produced the literal string `[object Object]` as an HMAC key is not
   reachable.

Note what step 3 means for ordering: `build_channels` takes only decls and no redactor
(`crates/flux-cli/src/app_cmd.rs:608`), which is *fine precisely because* registration already
happened at `secrets.rs:43`. The webhook channel does not need — and should not acquire — a redactor
handle to register its own secret.

### Three caveats, each verified, each with a required response

**(a) `Redactor::add_secret` silently drops anything shorter than 6 characters** —
`crates/flux-secret/src/lib.rs:195-201`, the floor at `:198`. A short webhook secret is registered
nowhere and redacted never. For a bearer token this is a known flux property; for an HMAC key it is
worse, because a key that short is also cryptographically weak. **Required:** the `verify` loader
refuses a resolved secret shorter than the redactor's floor. Refusing costs nothing real — no vendor
issues a 5-character signing secret — and it converts a silent redaction hole into a load error.

**(b) `WebhookSettings` derives `Debug`** — `crates/flux-channels/src/config.rs:18` — and already
holds the resolved plaintext `token` at `:31`. No call site formats it today (`grep '{:?}'` over
`crates/flux-channels/src/` finds none), so nothing leaks now; a `verify` record placed in that struct
would inherit a derived `Debug` that prints the HMAC key the first time anyone adds a trace line.
**Required:** the verification config gets a hand-written redacting `Debug`, or `WebhookSettings`
loses its derive. flux has the precedent to copy — `OAuthToken`'s `Debug` impl at
`crates/flux-credentials/src/lib.rs:98-102`, which prints `"<redacted>"` for the value while keeping
`Some(_)`/`None` observable so "a secret exists" stays diagnosable.

Note the same hazard one level up, pre-existing and not introduced here: `ChannelDecl` derives both
`Debug` and `Serialize` (`crates/flux-lang/src/program.rs:75`) and its `settings` is a plain
`serde_json::Value`, so after `resolve_secrets` the in-memory `Program` holds plaintext. The redactor
seeded at `secrets.rs:43` is what covers it — which is exactly why any *new* output path must go
through the redactor rather than around it.

**(c) Two paths in the webhook adapter go around the redactor** — `webhook.rs:104` (`eprintln!`) and
`webhook.rs:118` (error text into the HTTP response). **Required, and it is the cheapest of the
three:** no verification diagnostic is built from the secret, the presented signature, or the computed
digest. Log the channel name, the failure class and nothing else. A computed digest is a function of
the secret and an attacker-supplied body — printing it is an oracle, not a diagnostic.

### The comparison itself

`constant_time_eq` already exists in the file — `webhook.rs:123-132`, length-aware, with the comment
saying it mirrors flux-server's. It is directly reusable for the digest compare and should be reused
rather than a `subtle`-style dependency added; flux has no `subtle` in its workspace and does not need
one for a fixed-length digest comparison.

**No new third-party dependency is required.** `hmac = "0.13"`, `sha2 = "0.11"`, `base64 = "0.23"` and
`hex = "0.4"` are all already `[workspace.dependencies]` (`Cargo.toml:150-153`), and flux-providers
already computes HMAC-SHA256 with them for SigV4 (`crates/flux-providers/src/bedrock.rs:32`, `:42`,
`:697-704`). flux-channels adding them is a workspace-internal edge. Layering raises no question
either: flux-channels is **L6**, the top layer (`crates/flux-codegate/src/lib.rs:53-54`), and the four
crates are pure.

## 5. The six capabilities, and where each is filed

C-64's acceptance names six flux-side capabilities. Each is filed as a story on flux's board; the
foundation is the only hard prerequisite.

| # | capability | flux story | this design |
|---|---|---|---|
| 1 | a declarative `verify` block on `channel webhook` | C-291 | §2 |
| 2 | verification over the **raw body, before parsing** | C-291 | §1 |
| 3 | constant-time comparison + timestamp tolerance | C-292 | §3, §4 |
| 4 | discriminator → trigger-label routing | C-294 | §1 step 7 |
| 5 | a challenge/handshake hook answered without waking an agent | C-293 | below |
| 6 | the delivery id in the payload | C-295 | §3 |

```
C-291  raw-body capture + the `verify` declaration     ← the foundation
C-292  scheme matrix, constant-time compare, tolerance ← depends on C-291
C-293  challenge/handshake, answered without a turn    ← depends on C-291, independent of C-292
C-294  discriminator → trigger-label routing           ← depends on C-291
C-295  the delivery envelope: id + verified flag       ← depends on C-291
```

**Capability 5 (the challenge hook) is the one this document has not otherwise discussed**, because it
is the only one that is not about verification. Slack's `url_verification` echo and Meta's
`hub.challenge` GET arrive at the same path as real events, and answering them by waking a journey is
both wasteful and a way to hand vendor-shaped text to an agent for no reason. It rides the same
raw-body capture — a handshake is matched on the decoded body *after* verification where the vendor
signs it, and explicitly documented where the vendor does not. Note the GET half: the channel routes
only `post` today (`webhook.rs:71`), so Meta-style `hub.challenge` needs a `get` route as well.

## Invariants

1. **Verify before parse.** The raw bytes are the message. There is no normalize-then-verify step, and
   the JSON decode is textually after the comparison in one function. §1's ordering is the review
   checklist.
2. **Fail closed with zero delivery.** The test asserts a delivery count of `0`, not a status code.
3. **Timestamp tolerance is part of verification.** A `signed` template interpolating `{timestamp}`
   with no `tolerance` is a **load error**, not a warning. Without a window a captured request replays
   forever, which is worse than not timestamping — it reads as though replay were handled.
4. **The timestamp is read from a header.** A body-sourced timestamp selector is a load error, because
   honouring it would require parsing before verifying.
5. **The secret never surfaces.** It reaches flux as a `secret "ENV"` reference, is registered with
   the shared `Redactor` before the channel is built, is refused if shorter than the redactor's floor,
   is not printed by any derived `Debug`, and appears in no log line, error string or response body —
   nor does the computed digest.
6. **One fixed rejection string.** Every verification failure mode produces the same response body.
7. **`verify "none"` stays visible.** Absent and explicitly-none are different declarations, and a
   flow can tell a verified delivery from an unverified one without inspecting the absence of a field.
8. **A channel that cannot honour its declaration does not bind a port.** Unknown scheme, missing
   tolerance, body-sourced timestamp, too-short secret: all load errors, consistent with an unknown
   channel kind (`adapters/mod.rs:63`).

## Alternatives considered

- **A tower middleware layer instead of an in-handler check.** Rejected in §1: it needs a second place
  that buffers the body, and that is the seam a bypass grows along.
- **Verify inside the `Deliverer` rather than the channel.** Superficially attractive — one
  implementation for every transport. Rejected: the `Deliverer` receives a parsed `Value`
  (`crates/flux-channels/src/adapters/webhook.rs:110`), so by construction it is downstream of the
  parse. It is the wrong side of the line this whole design draws.
- **A generic signed-webhook primitive in flux with no connector-side declaration.** flux would carry
  the mechanism while the per-vendor parameters stayed tribal knowledge in operator config, invisible
  to this repository's drift check. Half the value, none of the maintenance story.
- **Ship the verification in flux-connectors as a library flux calls.** It would keep the matrix in one
  repository. Rejected: it makes flux depend on this repository at runtime, and flux-connectors' north
  star is that it compiles and ships no runtime.
- **Carry the parameters in a connector manifest flux installs.** This is [auth-seam.md](auth-seam.md)
  §3's rejected precondition, and the rejection transfers: a file granting verification authority with
  no integrity anchor is a trust-model change. Program text has the operator's own trust anchor and
  needs no new one.
- **Reuse the `token` bearer check and tell vendors to send a bearer.** They cannot. That is the
  premise.

## Open questions

- **Stripe's composite header (§2)** is unresolved and belongs to C-59/C-60. flux's side must accept a
  *set* of candidate digests either way, so the flux stories are not blocked on the answer — but the
  conformance matrix (C-60) is.
- **Where the verified flag and delivery id live** — an envelope on `flux_app::Event`, or a reserved
  payload prefix. [C-85](../stories/C-85-delivery-envelope.md) owns the decision; §3 states why the
  status quo (`Event { label, payload }`, `crates/flux-app/src/bus.rs:115-118`) cannot express it and
  why `seed_payload` (`crates/flux-app/src/app.rs:1988-1996`) makes the cheap answer collide with
  vendor fields.
- **Should a webhook channel be *required* to state a verification decision?** §3 recommends it for a
  non-loopback bind, mirroring `webhook.rs:40-46`. It is a breaking change for any existing
  non-loopback webhook program, so the call is flux's.
- **Body limit (§1)** — adopting C-189's explicit `DefaultBodyLimit` on the channel is a
  recommendation from reading, not from measurement.
- **Socket and poll transports need none of this.** `Transport::Socket` authenticates by the credential
  that opened the connection and `Transport::Poll` is an outbound call
  (`crates/connector-spec/src/inbound.rs:54-65`). This seam is webhook-only, and that is why C-63 can
  ship with no flux-side blocker at all.
