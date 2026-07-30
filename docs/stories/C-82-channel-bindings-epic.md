---
id: C-82
title: "Channel bindings — generalize a flux channel over a connector (epic)"
pillar: Spec
status: in-progress
priority: 2
design: docs/designs/channel-bindings.md
epic: channel-bindings
areas: [connector-spec, codegen, providers, bridge]
note: "EPIC — flux's channel dispatch is a closed match with one arm per vendor, and its slack arm hand-builds a chat.postMessage this repo already compiles. A binding is a COMPOSITION of an event and a reply operation, not a new primitive. IR + loader landed; codegen and the flux seam remain"
---

# Channel bindings — generalize a flux channel over a connector (epic)

## Goal
Let a connector **describe** a flux ingress surface, so that `flux-channels` gains one generic
`connector` kind instead of one hand-written adapter per vendor. A binding composes the events of
[C-58](C-58-inbound-events-epic.md)'s epic with an outbound operation this repository already emits.

## Acceptance
- [x] The three-way split is recorded: which flux channel kinds are vendors (belong here), which are
      generic transports, and which are time/lifecycle sources that must stay in flux. Compiling a
      scheduler here would make this repository a runtime.
- [x] `provider → service → (operation | event | channel)`, with one shared name namespace per
      service and the existing `#name` address fragment reused — amends
      [C-66](C-66-members-under-services.md).
- [x] A binding is a **composition**: it names declared events for inbound and a declared operation
      for the reply. No new primitive, and nothing new emitted into the module.
- [x] Every rule is a **refusal**. A dangling reply, an unbound required parameter, a webhook with no
      stated verification, a poll with no cursor — each is refused at load.
- [x] Slack ships both of its real transports (Socket Mode and the Events API) from one event set,
      one payload map and one reply — proving inbound is an abstraction over transports.
- [ ] Bindings reach the manifest and `catalog.json` — [C-83](C-83-channel-binding-codegen.md).
- [ ] The flux-side generic channel kind is designed and its stories filed —
      [C-84](C-84-flux-connector-channel-seam.md).
- [ ] The delivery envelope gap is filed — [C-85](C-85-delivery-envelope.md).

## Progress
- 2026-07-30 — **IR and loader landed.** `crates/connector-spec/src/inbound.rs` carries `Transport`,
  `VerificationScheme`/`HmacSpec`, `Selector`, `EventDecl`, `Reply` and `ChannelBinding`; `Connector`
  gained `events` and `channels`, both inside the hash domain. 31 tests in
  `crates/connector-spec/tests/channel_bindings.rs`, one per refusal rule.
- 2026-07-30 — `providers/slack.toml` declares `authority`/`api_version` (it had none), a `signing`
  credential, two events and two bindings. The emitted `slack.flux` is **byte-identical**, which is
  the proof that a binding stays out of the module.
- 2026-07-30 — Two findings recorded in the design: `Reply::result` (a journey's output has no path
  into the event that triggered it), and `HmacSpec::timestamp` (a `signed` template can say the
  timestamp is signed but not where it is read from).

## Notes
- **This epic exists because the inbound epic stopped one level short.** C-58–C-66 model the event;
  nothing modelled the surface an operator declares, which is where flux's per-vendor Rust lives.
- The punchline is `flux/crates/flux-channels/src/adapters/slack.rs:150` — it hand-builds a
  `chat.postMessage` whose three fields are the three body params of `slack-chat-post-message`.
- Sequenced **after** C-59's `[inbound]` in principle; in practice the event half landed here because
  a binding references event names and the two could not be split without a stub.
