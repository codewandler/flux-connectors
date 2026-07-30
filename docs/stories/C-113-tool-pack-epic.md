---
id: C-113
title: "The connector Tool pack — the flux interop layer (epic)"
pillar: Bridge
status: ready
priority: 1
design: docs/designs/connector-tool-pack.md
epic: tool-pack
areas: [bridge, codegen, connector-spec]
note: "EPIC — flux REMOVED flux-plugin-zendesk pending 'a flux-connectors interop layer'; D-200/D-201/D-202 are blocked on this and examples/zendesk.triage.flux is the written acceptance target. A Tool pack delegates to flux's own http.request, so flux keeps every byte of egress"
---

# The connector Tool pack — the flux interop layer (epic)

## Goal

Make a compiled connector **callable inside a flux runtime** without this repository growing a
runtime, a server, or a request path of its own.

This repo supplies a pack of `dyn Tool` implementations; `flux_sdk::ClientBuilder` constructs the
runtime, binds the ports and sets the configuration. Each generated Tool builds a request and
delegates to flux's own `http.request`, so egress stays entirely in flux.

## Acceptance

- [ ] A host can register every operation of a named provider with
      `ClientBuilder::try_register_pack`, and the operations resolve by their **dotted** names
      (`zendesk.ticket.show`) — the spelling flux's reference flow already uses and the one a
      composite declaration cannot have.
- [ ] Every generated Tool's `ToolSpec` carries the `risk`, `idempotency`, `effects` and input schema
      the connector author declared, so flux gates each operation individually.
- [ ] **The network gate is mirrored, not lost.** Delegating to `HttpRequestTool::execute` bypasses
      `Executor::dispatch`, so every generated Tool declares its own `permission_subjects` and
      `intents`. A test refuses any Tool whose `permission_subjects` is empty — C-115.
- [ ] **No credential reaches any surface.** Not a `ToolResult`, not an error, not a progress line —
      C-116.
- [ ] The pack is generated from the same IR as the `.flux` module, in one build, and a differential
      test asserts the two agree about the same operation — C-117.
- [ ] `vision.md` gains one clarifying line: a Tool pack is not a runtime, and why that distinction
      holds.
- [ ] The flux-side counterpart stories are filed, and `examples/zendesk.triage.flux` is re-pointed
      so `D-199`'s dependency note can close.

## Children

- [C-114](C-114-tool-spec-projection.md) — the crate and the `ToolSpec` projection
- [C-115](C-115-request-delegation.md) — request construction, delegation, and the mirrored gate
- [C-116](C-116-credential-store-port.md) — the `CredentialStore` port and redaction
- [C-117](C-117-pack-codegen.md) — codegen from the IR, joining the drift gate
- [C-118](C-118-connector-channel-adapter.md) — the connector-backed `flux-channels` adapter

## Notes

**This dissolves the `$auth` blocker.** [auth-seam.md](../designs/auth-seam.md) and
[C-26](C-26-file-seam-stories-on-flux.md) exist because flux's `{"$secret": "ENV"}` marker is
whole-value and headers-only, so `Bearer <token>` cannot be expressed. A Tool builds its header value
in Rust before `http.request` sees it, so the marker never needs to grow that capability. C-26's 11
paste-ready drafts should **not** be filed as written; the seam narrows to the composite-only case.

**The vision non-goal is intact.** "A runtime. This repo compiles; flux executes." A pack is data and
declarations handed to a runtime someone else constructs. It opens no socket and owns no executor.

**The naming asymmetry is the reason this is a Tool pack at all.** A dotted name is not a legal Flux
*declaration* (asserted in `crates/connector-flux/tests/op_emitter.rs`) but is the norm for a flux
*tool*. flux's reference flow calls `zendesk.ticket.show`; only a tool surface can spell it.
