---
id: C-31
title: Render a provider markdown page from the IR
pillar: Codegen
status: ready
priority: 8
design: docs/designs/provider-docs.md
epic: provider-docs
areas: [connector-flux]
note: also decides the tab dialect — hard to reverse once pages are committed
---

# Render a provider markdown page from the IR

## Goal
Emit one markdown page per provider from the IR, with a section per operation, so a reader can see
what a connector offers without reading compiler input or compiler output.

## Acceptance
- [ ] `flux-connectors build` emits `docs/providers/<name>.md` (or the agreed path) per provider.
- [ ] Page header: vendor, base URL, and the credentials the connector needs — **names only, never
      values**. A test asserts no credential value can appear.
- [ ] One section per operation with its signature, description, risk and idempotency, and a
      parameter table derived from each `Param`'s JSON Schema (name, in, type, required).
- [ ] The `<operation>.flux` fence shows **the actually generated Flux**, not a re-rendering — so the
      page cannot disagree with the module.
- [ ] **The tab dialect is decided and recorded.** Markdown has no standard tab syntax; MkDocs
      Material (`=== "Flux"`), Docusaurus (`<Tabs>`) and plain CommonMark are mutually incompatible,
      and the tabbed dialects render as literal noise wherever their renderer is absent. The design
      recommends plain CommonMark by default with tabbed dialects opt-in; confirm or overrule, and
      write the reason down.
- [ ] The emitter is **deterministic, total, and returns text rather than writing** — `connector-cli`
      already relies on all three for its byte-identical-no-op and atomic-write guarantees.

## Progress
- (not started)

## Notes
- This is a **third emitter** beside the Flux module and the connector manifest, fed by the same IR.
  Rendering from `providers/*.toml` instead would not work: a spec-ingested connector has no
  hand-written TOML.
- Response documentation is deliberately out of scope — `Operation::response_schema` is not populated
  richly enough yet to say anything useful.
- Expect doc pages to churn on every codegen change, because the Flux fence embeds generated output.
  That is the property that keeps them honest.
