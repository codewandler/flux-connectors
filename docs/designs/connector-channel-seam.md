# Design: the generic `connector` channel kind — the flux-side seam for a channel binding

**Status:** reviewed — verified against flux source · **Pillar:** Bridge ·
**Epic:** `channel-bindings` · **Stories:** [C-84](../stories/C-84-flux-connector-channel-seam.md) ·
**Handoff:** [channel-bindings-flux-stories.md](channel-bindings-flux-stories.md) (which flux ids
were filed) · **Companions:** [channel-bindings.md](channel-bindings.md) (the connector-side model
this seam consumes), [connector-tool-pack.md](connector-tool-pack.md) (the outbound interop layer
this seam **calls** rather than duplicates), [verified-webhook-seam.md](verified-webhook-seam.md)
(C-64 — the inbound transport primitive this seam **stands on** rather than restates),
[inbound-events.md](inbound-events.md)

> This design describes a change to **`../flux`**, not to this repository. It is recorded here
> because flux-connectors is the consumer that motivates it, and because the data it consumes —
> `ChannelBinding` — is defined here. No Rust in this repository changes for it. The implementation
> stories belong on flux's own board; [the handoff](channel-bindings-flux-stories.md) records which
> ids were filed.

### Provenance of the citations in this document

Every `path:line` below was read in `/home/timo/projects/flux` on **2026-07-30**, at workspace
version **0.40.0**, commit **`2abd0a13`** (released line: v0.38.0). Symbol names are stable and line
numbers are not — **re-grep by symbol** rather than trusting a number that does not land.

Two claims recorded in [C-84](../stories/C-84-flux-connector-channel-seam.md)'s Notes were **stale on
re-reading and are corrected here**:

- *"`AppDeliverer` serializes deliveries behind a mutex"* (inherited from the C-64 handoff). It does
  not, and has not since flux A-112/A-129: deliveries run **concurrently**, bounded by an admission
  limit, and a submission past the bound waits rather than being dropped
  (`crates/flux-channels/src/lib.rs:19-38`, `crates/flux-app/src/app.rs:503-513`). A connector
  channel therefore needs no serialization of its own **and** must not assume one-at-a-time delivery.
- *"`adapters/slack.rs` is ~218 lines"* — 217, unchanged in substance.

## Why

`flux-channels` dispatches a channel `kind` through a **closed `match`**
(`crates/flux-channels/src/adapters/mod.rs:46`); an unknown kind is a hard load error (`:63`), so
neither a plugin nor a connector can supply one. One arm of that match, `slack`, is 217 lines of
vendor-specific Rust, and its last act is to hand-build a `chat.postMessage` from `channel`, `text`
and `thread_ts` (`crates/flux-channels/src/adapters/slack.rs:150-154`).

Those three are the body parameters of `slack-chat-post-message`, an operation this repository
already compiles. [channel-bindings.md](channel-bindings.md) landed the declaration
(`crates/connector-spec/src/inbound.rs:306`, `ChannelBinding`) and `providers/slack.toml:385,414`
declares both of Slack's real transports from one event set, one payload map and one reply. What is
missing is the consumer: **flux cannot yet read any of it.**

## The one-sentence shape

> A channel is a **transport** (flux owns the socket) plus a **binding** (a connector declares the
> semantics). flux gains one generic `connector` arm that reads a binding from a published connector
> manifest and drives it; the reply is a **tool call through `Executor::dispatch`**, not new adapter
> code.

Everything below is the consequence of that sentence plus the three constraints it must not break.

## The three constraints

1. **The three-way split from [C-82](../stories/C-82-channel-bindings-epic.md) holds.** Vendor
   channels are declared here; generic transports (`webhook`/`http`) and time/lifecycle sources
   (`schedule`/`cron`, `startup`, `cli`) stay in flux, permanently. Compiling a scheduler here would
   make this repository a runtime, which `docs/vision.md` forbids. **This design adds no loop, no
   listener and no request path to this repository** — it adds a *reader* on flux's side.
2. **A binding is a composition, not a primitive.** Its inbound half names declared events; its
   outbound half names a declared **operation of the same connector**. Nothing new is emitted into
   the `.flux` module, and flux must not grow a second reply mechanism beside the operation.
3. **Every rule is a refusal, and the flux side must not be able to bypass it.** A webhook binding
   with no stated verification is already refused at load here (`VerificationScheme` is a tri-state —
   `crates/connector-spec/src/inbound.rs:173`, and an unset one on a `webhook` is a loader error).
   flux must reproduce that refusal against the *manifest it reads*, because a manifest can be edited
   after it is published.

## What the operator declares

A `ChannelDecl` is `{ name, kind, settings }` where `settings` is an opaque JSON bag
(`crates/flux-lang/src/program.rs:76`). The generic kind uses it like this:

```flux
channel support {
  kind      = "connector"
  connector = "slack"          # the connector id — selects the manifest
  service   = "api"            # optional; the reserved `default` service is elided
  binding   = "events-api"     # the ChannelBinding name within that service

  addr = "0.0.0.0:8790"        # transport = "webhook": where flux listens
  path = "/slack/events"

  # Every credential the binding NAMES, mapped to this deployment's secret. Uses flux's one
  # existing secret mechanism; the manifest carries names, never values.
  credentials {
    "slack.signing_secret" = secret "SLACK_SIGNING_SECRET"
    "slack.bot_token"      = secret "SLACK_BOT_TOKEN"
  }

  # Deployment policy. Keys are payload symbols the BINDING declares; values are this
  # deployment's ids. Never in the manifest — see "Allow-lists are operator config".
  allow {
    user    = ["U0123ABCDEF"]
    channel = ["C0123ABCDEF"]
  }
}

trigger mention { on = "support.app_mention", run = "answer" }
```

Note what is **absent from the manifest** and present here: the address, the path, the secret values
and the allow-lists. And what is absent from the program: the payload map, the signature scheme, the
discriminator, the reply operation and its parameter bindings. That division is the whole design.

### Where the manifest is read from

`~/.flux/connectors/<connector>.connector.toml`, resolved through `flux_system::System` like every
other real IO in flux, with an explicit `manifest = "…"` setting as an override. This mirrors
`~/.flux/flows` (`crates/flux-tools/src/flows.rs:26`), which is already how a connector's `.flux`
module reaches a flux host, so an installed connector is **one directory pair**, not two mechanisms.

The read happens in `build_channels`, **before any listener binds**. That placement is what makes
every rule below a load error rather than a first-delivery surprise.

## `build_channels` gains one arm, not one per vendor

```rust
match d.kind.as_str() {
    "schedule" | "cron" => …,
    "webhook" | "http"  => …,
    "connector"         => out.push(Box::new(ConnectorChannel::from_decl(d)?)),  // ← the one arm
    "slack"             => …,   // deleted; see below
    "cli" | "a2a"       => …,
    other => anyhow::bail!("unknown channel kind `{other}` …"),
}
```

`ConnectorChannel::from_decl` is decl-only, exactly like the arms beside it, and it refuses:

| refusal | why it is a load error and not a runtime one |
|---|---|
| no manifest for `connector` | the operator named a connector this host has not installed |
| no binding named `binding` in it | typo, or a binding removed by a connector upgrade |
| `transport` the arm cannot serve | a `poll` binding needs a `schedule` channel + `trigger`, not this |
| `verification` unset on a `webhook` binding | **the load-time refusal this repository already makes, reproduced against the file** |
| a credential the binding names with no `credentials` entry | otherwise a signature check fails open or a reply 401s on first delivery |
| an `allow` key that is not a declared payload symbol | a typo would silently allow everyone |
| a `payload` path that fails `validate_path` | the same grammar the loader enforces (`crates/connector-spec/src/inbound.rs:385`) |
| a `reply.bind` naming a payload symbol the map does not declare | a dangling reply |

One refusal that **cannot** happen in `build_channels`: *does a tool exist for the reply operation?*
That needs the registry, which lives on the `App` (`crates/flux-app/src/app.rs:458`). It is asserted
in `serve` (`crates/flux-channels/src/host.rs:21`) before any channel task is spawned — flux already
splits construction this way for `a2a`, which is built from the live `App` rather than the decl
(`crates/flux-channels/src/adapters/mod.rs:62`, `host.rs:37-40`).

## The reply is an operation call — this is the crux

`Channel::start` receives one seam, `Arc<dyn Deliverer>`
(`crates/flux-channels/src/channel.rs:22`), and `Deliverer` has exactly one method: `deliver(label,
payload) -> Vec<JourneyRun>` (`crates/flux-channels/src/deliver.rs:13-15`). There is no way for a
channel to call an operation, which is precisely why `slack.rs` opens its own Slack client.

The `send` op is not the answer either: it records the message and **prints only for a `cli`
channel** (`crates/flux-app/src/ops.rs:154-167`, the `is_cli_channel` branch at `:163`). A journey
that "replies" through `send` on a Slack channel writes to a log and nothing else.

So the seam widens by exactly one **defaulted** method:

```rust
#[async_trait]
pub trait Deliverer: Send + Sync {
    async fn deliver(&self, label: &str, payload: Value) -> anyhow::Result<Vec<JourneyRun>>;

    /// Call a registered operation through the host's full safety envelope.
    /// Defaulted to a refusal so every existing test double compiles unchanged, and so a
    /// deliverer that cannot dispatch says so loudly instead of dropping the reply.
    async fn call_operation(&self, _op: &str, _params: Value) -> anyhow::Result<Value> {
        anyhow::bail!("this deliverer cannot call operations")
    }
}
```

`AppDeliverer` implements it over a new `App::call_op`, which builds an executor from the App's
shared `ExecutionEnvironment` — the same template every journey executor is derived from
(`crates/flux-app/src/app.rs:1605`, `ExecutionEnvironment::into_executor` at
`crates/flux-runtime/src/lib.rs:2579`) — and dispatches through `Executor::dispatch`
(`crates/flux-runtime/src/lib.rs:3558`, *"Run a tool call through the full safety envelope"*).

Three properties follow, and they are the reason this shape was chosen over any other:

- **`Channel::start`'s signature does not change.** A defaulted method on a trait beats a new
  parameter on every adapter.
- **The reply traverses authorization → approval → guarded IO.** flux's own operating contract says
  *"Every tool runs through `Executor::dispatch` … the dispatcher is the policy/approval/redaction
  gate"* (flux `AGENTS.md`, non-negotiable conventions). A channel that posted to Slack outside the
  executor would be a second, unpoliced request path — which is exactly what `slack.rs:155-161` is
  today.
- **flux-channels may take the dependency.** It is L6; `flux-runtime` is L2
  (`crates/flux-codegate/src/lib.rs:44,54`), and the layering rule permits it. Today `flux-runtime`
  is only a *dev*-dependency of `flux-channels` (`crates/flux-channels/Cargo.toml`), so this
  promotes it — and the seam is deliberately typed in `serde_json::Value` + `anyhow::Result` so
  `flux-channels` need not name `ToolResult` at all.

### The reply's permission envelope is *narrower* than a journey's

A journey's grants come from its declared capabilities, defaulting to a legacy allow-list
(`crates/flux-app/src/app.rs:1611-1618`). A channel reply is not a journey and must not inherit that.

**Invariant: the reply executor's allow-list contains exactly one entry — the tool for the binding's
declared reply operation.** Host `deny` rules still win, and the App's `Approver` is unchanged. A
manifest that was tampered with after publication can therefore change *which vendor endpoint* the
reply hits, but it cannot reach any other op in the registry, and it cannot reach a host the
connector's `http_hosts` did not declare (that gate is the Tool's, per
[connector-tool-pack.md](connector-tool-pack.md)).

Stated as the failure it prevents: without this, installing a connector whose manifest names
`command.invoke` as its reply operation would be remote code execution behind a webhook.

### Composing with the Tool pack rather than competing with it

The tool the reply dispatches is the one
[connector-tool-pack.md](connector-tool-pack.md) registers for that operation — the pack's dotted
projection of `slack-chat-post-message` (its naming rule is C-114's, not this design's). That is the
composition the two designs owe each other:

- The pack owns request construction, credential resolution, redaction, and the **mirrored network
  gate** (`permission_subjects` + `NetworkFetch`, which delegation to
  `flux_web::http::HttpRequestTool::execute` would otherwise lose —
  `crates/flux-web/src/http.rs:118,126`).
- This design owns inbound: transport, verification, discrimination, payload mapping — and then
  hands one `(name, params)` pair to the executor.

**Neither duplicates the other's request path.** If the reply had its own HTTP client, the connector
channel would be the second unpoliced egress the pack design exists to avoid.

The alternative considered was `flow_run` over the connector's stored composite op
(`crates/flux-tools/src/flows.rs`, which *"runs a named flow … through the engine's depth-guarded
authored flow host, so it inherits the approval + IO envelope"*). It is a real envelope and it needs
no pack — but the composite cannot make a live call at all until the `$auth` seam ships
([auth-seam.md](auth-seam.md)), and a stored flow resolves by file stem rather than by a
registry name that can be permission-scoped to one entry. Recorded as the fallback, not the plan.

### `Reply::result` — the field no path can supply

`Reply::result` (`crates/connector-spec/src/inbound.rs:249`) names the parameter that carries the
journey's own output, because no dotted path into the triggering event reaches it. flux fills it from
the `Vec<JourneyRun>` that `deliver` returns, joined on newlines and skipping empties — which is
literally what `slack.rs:143-148` already does. A binding with no `result` and a reply whose required
parameters are all bound is legal (a fire-and-forget acknowledgement); a binding whose reply produces
an empty string sends nothing, as today (`slack.rs:149`).

## Verification: raw bytes, before parsing

`webhook.rs` destructures the request with axum's `Json<Value>` extractor
(`crates/flux-channels/src/adapters/webhook.rs:86`). **By the time `handle` runs, the raw bytes are
gone.** There is no HMAC path anywhere in the file; the only check is an optional static bearer
(`:88-97`) — which no vendor sends.

So the generic `connector` kind cannot be built on the existing webhook handler. It needs a primitive
this design **consumes rather than restates** — the one [C-64](../stories/C-64-flux-webhook-seam.md)
specified and [verified-webhook-seam.md](verified-webhook-seam.md) records:

- the request body captured as `Bytes` and verified **before** it is parsed;
- one parameterized HMAC over `{digest, encoding, prefix, signed-template, timestamp-selector,
  tolerance}` — the four-axis collapse modelled at `crates/connector-spec/src/inbound.rs:128`
  (`HmacSpec`);
- constant-time comparison — flux already has `constant_time_eq`
  (`crates/flux-channels/src/adapters/webhook.rs:123`), so this is a reuse, not an invention;
- **fail closed with zero delivery**: the assertion is on the deliverer's delivery count, not on the
  response status.

**Those are filed on flux's board as `C-291` and `C-292` (epic `verified-webhook-channel`), and this
epic depends on them rather than duplicating them.** The one thing this design adds is *where the
parameters come from*: C-291 has an operator hand-writing a `verify` record in the program; the
connector kind reads the same parameters out of the manifest's `HmacSpec`. Same verifier, two
declaration sources — and the connector source is the one that cannot be weakened by hand.

Three refusals ride on it, all reproductions of loader rules that already hold here:

- `verification` unset on a `webhook` binding → refuse at load. Silence is never a verification
  answer.
- `verification = "none"` on a `webhook` binding → serve it, and **say so**. C-291 goes further for a
  hand-declared channel — a non-loopback bind must state one or the other — and a connector channel
  inherits that rule unchanged, because the manifest always states one.
- a `signed` template interpolating `{timestamp}` with no `tolerance` or no timestamp selector →
  refuse. A host left to guess falls back to its own clock, which verifies nothing. (C-291 additionally
  refuses a **body-sourced** timestamp selector as unimplementable-by-construction; this repository's
  loader should adopt the same refusal — recorded as a finding below.)

## Routing: a fully-qualified trigger label, and no globbing

flux matches a trigger by **exact string equality** — `self.program.triggers.iter().filter(|t| t.on
== label)` (`crates/flux-app/src/app.rs:1108`). There is no globbing and no prefix match.

flux's `C-294` already owns the general mechanism (a `discriminator` on a webhook channel firing
`"<channel>.<event>"`, exact matching kept, no globbing, an unmatched label a logged no-op). **This
design does not restate it; it narrows it for the connector kind.**

**Decision: do not add globbing.** The label a connector channel fires is:

```
"<channel>.<discriminator value>"   when the discriminator resolves to a DECLARED event
"<channel>"                          when the binding declares no discriminator
```

so `trigger on "support.app_mention"` works, and `N` declared events means `N` trigger declarations.
Three reasons that is the right trade and not merely the cheap one:

1. **Prefix matching makes trigger resolution ambiguous.** Two patterns matching one label have no
   defined precedence, and flux's trigger table is a flat `Vec` filtered by equality. Adding globbing
   is a language change to fix a verbosity complaint.
2. **The verbosity is generable.** The trigger block for a binding is derivable from its declared
   event list; this repository can render it into the connector's docs page and catalogue entry as a
   paste-ready snippet. That is a rendering, not an emission — a binding still emits nothing into the
   `.flux` module.
3. **An undeclared discriminator value must not mint a label.** This is the narrowing the connector
   kind adds to C-294. There, a discriminator value *"that would produce a label with unexpected
   characters is sanitised or refused" —* a character-level rule, because a hand-declared channel has
   no list of legal events. A binding **does**: `ChannelBinding::events`
   (`crates/connector-spec/src/inbound.rs:322`) is a closed set. So the rule strengthens from
   sanitising to membership: a value not in that list is a **logged no-op**, never a label of its own
   and never a fallback to the bare channel name. Without it a vendor names flux's trigger labels,
   and sanitising the characters does not stop that.

An event whose label matches no trigger is likewise a logged no-op, not an error, for the same
reason C-294 gives — vendors send event types nobody subscribed to, and a 500 teaches them to retry
forever.

### `EventDecl::when` — declared, and the one place this design finds a real gap

`EventDecl::when` (`crates/connector-spec/src/inbound.rs:222`) is *"field equalities that narrow a
coarse vendor event into this one"* — GitHub's single `issues` event with an `action` field becoming
`issues.opened`. flux matches it after the discriminator, appending nothing to the label: `when`
selects *which declared event* a delivery is, and the event's own name is what reaches the label.

**v1 supports `const` equality only.** That covers the case the IR documents. It does **not** cover
absence — "this is the `message` event only when `subtype` and `bot_id` are absent" — which is
exactly Slack's loop guard (see the accounting below). That limitation is named, not discovered
later.

## Allow-lists are operator config, deliberately

`allow_users` / `allow_channels` live on `SlackSettings`
(`crates/flux-channels/src/config.rs:100,103`) and gate delivery at
`crates/flux-channels/src/adapters/slack.rs:131,183-187`. They are a **deployment policy about who
may trigger this agent**. A vendor spec cannot know them, a published manifest must not carry them,
and a connector upgrade must not be able to change them. They stay on the flux side.

The generic form keeps them generic without inventing new connector IR: the `allow` block's **keys
are payload symbols the binding declares**, and its values are this deployment's ids.

```
allow { user = ["U0123ABCDEF"], channel = ["C0123ABCDEF"] }
```

- `user` and `channel` are declared at `providers/slack.toml:441-446`, so the keys are checked
  against real data and a typo is a **load error**, not a filter that silently allows everyone.
- An empty or absent list allows everything, matching today's behaviour exactly
  (`slack.rs:183-187`). That is a permissive default and it is stated, not implied.
- The mechanism is vendor-neutral: a GitHub binding declaring `sender` and `repo` gets
  `allow { sender = [...], repo = [...] }` with no new code.

The alternative — a `[channels.identity]` block in the manifest declaring which symbols are
identity-bearing — was considered and **deferred**. It buys a renderer the ability to label the field
"Allowed users" instead of "user", and it costs new IR, a new refusal, and a new way for a manifest
to influence an access decision. Revisit it when a UI needs it, not before.

## Can `adapters/slack.rs` be deleted? An honest accounting

Every behaviour in that file, and where it goes:

| behaviour (`crates/flux-channels/src/adapters/slack.rs`) | declared home | status |
|---|---|---|
| payload map — `text`/`user`/`channel`/`thread`/`conversation` (`:172-180`) | `[channels.payload]`, `providers/slack.toml:441-446` | **declared** |
| `conversation` = thread ts else channel (`:167-169`) | `conversation = "event.thread_ts"` (`slack.toml:446`) | **partial** — no `coalesce`; a non-threaded message gets no conversation and runs one-shot (`crates/flux-app/src/app.rs:1563-1567`). Recorded gap in [channel-bindings.md](channel-bindings.md) |
| reply — `chat.postMessage` with `channel` + `thread_ts` (`:150-154`) | `[channels.reply]` + `[channels.reply.bind]`, `slack.toml:448-454` | **declared** |
| reply text = joined `JourneyRun` results (`:143-148`) | `Reply::result = "text"` (`slack.toml:450`) | **declared** |
| allow-lists (`:131,183-187`) | flux channel settings, `allow { … }` | **flux, deliberately** |
| bot/subtype loop guard (`:109`) | `EventDecl::when` | **GAP — see below** |
| Socket Mode connection loop (`:56-82`) | a flux transport under the binding driver | **flux, and it must be ported** |
| event-type dispatch (`:97-127`) | `discriminator = { source = "body", name = "event.type" }` (`slack.toml:391,419`) | **declared** |

### The two gaps, both in *this* repository

**1 · The loop guard is documented, not declared.** `providers/slack.toml:361-363` says `bot_id` and
`subtype` *"are the loop guard … declared here so that the condition is visible in the schema rather
than only in adapter code"* — and they are declared in `schema`, which describes the payload, not in
`when`, which selects the event. A connector-driven `message` binding would therefore deliver flux's
own replies back to flux and recurse. Worse, `when` cannot express *absence*, which is the shape the
guard needs.

*Consequence, and the epic's exit condition:* the connector channel ships with Slack's `app_mention`
usable and `message` **not** usable. That costs nothing today — `providers/slack.toml:359` already
sets `default = false` on `message`, for the unrelated firehose reason. Two follow-ups belong on
**this** board: absence matching in `when`, and moving `bot_id`/`subtype` from `schema` into `when`.

**2 · The endpoint challenge is undeclarable.** Slack's Events API answers a `url_verification` POST
by echoing `challenge`, and `providers/slack.toml:465-474` documents it as a *manual* setup step
(*"wait for Slack's `url_verification` challenge to pass"*) because Slack publishes no
`events.subscribe` method. `ChannelBinding` has **no `challenge` field**
(`crates/connector-spec/src/inbound.rs:306`). Until it does, a webhook-transport binding cannot
complete registration with any vendor that handshakes — the endpoint has to answer live, and nothing
tells flux what to echo.

flux's side of this is **already filed** — `C-293` gives `channel webhook` a declared `challenge` that
is answered without waking an agent, covering Slack's body-borne `url_verification` and Meta's GET
`hub.challenge`. The gap is on **this** side: there are no manifest parameters to feed it, so a
connector channel would have to hard-code a vendor's handshake, which is the thing this epic exists
to delete.

*Consequence:* the `events-api` binding is not registerable, so **the Slack Events API cannot replace
Socket Mode yet**, and `adapters/slack.rs` cannot be deleted by pointing operators at the webhook
transport. A third follow-up belongs on this board: a declared `[channels.challenge]`
(`{ when: Selector, echo: Selector }`) that projects onto C-293's declaration.

### So: yes, but only via the socket transport

The answer to *"can `slack.rs` be deleted without losing behaviour"* is **yes for behaviour, no for
deployment shape** — unless Socket Mode is ported as a transport. So it is ported:
`adapters/slack.rs` becomes `transports/slack_socket.rs`, still feature-gated on `slack` and still
carrying `slack-morphism`, holding the connection loop and **nothing else** — no payload map, no
reply, no allow-list, no dispatch. It hands raw event JSON to the same binding driver the webhook
transport feeds. Roughly 40 of 217 lines survive, and every line that this repository already
compiles is gone.

That is what makes the acceptance honest: the vendor SDK does not vanish, because Socket Mode is a
vendor protocol and no manifest can describe a WebSocket handshake. What vanishes is the *behaviour*
— and behaviour is exactly what a binding declares.

## The delivery envelope — not settled here

`flux_app::Event` is `{ label, payload }` and nothing else (`crates/flux-app/src/bus.rs:115-118`).
There is no id, no timestamp, no source and no verified flag. And `seed_payload`
(`crates/flux-app/src/app.rs:1988`) binds the whole payload to `$input` **and every top-level field
to its own symbol** — so anything a channel adds to the payload becomes a flow symbol that can
collide with a mapped one.

This design adds no envelope of its own, and it does not decide the question — flux's `C-295` owns it,
and [C-85](../stories/C-85-delivery-envelope.md) owns this side's consequences. Two positions it does
take, because a connector channel would otherwise have to invent them:

- `delivery_id` (`crates/connector-spec/src/inbound.rs:344`) goes wherever C-295 puts envelope data,
  and until that lands it goes nowhere. A payload key that a vendor field can shadow is not a dedupe
  key.
- A `verified` flag is deliberately **never** payload-derived. C-295 makes the same call
  (`payload_cannot_forge_the_verified_flag`), and the connector kind is the reason it matters here:
  the binding's tri-state verification is a published, validated fact, and normalising it away at
  delivery would make this repository's invariant true and useless.

## What flux must add, in one list

Numbers 1–3 are **already filed** by the verified-webhook seam (`C-291`…`C-295`, epic
`verified-webhook-channel`); this epic depends on them. 4–7 are what this design adds.

1. Raw request bytes captured before parsing, plus one parameterized, constant-time, replay-bounded
   HMAC verifier. *(`C-291`, `C-292`.)*
2. A declared endpoint-challenge hook, answered without waking an agent. *(`C-293` — and it is
   **blocked on this repository declaring the parameters**, see the gaps above.)*
3. Discriminator → fully-qualified trigger label, exact matching kept. *(`C-294`.)* A delivery
   envelope a payload cannot forge. *(`C-295`.)*
4. `Deliverer::call_operation` (defaulted to a refusal) and `App::call_op`, dispatching through
   `Executor::dispatch` under an allow-list of exactly one op.
5. A `connector` arm on `build_channels`: manifest resolution through `flux_system::System`, binding
   load, and every load-time refusal in the table above — including the narrowing of C-294's
   discriminator rule to the binding's closed `events` set, and `EventDecl::when` matched by `const`
   equality.
6. The reply wired to the Tool pack's registered operation, and `adapters/slack.rs` reduced to a
   transport that carries the Socket Mode connection loop and nothing else.
7. Operator allow-lists keyed on the binding's declared payload symbols, with an unknown key a load
   error.

## What this repository owes, in one list

Designing the flux side surfaced four gaps on **this** side. None of them belongs to C-84, and all
four are cheap to state and easy to lose:

1. **`EventDecl::when` cannot express absence.** Needed for Slack's loop guard, and it is the reason
   the connector channel ships `app_mention`-only. Today `when` is field → `JsonSchema`
   (`crates/connector-spec/src/inbound.rs:222`).
2. **`providers/slack.toml` declares the loop guard in the wrong field.** `bot_id`/`subtype` are in
   `schema` (`:364`), which describes the payload; the guard needs them in `when`, which selects the
   event. The comment at `:361-363` says they are declared *"so that the condition is visible … rather
   than only in adapter code"* — it is visible, and it is not yet actionable.
3. **`ChannelBinding` has no `challenge`.** flux's `C-293` will answer a declared handshake; nothing
   declares one. Without it no webhook-transport binding is registerable with a handshaking vendor.
4. **The loader should refuse a body-sourced verification timestamp.** flux's `C-291` refuses it as
   unimplementable by construction — honouring it would require parsing before verifying — and
   `HmacSpec::timestamp` is a full `Selector` here (`crates/connector-spec/src/inbound.rs:151`), so a
   provider TOML can currently declare one this seam can never honour. Refuse it at the loader, where
   every other rule of this shape lives.

Already recorded elsewhere and unchanged: the `conversation` coalesce gap
([channel-bindings.md](channel-bindings.md)) and the delivery envelope
([C-85](../stories/C-85-delivery-envelope.md), now with flux's `C-295` as its counterpart).

## Invariants

1. **The reply is a dispatch, never a request.** A connector channel opens no HTTP client of its own.
   If it cannot answer through an op the registry already holds, it does not answer.
2. **The reply's allow-list is exactly one op.** A manifest cannot widen it.
3. **Verification precedes parsing, and a failure delivers nothing.** Asserted on the delivery count,
   not the status code.
4. **A manifest never carries an access decision.** Allow-lists, addresses and secret values are the
   operator's; the manifest carries names and shapes.
5. **An undeclared event never becomes a trigger label.** The binding's `events` list is the closed
   set of labels a vendor can reach.
6. **Every refusal this repository makes at load, flux makes again against the file it reads.** The
   manifest is a published artifact, and a published artifact can be edited.

## Alternatives considered

- **flux reads nothing; this repository ships a `Channel` implementation.** Symmetrical with the Tool
  pack, and rejected: a `Channel::start` *is* a listener loop, and shipping one here would give this
  repository a server — the non-goal `docs/vision.md` names. The Tool pack is safe precisely because
  a `Tool` opens no socket; a `Channel` does.
- **One flux adapter per vendor, generated.** It reproduces the closed match with more entries, needs
  a build step in flux for every connector, and puts vendor code back in flux — the cost principle 1
  exists to eliminate.
- **The reply as a new `channel.reply` op a journey calls.** Moves the binding's outbound half into
  every operator's flow, which is the hand-maintained integration this epic removes, and gives the
  reply a journey's full grants instead of one op's.
- **Prefix/glob trigger matching.** See "Routing" — a language change with undefined precedence, to
  avoid writing N trigger lines that are mechanically generable.
- **Verification declared in the program rather than read from the manifest.** That is C-64's shape
  and it stays valid for a hand-declared webhook. For a connector channel it would let an operator
  weaken a scheme the vendor publishes and this repository validated, which is the bypass constraint
  3 forbids.
