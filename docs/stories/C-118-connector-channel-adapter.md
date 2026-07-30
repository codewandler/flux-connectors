---
id: C-118
title: "A connector-backed flux-channels adapter"
pillar: Bridge
status: ready
priority: 5
design: docs/designs/connector-tool-pack.md
epic: tool-pack
areas: [bridge, connector-spec]
note: "the second surface — flux's channel dispatch is a closed match with one arm per vendor, and its slack arm hand-builds a chat.postMessage this repo already compiles. Blocked on C-83's binding codegen"
---

# A connector-backed flux-channels adapter

## Goal

Let a connector's channel binding drive a flux ingress surface, so `flux-channels` gains one generic
`connector` kind instead of one hand-written adapter per vendor.

## Acceptance

- [ ] A `Channel` implementation (`crates/flux-channels/src/channel.rs:16`) is constructed from a
      connector's declared binding rather than from vendor-specific Rust.
- [ ] Inbound: the binding's declared verification runs before any payload is trusted — a webhook with
      no stated verification is already refused at load by C-82, and that refusal must not be
      bypassable here.
- [ ] Outbound: the reply goes through the operation's Tool from C-115, so the hand-built
      `chat.postMessage` in flux's slack arm is replaced by the compiled one, not duplicated beside it.
- [ ] **Failing-first test:** an inbound payload with a bad signature is rejected before dispatch, and
      the test fails against an adapter that verifies after decoding.
- [ ] Slack's two transports (Socket Mode and the Events API) both work from the one binding, which is
      the property C-82 already proved at the IR level.
- [ ] The gate is green on both repositories' workspaces.

## Notes

- **Blocked on [C-83](C-83-channel-binding-codegen.md)** — bindings must reach the manifest and
  `catalog.json` before an adapter can be built from them. Do not start this before C-83 lands.
- Depends on C-115 for the reply path and C-116 for credentials.
- Keep the three-way split C-82 recorded: vendor channels belong here, generic transports and
  time/lifecycle sources stay in flux. **Compiling a scheduler here would make this repository a
  runtime**, which the vision forbids.
- This is the smaller consumer of the two surfaces. It should not delay C-114 – C-117.
