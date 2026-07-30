# Handoff: ready-to-paste flux stories for verified inbound webhooks

> **This file is a handoff artifact, not a tracked backlog.** Nothing in it is a story on *this*
> repo's board, and `/track:board` must never pick these up. Each block below is a complete story file
> destined for **`../flux`**'s `docs/stories/`. A human copies a block verbatim into
> `/home/timo/projects/flux/docs/stories/<id>-<slug>.md` and runs flux's own `/track:board`.
>
> Source design: [inbound-events.md](inbound-events.md) · Parent story:
> [C-64](../stories/C-64-design-verified-webhook-seam.md)

## Before you paste

- **IDs are provisional and the previous handoff's were wrong.** The `auth-seam` handoff claimed
  `C-266 … C-276`; by the time anyone looked, flux's fleet had consumed that entire range with
  unrelated work (C-266 sandbox CI, C-268 wasm epic, C-274 events sqlite, …) and flux's highest `C-`
  id was already **275**. Do not assume. Re-check immediately before pasting:

  ```bash
  ls ../flux/docs/stories | grep -oP '^C-\d+' | sort -t- -k2 -n | tail -1
  ```

  and renumber every block plus its cross-references.
- **Naming: this is `webhook signature verification`, never "the inbound auth seam".** flux already
  has a **done** `request-auth-seam` (`docs/designs/request-auth-seam.md`, D-64/D-68) covering inbound
  *bearer → principal* resolution. A story titled "inbound auth" will be read as a duplicate of
  shipped work and closed.
- **A design doc must exist in flux** if a block sets `design:`. Either port
  [inbound-events.md](inbound-events.md) into flux as `docs/designs/verified-webhook-channel.md`, or
  drop the `design:` line. Never leave a `design:` pointing at a file flux does not have.
- **Facts verified in flux** (re-grep by symbol, not line number):
  `crates/flux-channels/src/adapters/webhook.rs` — `WebhookSettings { addr, path, async, token }`, an
  optional **static bearer token**, and **no** HMAC/signature path anywhere in the file.
  `crates/flux-channels/src/adapters/mod.rs` — `build_channels` dispatches
  `"webhook" | "http"`, and an unknown kind is a hard error.
  `AppDeliverer` serializes deliveries behind a mutex, so a verified channel inherits the
  one-delivery-at-a-time property (flux story A-112 is the isolation follow-up).

## Sequencing

```
F-1  declarative `verify` block on `channel webhook`   ← the foundation; raw-body capture lives here
F-2  scheme matrix + constant-time compare + tolerance ← depends on F-1
F-3  challenge/handshake hook (answer without a turn)  ← independent of F-2, needs F-1
F-4  discriminator → trigger-label routing            ← depends on F-1
F-5  delivery id + dedupe surface                      ← depends on F-1
```

F-1 is the only hard prerequisite. F-2 through F-5 can run in parallel afterwards.

---

## F-1 — capture the raw body and add a declarative `verify` block

```markdown
---
id: C-NNN
title: A declarative `verify` block on `channel webhook`, over the raw request body
pillar: Core
status: backlog
areas: [flux-channels]
note: "channel webhook authenticates with an optional STATIC BEARER TOKEN and has no signature path, so a vendor that signs its payloads but cannot send a custom Authorization header has no authenticated route into flux at all"
---

# A declarative `verify` block on `channel webhook`, over the raw request body

## Goal
Let a `channel webhook` declaration carry a signature-verification scheme, so a signed vendor webhook
(GitHub, Stripe, Slack, Zendesk) has an authenticated path into flux. Today `WebhookSettings` is
`{ addr, path, async, token }` and the only check is a static bearer — which vendors do not send.

## Acceptance
- [ ] `WebhookSettings` gains an optional `verify` block: `scheme`, `algorithm`, `encoding`, `header`,
      optional `prefix`, a `signed` template over `{body}`/`{timestamp}`, optional `tolerance`, and a
      `secret` that is a **host-resolved reference** (`verify_secret secret "KEY"`), never a literal.
- [ ] **The raw body bytes are captured before parsing and verification runs against them.**
      Failing-first test `verify_uses_raw_body_not_reserialized`: a body whose JSON keys are reordered
      after parsing fails verification, proving the raw bytes are what is checked. Any
      normalize-then-verify path is a bypass, not a convenience.
- [ ] **Fail closed with zero delivery.** Test `bad_signature_delivers_nothing` asserts the recording
      deliverer's delivery count is **0** — not merely that the response was 401/403. An agent, journey
      or model call reached on a bad signature is the defect this story exists to prevent.
- [ ] `verify` and `token` compose: if both are declared, both must pass.
- [ ] The secret is registered with the redactor, so it cannot surface in a transcript or error.
- [ ] A `verify` block naming an unknown scheme is a **load error**, consistent with an unknown channel
      kind being a load error.

## Progress
- (not started)

## Notes
- Verified: `crates/flux-channels/src/adapters/webhook.rs` has no HMAC path; `handle` passes the parsed
  body straight to `deliver`. Raw-body capture is the structural change — everything else layers on it.
- Upstream source of the scheme parameters: flux-connectors' inbound spec (its C-59/C-60), which
  generates them per vendor from the vendor's own documentation.
```

---

## F-2 — the scheme matrix, constant-time comparison, and replay tolerance

```markdown
---
id: C-NNN
title: Webhook signature schemes — one parameterized HMAC, constant-time, replay-bounded
pillar: Core
status: backlog
areas: [flux-channels]
note: "four vendors' 'unique' schemes collapse to one algorithm over {digest, encoding, signed-template, tolerance}; the test vectors must come from vendor docs, never from our own implementation"
---

# Webhook signature schemes — one parameterized HMAC, constant-time, replay-bounded

## Goal
Implement the verification F-1 declares, covering the real vendor schemes with one parameterized
algorithm rather than a function per vendor.

## Acceptance
- [ ] HMAC with `sha256` (and `sha1` for legacy GitHub), `hex` and `base64` encodings, an optional
      literal prefix, and a `signed` template over `{body}` / `{timestamp}`.
- [ ] Failing-first test `vendor_signature_vectors_verify` using vectors **from each vendor's own
      documentation** — GitHub `X-Hub-Signature-256`, Stripe `Stripe-Signature` (`t=`/`v1=`), Slack
      `v0:{ts}:{body}`, Zendesk base64. Self-generated fixtures would agree with our implementation and
      prove nothing.
- [ ] Comparison is **constant-time**; a `==` on the digest is the defect under test.
- [ ] `tolerance` is enforced for any timestamped scheme: test `stale_timestamp_is_rejected` with a
      signature that is otherwise valid.
- [ ] Negative matrix: wrong secret, mutated body, truncated signature, missing header, wrong prefix —
      each rejected with zero delivery.

## Progress
- (not started)
```

---

## F-3 — answer the vendor's endpoint challenge without waking an agent

```markdown
---
id: C-NNN
title: Webhook challenge/handshake — answer endpoint verification without a turn
pillar: Core
status: backlog
areas: [flux-channels]
note: "Slack's url_verification echo and Meta's hub.challenge GET arrive at the same path as real events; waking a model to answer a handshake is both wasteful and a way to feed vendor-shaped text to an agent"
---

# Webhook challenge/handshake — answer endpoint verification without a turn

## Goal
Let a `channel webhook` satisfy a vendor's endpoint-verification handshake itself, so registering a
webhook does not depend on an agent happening to echo the right field back.

## Acceptance
- [ ] An optional `challenge` declaration: which field or query parameter carries the token, and what
      to echo (Slack: `type == "url_verification"` → echo `challenge`; Meta: GET `hub.challenge`).
- [ ] Failing-first test `challenge_answers_without_delivery`: the handshake gets the correct response
      body and the delivery count is **0** — no trigger fires, no model call happens.
- [ ] The challenge path is subject to the same verification as events where the vendor signs it, and is
      explicitly documented where it cannot be.
- [ ] A challenge-shaped body that does not match the declaration is treated as an ordinary event, not
      silently swallowed.

## Progress
- (not started)
```

---

## F-4 — route by event type instead of one mega-trigger

```markdown
---
id: C-NNN
title: Route a webhook to a trigger label by its event discriminator
pillar: Core
status: backlog
areas: [flux-channels, flux-app]
note: "today one webhook channel = one trigger label, so every vendor event lands in one flow that must switch on JSON; the vendor already tells us the type in a header or field"
---

# Route a webhook to a trigger label by its event discriminator

## Goal
Let a webhook channel fan out to per-event triggers — `trigger on "github.issues.opened"` — instead of
a single trigger that reimplements dispatch inside the flow.

## Acceptance
- [ ] An optional `discriminator` on the channel: `source` (`header` | `body`), `name`, and an optional
      `when` narrowing on a body field.
- [ ] The channel fires `"<channel>.<event>"` when the discriminator resolves, and plain `"<channel>"`
      when it does not — so existing single-trigger programs keep working unchanged.
- [ ] Failing-first test `discriminator_routes_to_distinct_triggers`: two events on one channel reach
      two different triggers.
- [ ] An event with no matching trigger is **not** an error; it is a logged no-op (vendors send event
      types you did not subscribe to).
- [ ] Trigger matching stays an exact label match — no globbing introduced by this story.

## Progress
- (not started)
```

---

## F-5 — surface the delivery id so a flow can dedupe

```markdown
---
id: C-NNN
title: Surface the webhook delivery id for at-least-once dedupe
pillar: Core
status: backlog
areas: [flux-channels]
note: "every vendor redelivers on a non-2xx and some redeliver spuriously; without the delivery id in the payload a flow cannot tell a retry from a second real event"
---

# Surface the webhook delivery id for at-least-once dedupe

## Goal
Give a flow what it needs to be idempotent under redelivery: the vendor's own delivery identifier.

## Acceptance
- [ ] An optional `delivery_id` declaration (`source`, `name`) whose resolved value is placed in the
      delivered payload under a documented key.
- [ ] Failing-first test `delivery_id_reaches_the_payload`.
- [ ] Documented guidance that the payload key is stable, since flows will key dedupe state on it.
- [ ] No dedupe state is kept in the channel itself — the channel reports, the flow decides. (A cache
      in the channel would be per-process and silently wrong across restarts and replicas.)

## Progress
- (not started)
```
