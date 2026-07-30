# Design: channel bindings — generalize a flux `channel` over a connector

**Status:** accepted (IR + loader landed) · **Pillar:** Spec (+ Codegen, Bridge) ·
**Epic:** `channel-bindings` · **Stories:** C-82 … C-85 ·
**Amends:** [inbound-events.md](inbound-events.md), [C-66](../stories/C-66-members-under-services.md)

## Why

[inbound-events.md](inbound-events.md) models an **event** — the vendor calls us — and stops there.
One level up sits the thing an operator actually declares, and today flux hard-codes it.

`flux-channels`' adapter dispatch is a **closed `match`** on a `kind` string; an unknown kind is a
load error. One of its arms, `slack`, is 218 lines of vendor-specific Rust at L6. That adapter's last
act is to build a `chat.postMessage` request by hand, from `channel`, `text` and `thread_ts`.

Those are exactly the three body parameters of `slack-chat-post-message`, an operation **this
repository already compiles** from `providers/slack.toml`. flux is hand-writing an outbound call the
connector already generates — which is precisely the hand-maintained integration cost principle 1
exists to eliminate, re-enacted on the inbound side.

## The sharpening: flux's channel kinds split three ways

"Everything that is a channel in flux is a provider here" is right for one of the five kinds. Taking
it literally for the rest would break the **no runtime** non-goal, so the split is worth stating
before anything is built on it.

| flux `kind` | What it actually is | Home |
|---|---|---|
| `slack` | **a vendor** — a spec, operations, credentials, event schemas | **here**, as a channel binding |
| `webhook` / `http` | a generic **transport** — the substrate a vendor binding rides on | flux |
| `a2a` | a **protocol**, not a vendor | flux; [C-46](../stories/C-46-generic-connectors.md) covers it as a *generic* connector |
| `schedule` / `cron`, `startup` | a **time or lifecycle source** — no vendor, no spec, nothing to compile | flux, permanently |
| `cli` | a host surface | flux, permanently |

**A channel is a transport (flux owns) plus a binding (a connector declares.)** Compiling the
scheduler here would make this repository a runtime, which the north star forbids.

## The idea: a binding is a composition, not a primitive

A channel binding needs no new primitive. It composes two things the IR already has:

- **inbound** — the [`EventDecl`]s of its service, which is C-59's work;
- **outbound reply** — an **operation of the same connector**, referenced by id and rendered as an
  oip, with its parameters filled from the inbound payload.

So `slack` becomes: transport `socket`, events `app_mention`/`message`, reply
`com.slack.api:v1#slack-chat-post-message` with `channel` and `thread_ts` bound off the message.
Nothing new is emitted into the module — the reply operation is already there.

### The journey's own output is not in the payload

The one field the payload map cannot supply is the most important one. A reply's *text* is what the
flow computed, and no dotted path into the triggering event reaches it. flux's adapter makes the same
split in code: it joins the `JourneyRun` results and passes them as `text`, while `channel` and
`thread_ts` come off the received message.

`Reply::result` is that line declared. A required parameter is covered by `bind` **or** by `result`,
and a parameter named by both is refused — one parameter carries one value.

## Shape

```toml
[[events]]
name = "app_mention"                    # the VENDOR's spelling, never respelled
schema = { type = "object", ... }

[[channels]]
name = "events-api"
transport = "webhook"                   # webhook | socket | poll
events = ["app_mention", "message"]
discriminator = { source = "body", name = "event.type" }
delivery_id   = { source = "body", name = "event_id" }

[channels.verification.hmac]            # the C-60 matrix, in the vendor's own parameters
algorithm = "sha256"
encoding  = "hex"
header    = "X-Slack-Signature"
prefix    = "v0="
signed    = "v0:{timestamp}:{body}"
timestamp = { source = "header", name = "X-Slack-Request-Timestamp" }
secret    = "slack.signing_secret"      # a CREDENTIAL NAME, never a value
tolerance = "5m"

[channels.payload]                      # Flux symbol -> dotted path into the vendor envelope
text = "event.text"
channel = "event.channel"
thread = "event.thread_ts"

[channels.reply]
operation = "slack-chat-post-message"
result    = "text"                      # carries the journey's output

[channels.reply.bind]
channel   = "channel"
thread_ts = "thread"
```

Note what is **absent**: no URL, no secret, no schedule. The endpoint address is the operator's
deployment detail, the secret is a reference the host resolves, and the loop driving a `poll` binding
is an operator's `channel schedule` + `trigger`.

### Reuse over invention

`payload` paths use the **existing dotted grammar** `Param::wire` already carries and `body_tree()`
already parses — `event.thread_ts`, not JSONPath `$.event.thread_ts`. A second path language in the
same repository would be the homegrown DSL principle 2 forbids.

## The model: three member kinds, one namespace

C-66 proposed `provider → service → (operation | event)`. This amends it to:

```
provider → service → (operation | event | channel)
```

with **one shared name namespace per service**. That settles C-66's open bullet — events and channels
reuse the existing `#name` fragment, so `Oip` needs no grammar change and no kind discriminator. Two
reasons it must be one namespace, and the second is load-bearing:

1. All three render into the same address space, and an oip carries no kind tag to tell two apart.
2. All three project into flux's declaration namespace — an operation is an `op` a model calls by
   name, an event is a trigger label — so a collision would resolve differently depending on which
   surface asked.

A **cross-kind** collision is refused here; a **within-kind** duplicate is reported by that kind's own
pass, so one problem produces one line.

### Member names are wider than operation ids

An event keeps its vendor name: Slack's really is `app_mention`, GitHub's really is `issues.opened`.
So a member name admits `-`, `_` and `.`. An operation id is *additionally* a declarable Flux symbol,
and that narrower rule stays where it belongs — `connector-flux` refuses an unspellable operation id
at emission. **This function guards the address; the emitter guards the declaration.**

## Polling: the cursor carries the correctness, because the schedule cannot

flux's cron is one in-process task per channel, UTC-only, and **missed-tick replay is a named
non-goal** of its own design (`../../flux/docs/designs/event-trigger-channels.md`). The durable path,
`schedule_wakeup`, is at-most-once and fires only on next session open — there is no proactive poller.

A restart therefore drops ticks and replays none of them. So `transport = "poll"` **requires** a
cursor operation rather than merely permitting one: the schedule cannot be trusted to have run, and
resuming from a recorded position is the only thing that makes a poll correct. `interval` is
advisory — a documentation hint and a starting value, never a guarantee about cadence.

This sharpens [C-63](../stories/C-63-poll-transport.md), which had the cursor as optional.

## One auth-model addition

`AuthScheme` gains `Signing`: a credential that is **never placed in a request**, only used to verify
an inbound one. It is the one deliberate divergence from `flux_plugin_protocol::AuthScheme` — every
other variant answers "where does this secret go on the way out", and a webhook signing secret has no
answer because it never goes out.

The alternative was a second credential namespace beside `[[auth]]`, and it is worse: a connector
manifest names every credential the connector requires (principle 5), and an operator provisioning one
would have had to know inbound secrets live somewhere else. One namespace, one list, one place to
look. The complement is enforced too — an operation cannot authenticate with a `signing` credential.

## Codegen — the strict split, unchanged

A binding **declares**; it never installs, and it emits **nothing into the module**. flux lifts `op`
declarations only from `~/.flux/flows`, while `channel` and `trigger` are Program members an operator
writes. So a binding reaches the manifest and the catalogue, and the emitter must refuse to dress one
up as a pollable op — the same rule C-61 states for events.

Asserted, not intended: `crates/connector-flux/tests/slack_connector.rs` pins the emitted module
byte-for-byte, and adding two bindings to `providers/slack.toml` changed it not at all.

## Invariants

1. **A binding holds completely or is refused.** A dangling reply, an unbound required parameter, a
   poll with no cursor: each builds, ships, passes every artifact check, and then fails on an
   operator's first real delivery. Every rule is a refusal.
2. **Silence is never a verification answer.** A `webhook` binding states an HMAC scheme or states
   `verification = "none"` deliberately; an unset one is refused. Never present an unverified event as
   trusted.
3. **Replay is bounded.** A `signed` template interpolating `{timestamp}` requires both a `tolerance`
   and a selector saying where the timestamp is read from — a host left to guess would fall back to
   its own clock, which verifies nothing.
4. **No secret in any artifact.** `secret` is a credential name; `crates/connector-cli/tests/site_catalog.rs`
   runs a build with a sentinel env value and asserts it appears nowhere.
5. **The two directions never share a credential.** A verification secret is `scheme = "signing"`;
   using an outbound credential to verify would spend the same value both ways.

## What this does not settle

- **The flux side.** `build_channels` still has one arm per vendor. Making it read a connector
  manifest binding — one generic `connector` kind instead of one arm per vendor — is C-84, and it is
  what retires `adapters/slack.rs`.
- **The delivery envelope.** `flux_app::Event` is `{ label, payload }` and nothing else — no id, no
  timestamp, no source, no verified flag. So "delivery id in the payload" stuffs envelope into
  payload, and it collides with `seed_payload`, which binds every top-level payload field as a flow
  symbol. C-85.
- **Codegen.** Bindings are in the IR and in the hash domain; publishing them into the manifest and
  `catalog.json` is C-83.
- **The conversation fallback.** flux keys a session on `thread_ts` when present and `channel`
  otherwise. A payload map binds one symbol to one path and has no `coalesce`. Recorded as a gap in
  `providers/slack.toml` rather than papered over with an invented spelling.

## Alternatives considered

- **A binding as a new primitive**, with its own event and reply machinery. Rejected: both halves
  already exist, and a third primitive would need its own address form, its own namespace rule and its
  own codegen path for no gain.
- **Provider-level `[[channels]]`**, beside C-59's `[inbound]`. Simpler now, but it cannot describe a
  multi-service vendor — the exact failure C-66 was filed to prevent for events, so it would have to
  move again immediately.
- **Leave channels entirely to flux.** Then every operator hand-writes each vendor's payload mapping
  and reply, which is the hand-maintained integration config principle 1 exists to eliminate, and
  `adapters/slack.rs` stays where it is.
- **A `signing` credential in its own namespace.** See above — it splits the manifest's credential
  list, which is the one place an operator looks.
