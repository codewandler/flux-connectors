---
id: C-147
title: "The explorer runs an operation"
pillar: Codegen
status: ready
priority: 4
design: docs/designs/connector-tool-pack.md
epic: tool-pack
areas: [web]
note: "no wasm — this is reachable in plain TypeScript over the published catalogue, and proves the interaction is worth having before anyone pays for a wasm toolchain. C-148 decides wasm afterwards, with evidence"
---

# The explorer runs an operation

## Goal

Let an operation's page in the explorer show the request it would make, and a demonstration response
— so the catalogue becomes something you can try rather than only read.

## Acceptance

- [ ] An operation page can show its **constructed request**: method, URL with path parameters
      filled, headers, body. Driven by caller-supplied inputs against the operation's composed input
      schema.
- [ ] Where a fixture exists ([C-146](C-146-demo-fixtures.md)), the page can show the demonstration
      **response**, labelled as recorded rather than live.
- [ ] **It is unmistakably not a live call.** A reader must not come away believing the site called
      the vendor. Label it, and do not use language like "sent" or "succeeded".
- [ ] **No credential is ever collected.** No input field asks for a token, and the page cannot be
      made to hold one.
- [ ] The components follow [C-142](C-142-reusable-explorer-components.md)'s tiers: presentational
      components take what they render as props, and nothing reaches for data itself.
- [ ] **The hand-maintained-data guard still passes** — no provider, service or address named in
      explorer sources.
- [ ] **Failing-first test**, and the site behaves identically everywhere else.

## Notes

- **No wasm in this story, deliberately.** The request construction is derivable from the published
  catalogue in plain TypeScript, and this is the cheap version that proves whether the interaction is
  worth having at all. [C-148](C-148-decide-wasm-runtime.md) decides the expensive question
  afterwards, with this as evidence.
- **Depends on [C-145](C-145-dry-run-transport.md)** for the request-construction contract. The
  TypeScript here and the Rust dry-run must agree; if they can share a fixture set, do that rather
  than writing the logic twice — two derivations of one request is the drift C-117 exists to catch.
- Keep the explorer's existing character: it never claims an operation *works*. `StatusBadge`'s
  comment records why — nothing in this catalogue can make a live API call yet, so a green tick would
  be a lie. A "run" button must not quietly become that tick.
