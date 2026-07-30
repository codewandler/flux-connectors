---
id: C-61
title: Codegen — inbound events into the manifest and catalogue, nothing into the module
pillar: Codegen
status: backlog
design: docs/designs/inbound-events.md
epic: inbound-events
areas: [connector-flux, catalog]
note: "a connector module carries `op` declarations ONLY (flux lifts nothing else from ~/.flux/flows), so an event emits into the manifest + catalogue and the emitter REFUSES to fake one as a pollable op — the correction C-66 forced on this story"
---

# Codegen — inbound events into the manifest and catalogue, nothing into the module

## Goal

Turn the inbound IR into committed, reviewable artifacts that declare what a vendor will send us —
without emitting anything flux would silently ignore, and without dressing an event up as a callable
operation.

## Acceptance

- [ ] Each `[[inbound.event]]` emits into the **manifest** (`<name>.connector.toml`): event name,
      direction, transport, payload schema, the verification scheme's parameters, and the **credential
      name** the host must supply. No endpoint URL (an operator deployment detail), no secret value.
- [ ] **Nothing is emitted into `<name>.flux` for an event.** Failing-first test
      `inbound_event_emits_no_module_declaration`. A connector module is loaded from `~/.flux/flows` and
      flux lifts **`op` declarations only**, so a generated `channel`/`trigger`/`event` construct there
      is dead text; `channel` and `trigger` are Program members an operator declares.
- [ ] **The emitter refuses rather than pretends.** If anything requests a runnable form for an event,
      the build fails loudly naming this story — emitting an event as a pollable `op` is exactly the
      plausible-but-wrong output `AGENTS.md` forbids. (A genuine poll transport is C-63: a cursor `op`
      plus a documented program pattern, not an event pretending to be callable.)
- [ ] The **public catalogue and explorer** carry events distinguishably from operations, so a consumer
      cannot read an unrunnable event source as a callable tool.
- [ ] Event names are a **stable public contract** across regeneration, like op naming (C-23), and share
      one namespace per service with operations — a collision is a loud error (C-66).
- [ ] Generated output is deterministic and formatted, so the reviewed diff stays minimal.

## Progress
- (not started)

## Notes
- **This story was corrected before it started.** Its first draft emitted typed event declarations into
  `<name>.flux` with trigger labels. That is wrong: `docs/designs/connector-pipeline.md` establishes that
  the module carries `op` declarations and flux lifts only those.
  [C-66](C-66-members-under-services.md) raised the same objection independently, from the member-kind
  side. The verification *parameters* are what cross into flux — consumed by a program's
  `channel webhook` declaration (C-64), not by the module.
