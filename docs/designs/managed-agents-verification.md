# Design: Managed Agents — the two event vocabularies, and whether `HmacSpec` can express the webhook signature

**Status:** accepted (a negative result, and a gap filed) · **Pillar:** Spec ·
**Epic:** `anthropic-managed-agents` · **Story:** [C-446](../stories/C-446-managed-agents-events-and-verification.md) ·
**Parent:** [anthropic-managed-agents.md](anthropic-managed-agents.md) ·
**Extends:** [inbound-events.md](inbound-events.md), [channel-bindings.md](channel-bindings.md)

> **Two kinds of claim, labelled separately throughout.**
>
> - **Vendor facts** come from the bundled `claude-api` skill reference — `shared/managed-agents-webhooks.md`
>   and `shared/managed-agents-events.md`, read **2026-08-02**. Nothing here was answered from
>   recollection and nothing was fetched from the network. No Managed Agents document is vendored under
>   `specs/`, so that reference is the whole of what this repository knows.
> - **Repository facts** were measured this session with the command quoted beside them. Per
>   `AGENTS.md` § *Before you assert anything*, re-measure before quoting: symbol line numbers move.

## The verdict, in one line

**Not expressible.** The Managed Agents webhook signature cannot be declared in `HmacSpec` as it
stands, and — separately and more immediately — it cannot be declared *at all* today, because the
model has no way to say the one true thing about it. [C-447](../stories/C-447-verification-scheme-is-unmodelled.md)
is the gap.

Read § *Two independent gaps* for which is which. The distinction matters: one is conditional on a
vendor fact this repository does not yet hold, and the other holds regardless.

## What the vendor publishes about the webhook signature

Sourced from `claude-api` → `shared/managed-agents-webhooks.md`, read 2026-08-02.

| the Acceptance asks for | what the reference states |
|---|---|
| **headers** | Three, on every delivery: `webhook-id`, `webhook-timestamp`, `webhook-signature`. |
| **freshness window** | Verification "rejects payloads more than **~5 minutes** old". |
| **secret** | A `whsec_`-prefixed, 32-byte signing secret, shown once at endpoint creation. The SDK reads it from `ANTHROPIC_WEBHOOK_SIGNING_KEY`. |
| **signed content** | **Not stated.** |
| **digest** | **Not stated** beyond the word "HMAC-signed". |
| **encoding** | **Not stated.** |
| **header value format** | **Not stated** — no prefix, no version tag, no delimiter is published. |

Four further delivery facts the reference does state, because they bear on the binding rather than on
the signature:

- **Registration is Console-only.** *"Console → Manage → Webhooks. There is no programmatic
  endpoint-management API yet."*
- **`webhook-timestamp` is re-stamped on every delivery attempt**, and times the *attempt*; the
  payload's own `created_at` times the *event*.
- **Deliveries are at-least-once and unordered.** Up to three attempts with jittered backoff between
  5 and 120 seconds, then the event is **dropped**. The top-level `id` — equal to the `webhook-id`
  header — is per *event*, not per delivery, and is the documented dedupe key.
- **Payloads are thin**: `data.type` plus resource ids, and the caller re-fetches the resource.

**The reference does not document the signature construction because it directs callers to
`client.beta.webhooks.unwrap()` instead**, and it says so explicitly: *"don't hand-roll verification
against a single `X-Webhook-Signature` header, which is not the wire format."* That is a sound
instruction to an SDK user and it is precisely the wrong shape for this repository, which ships no
verifier and must **declare** the parameters a host will use.

### What this repository must not do about the missing three

There is a widely-implemented three-header convention that these header names belong to, and it would
be easy to fill `signed`, `algorithm` and `encoding` from it. **That would be inventing a verification
scheme**, which `AGENTS.md` forbids twice over — *"Refuse ambiguous or unsafe output"* and *"Never
present an unverified event as trusted"* — and C-188's rule states the cost exactly: *"The one thing
worse than Twilio having no binding is Twilio having a binding that reports success without verifying
anything."* A guessed `signed` template is worse than a guessed anything else in this model, because
every other check the loader makes still passes on it.

So the three unknowns are recorded as unknowns, and closing them is a vendor-document task, not a
design task.

## Field by field against `HmacSpec`

`HmacSpec` is `crates/connector-spec/src/inbound.rs:152` (measured 2026-08-02:
`grep -n "pub struct HmacSpec" crates/connector-spec/src/inbound.rs`). Its loader is
`validate_hmac`, `crates/connector-spec/src/provider.rs:3095`.

| field | fillable from the reference? | evidence |
|---|---|---|
| `header` | **yes** — `"webhook-signature"` | one header carries the digest, which is the shape `HmacSpec` models |
| `secret` | **yes** — an `[[auth]]` entry with `scheme = "signing"` | the `whsec_` secret is a verification secret, never sent outbound; `AGENTS.md` § Authentication contract |
| `timestamp` | **yes** — `{ source = "header", name = "webhook-timestamp" }` | header-sourced, which is the only source the loader accepts (`provider.rs:3163` refuses `body`) |
| `tolerance` | **yes** — `"5m"` | ~5 minutes; `parse_tolerance` (`inbound.rs:612`) accepts `5m` and caps at 1h |
| `timestamp_format` | **unknown** | the reference does not state the timestamp's spelling. The field defaults to `unix_seconds`, so an unstated format would be *guessed by omission* |
| `algorithm` | **unknown** | "HMAC-signed"; no digest named |
| `encoding` | **unknown** | no encoding named |
| `prefix` | **unknown** | no header-value format published |
| `signed` | **unknown, and structurally doubtful** | see below |

**Four of nine fields are fillable; five are not** (counted 2026-08-02:
`sed -n '152,237p' crates/connector-spec/src/inbound.rs | grep -c "^    pub "` → `9`). One of the five
is `signed`, the field `SIGNED_PLACEHOLDERS`' own doc comment calls *"the rule the whole struct rests
on"*. Note `timestamp_format` is in the unfillable five for a quieter reason than the others: it is
`Option`, absent means `unix_seconds`, so leaving it out does not refuse — it **guesses by omission**.

### Why `signed` is doubtful even once the vendor documents it

`SIGNED_PLACEHOLDERS` (`inbound.rs:541`) is closed at four names: `{body}`, `{sorted_form}`,
`{timestamp}`, `{url}`. The loader refuses any other name (`provider.rs:3108-3118`), by design.

The scheme sends **three** headers. Two of them have homes: `webhook-signature` is the digest,
`webhook-timestamp` is `{timestamp}`. `webhook-id` has none. It is a per-delivery identifier read
from a header, and if it enters the signed string there is no placeholder for it and no general axis
for one — `Selector` (`inbound.rs:100`) can *address* an arbitrary header, and `signed` cannot
*interpolate* a selector. A third header that the scheme sends and the signature does not cover would
be unusual, but this repository has not read a vendor statement either way, so the honest status is
**doubtful, not decided**.

This is the same shape twice before. C-141 found it on Stripe's composite `Stripe-Signature`: *"a
`Selector` addressing a whole header, so no component can be taken out of that list… That needs a new
extraction axis."* C-188 found it on Twilio and closed it by **widening the vocabulary** rather than
adding a field. The Managed Agents case is a third instance and would be answered the same way — but
only after the vendor states what is signed, and this design will not pre-commit the shape of an
answer to a question nobody has read the facts for.

## Two independent gaps

**Gap A — `signed` cannot interpolate a selected per-delivery value.** Conditional on the vendor
documenting that `webhook-id` is signed. The precedents (C-141, C-188) make the remedy predictable —
a fifth placeholder, subject to `PAYLOAD_PLACEHOLDERS` (`inbound.rs:556`) still being able to answer
"does the payload reach the digest" — but the trigger is a vendor fact, not a design decision. **Not
filed as a story**, because filing a story against an unverified premise is exactly the dispatch
failure `AGENTS.md` records against C-413.

**Gap B — the model cannot say "the vendor signs, and we cannot yet model how".** Unconditional, and
the reason nothing can be declared today. `ChannelBinding::verification` is tri-state
(`inbound.rs:395-406`, enforced at `provider.rs:3047`):

| state | what it asserts |
|---|---|
| unset | legal for `socket`/`poll`; a **loader error** for `webhook` |
| `Some(None)` | *"the vendor publishes no signature"* |
| `Some(Hmac(..))` | *"here are the parameters"* |

Managed Agents is none of the three. It is not unset — that is refused, correctly. It is not
`Hmac(..)` — four parameters are unknown. And **`verification = "none"` would be a false statement,
not a conservative one**: the vendor does sign every delivery, and a manifest saying otherwise tells a
host there is nothing to check. That is worse than the loader refusal it would silence, and it is the
misuse the story named in advance.

The three states answer *"what does the vendor publish?"*. The missing fourth answers *"can this
repository express it?"* — and those are different questions that have been collapsed into one field.
Filed as **[C-447](../stories/C-447-verification-scheme-is-unmodelled.md)**.

**Until C-447 lands, the Managed Agents webhook binding is not declarable, and the correct interim
state is to declare the events and no `[[channels]]` binding for them** — exactly what Twilio did
between C-109 and C-188, and what C-60 did for Stripe. The events are still useful: they reach the
manifest and the catalogue, and a binding can be added the day the gap closes.

## The two event vocabularies

**These are two different namespaces and nothing maps between them.** They are enumerated separately
below because a reader who sees `session.status_idle` on one side and `session.status_idled` on the
other will otherwise assume a typo. Both are from the `claude-api` reference, read 2026-08-02 —
`shared/managed-agents-events.md` for the stream, `shared/managed-agents-webhooks.md` for the webhook.

### The SSE stream vocabulary — `GET /v1/sessions/{id}/events/stream`

Received:

| group | types |
|---|---|
| agent | `agent.message`, `agent.thinking`, `agent.tool_use`, `agent.tool_result`, `agent.mcp_tool_use`, `agent.mcp_tool_result`, `agent.custom_tool_use`, `agent.thread_context_compacted` |
| session status | `session.status_running`, `session.status_idle`, `session.status_rescheduled`, `session.status_terminated`, `session.error` |
| spans | `span.model_request_start`, `span.model_request_end`, `span.outcome_evaluation_start`, `span.outcome_evaluation_ongoing`, `span.outcome_evaluation_end` |
| multiagent | `session.thread_created`, `session.thread_status_running`, `session.thread_status_idle`, `session.thread_status_rescheduled`, `session.thread_status_terminated`, `agent.thread_message_sent`, `agent.thread_message_received` |

The stream also **echoes back** the client-sent events (`user.message`, `user.interrupt`,
`user.tool_confirmation`, `user.custom_tool_result`, `user.define_outcome`), and carries two
**stream-only, never-persisted** preview frames, `event_start` and `event_delta`, which the reference
calls out as the one exception to the `{domain}.{action}` naming convention.

Sendable (the inbound half's counterpart, not events a binding carries): `user.message`,
`user.interrupt`, `user.tool_confirmation`, `user.custom_tool_result`, `user.define_outcome`,
`system.message`.

### The webhook `data.type` vocabulary

A **thin** envelope — `{type: "event", id, created_at, data: {type, id, organization_id, workspace_id}}` —
so the discriminator is `data.type` in the body, not a header.

| group | types |
|---|---|
| session | `session.status_scheduled`, `session.status_run_started`, `session.status_idled`, `session.status_rescheduled`, `session.status_terminated`, `session.updated`, `session.deleted` |
| session threads | `session.thread_created`, `session.thread_idled`, `session.thread_terminated`, `session.outcome_evaluation_ended` |
| vaults | `vault.created`, `vault.archived`, `vault.deleted`, `vault_credential.created`, `vault_credential.archived`, `vault_credential.deleted`, `vault_credential.refresh_failed` |
| agents | `agent.created`, `agent.updated`, `agent.archived`, `agent.deleted` |
| deployments | `deployment.created`, `deployment.updated`, `deployment.paused`, `deployment.unpaused`, `deployment.archived`, `deployment.deleted`, `deployment_run.started`, `deployment_run.succeeded`, `deployment_run.failed` |
| environments | `environment.created`, `environment.updated`, `environment.archived`, `environment.deleted` |
| memory stores | `memory_store.created`, `memory_store.archived`, `memory_store.deleted` |

### How different the two really are

The reference states the separation itself: *"These are **webhook** `data.type` values — a separate
namespace from SSE event types… Don't reuse SSE constants in webhook handlers."* Three concrete
consequences for the declaration:

- **The near-collisions are the hazard, not the disjointness.** `session.status_idle` (stream) versus
  `session.status_idled` (webhook); `agent.thread_message_sent` versus nothing; `session.thread_created`
  is the one name that appears in **both** sets and means the same thing in each — which is exactly
  what makes the others look like typos.
- **The two describe different subject matter.** The stream is the *agent's turn* — messages, tool
  calls, model spans. The webhook is the *resource lifecycle* — sessions, agents, vaults, deployments,
  environments, memory stores. Most webhook types have no stream counterpart at all.
- **`EventDecl::name` admits `.`** (`AGENTS.md` § Member contract: a member name *"admits `-`, `_` and
  `.`, because an event keeps its vendor spelling"*), so both spellings survive verbatim into the
  member namespace, and the two similar names remain two distinct members. That is the model working;
  it is also why a reader needs this section.

## Two bindings over two event sets on one service is expressible

**Confirmed by reading the loader, not assumed.** `validate_channel_events`
(`crates/connector-spec/src/provider.rs:3011`) makes exactly two demands of a binding's `events`: each
name is declared by an `[[events]]` block, and each declared event's `service` equals the binding's
`service`. There is **no** rule that two bindings on one service share events, no rule that their
event sets intersect, and no rule that they partition — the partition invariant this repository does
enforce is over *operations* and *services* (`crates/connector-spec/tests/service_partition.rs`), not
over bindings and events.

So the shape the epic needs is already legal:

```
service = managed-agents
  [[events]]   × stream vocabulary        → carried by the `socket` binding
  [[events]]   × webhook data.type set    → carried by the `webhook` binding
  [[channels]] socket
  [[channels]] webhook
```

Slack proved two bindings on one service (`providers/slack.toml`, `socket` + `events-api`) but over
*one* event set — its two bindings both carry `["app_mention", "message"]`, which the file calls
*"same two events, same payload map, same reply, two transports"*. Managed Agents would be the first
connector with two bindings over **disjoint** vocabularies. That is new to the catalogue and **not**
new to the model, and the difference is worth stating plainly: nothing needs to change in
`connector-spec` for it.

One consequence to carry into the declaration story: the two bindings will need **different
`discriminator` selectors** — `{source = "body", name = "type"}` on the socket half, `{source =
"body", name = "data.type"}` on the webhook half — which `Selector` already expresses per binding.

**Measured 2026-08-02:** the catalogue ships 4 channel bindings and 8 events —
`grep -c "^\[\[channels\]\]" providers/*.toml` → `slack:2`, `twilio:2`;
`grep -c "^\[\[events\]\]" providers/*.toml` → `slack:2`, `stripe:4`, `twilio:2`.

## The socket binding states no verification, and stating one is an error

`validate_channel_verification` (`provider.rs:3047`) is stronger than the epic assumed. Unset
verification is not merely *permitted* on a `socket` binding — declaring **any** verification on a
non-`webhook` transport is a loader error: *"channel binding … states `verification`, which only the
`webhook` transport uses. A `socket` binding is authenticated by the credential that opens the
connection."*

So the socket half writes no `verification` key and carries the reason in a comment, in the form
`providers/slack.toml:450` already uses (verified this session:
`grep -n "No .verification.: nothing arrives unsolicited" providers/slack.toml`):

```toml
# No `verification`: nothing arrives unsolicited over a socket we opened and authenticated.
```

The reason holds here for the same structural cause and a different credential: the SSE stream is
opened by an outbound `GET` the caller makes with its own API key, so nothing arrives on it that the
caller did not ask for. Slack's is an app-level token; this is the connector's API key. The sentence
should say which — a comment that reads as copied from Slack teaches the next author that the rule is
a habit rather than a consequence.

## The reply is a declared operation, and one thing about it is unsettled

`POST /v1/sessions/{id}/events` (`SendEvents`) is the socket binding's outbound half. It must be an
ordinary `[[operations]]` entry of the same connector and the same service — the loader enforces
precisely that, and says why: *"A binding's reply is an ordinary operation of this same connector —
that is what makes it a composition rather than a second code path"* (`provider.rs:3250`, and the
service check immediately after). There is no parallel reply mechanism to propose and none is
proposed.

**The unsettled part, flagged rather than assumed.** `validate_channel_reply` also requires that
*every required parameter* of the reply operation is covered — by `[channels.reply.bind]` from the
`payload` map, or by `result` — because *"a partially bound reply would compile and then fail at the
first delivery"*. `SendEvents` requires a `session_id` path parameter, and **the SSE frames shown in
the reference carry no session identifier**: the documented shape is
`{"type": …, "id": "sevt_…", "processed_at": …}`, and the session id is knowledge the *caller* holds
because it opened that stream, not a field the event delivers.

If that holds against the full event schema, the reply cannot bind `session_id` from the payload and
the binding would be refused — a third gap, of a different kind: a value that is constant for the
*connection* rather than present in the *event*. It is the same shape as the SCHEMA GAP
`providers/slack.toml` records above its own binding, and the same rule applies — do not invent a
spelling for it here.

**This is an open question, not a finding.** The reference shows example frames, not a complete
schema, and an absence in an example is not an absence in the payload. The declaration story must
resolve it against a full event schema **before** it is dispatched, because an implementor who
discovers it mid-story is an implementor who will be tempted to invent the missing spelling.

## What this design settles, and what it does not

Settled:

- The webhook signature is **not expressible** today, for a reason that is a model gap rather than a
  research failure, and C-447 is that gap.
- `verification = "none"` is **refused as a workaround**, with the reason written down.
- The two event vocabularies are enumerated and are **separate namespaces**.
- Two bindings over two event sets on one service **needs no model change**.
- The socket half **must** omit `verification`, and its reply **must** be a declared operation.

Not settled, and deliberately left to whoever holds the facts:

- The signed string, the digest, the encoding, the header-value format, and the timestamp spelling —
  all vendor facts, none published in the bundled reference.
- Whether `webhook-id` is signed, and therefore whether Gap A is real.
- Whether the SSE event payload carries a session identifier, and therefore whether the reply binds.
