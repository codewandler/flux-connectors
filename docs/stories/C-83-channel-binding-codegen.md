---
id: C-83
title: Publish events and channel bindings into the manifest and the catalogue
pillar: Codegen
status: ready
priority: 3
design: docs/designs/channel-bindings.md
epic: channel-bindings
areas: [connector-cli, catalog, web]
note: "the strict split: bindings reach the manifest and catalog.json and NOTHING reaches the module. The emitter must refuse to dress a binding up as a pollable op"
---

# Publish events and channel bindings into the manifest and the catalogue

## Goal
Make the third member kind visible to consumers. Events and bindings are in the IR and in the hash
domain; they currently reach no artifact, so a host has no way to read what a connector declares.

## Acceptance
- [ ] `connectors/<id>.connector.toml` carries an `[[events]]` and a `[[channels]]` block: for each
      binding its transport, the events it carries, the verification parameters, the discriminator and
      delivery id, the payload map, and the reply as a **rendered oip**.
- [ ] The verification block names the **credential**, never a value.
      `crates/connector-cli/tests/site_catalog.rs::no_credential_value_reaches_the_document` must stay
      green with a sentinel set for the signing secret too.
- [ ] `catalog.json` carries both, under the existing every-key-always-present rule
      ([catalog-json.md](../designs/catalog-json.md)). Additive, so no `SCHEMA_VERSION` bump.
- [ ] A binding whose `verification = "none"` is published **loudly** — a consumer must be able to
      tell a deliberately-unverifiable surface from a verified one without inspecting the absence of a
      field.
- [ ] **Nothing reaches the `.flux` module.** A test asserts every shipped module is byte-identical
      across this story, and the emitter *refuses* rather than degrades if asked to emit a binding —
      the tempting wrong output is an event dressed up as a pollable op, which is exactly what
      `AGENTS.md` forbids.
- [ ] `--service <name>` selects that service's events and bindings along with its operations, per
      [C-66](C-66-members-under-services.md)'s acceptance. A selection that silently stayed
      operations-only would be the worst kind of partial success.
- [ ] The public site renders a provider's inbound surface, or the story records why it does not yet.

## Progress
- Not started. The IR half landed 2026-07-30 with [C-82](C-82-channel-bindings-epic.md).

## Notes
- Same strict split [C-61](C-61-inbound-codegen.md) states for events; this story supersedes its
  manifest half for bindings and should be read alongside it.
- `crates/connector-cli/src/catalog.rs` and `src/site.rs` share the credential and host walks
  deliberately, so a site and a `cargo add` consumer cannot be told different things. Keep that.
