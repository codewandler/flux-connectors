---
id: C-148
title: "Decide: does the connector pack participate in a wasm flux runtime?"
pillar: Bridge
status: ready
priority: 5
design: docs/designs/connector-tool-pack.md
epic: tool-pack
areas: [bridge]
note: "DECISION, and deliberately last. flux is ALREADY engineering for wasm with a story number — flux-flow's manifest says shedding a C library is what makes a wasm32 build of the engine possible (C-274). The question is whether this repo's pack joins"
---

# Decide: does the connector pack participate in a wasm flux runtime?

## Goal

Answer, in writing, whether `connector-pack` should build for `wasm32-unknown-unknown` so a flux
engine can execute connector operations **in a browser** — or whether the explorer stays TypeScript
over the published catalogue.

This produces a decision, not code, and it comes **after** [C-145](C-145-dry-run-transport.md) –
[C-147](C-147-explorer-runs-an-operation.md) so it is made with evidence rather than enthusiasm.

## What is already true, so nobody re-derives it

**flux is already building for wasm, deliberately and with a story id.** Read these before deciding:

- `../flux/crates/flux-flow/Cargo.toml:43` — *"a C library, so shedding it is what makes a `wasm32`
  build of the engine possible (C-274)"*.
- `../flux/crates/flux-events/Cargo.toml:15,47` — `rusqlite` is shed for the same reason,
  *"`wasm32-unknown-unknown` above all: the driver links a C library"*.
- `../flux/crates/flux-lang/Cargo.toml:66` — carries `crate-type = ["cdylib"]`, *"the shape
  `wasm32-unknown-unknown` needs"*.

So this is not a proposal to make flux wasm-capable. It is a question about whether **this**
repository's pack joins an effort already under way.

## The two obstacles to answer for

- **`flux-web` reaches the network through `reqwest`.** On `wasm32-unknown-unknown` there are no
  sockets; reqwest has a `fetch`-backed wasm mode, but whether flux's `http.request` builds under it —
  with its SSRF guard, DNS handling and header machinery — is unknown and must be measured, not
  assumed.
- **`flux-runtime` pulls `tokio`.** `flux-lang`'s manifest already records that
  `wasm32-unknown-unknown` has no sockets, so tokio's `net` feature pulls `mio`. Whether the runtime's
  tokio usage is shed-able is the same class of question flux answered for `rusqlite`.

## Acceptance

- [ ] A decision recorded in [connector-tool-pack.md](../designs/connector-tool-pack.md) with its
      reasoning, and this story closes `done` **whichever way it goes**. "No" is a successful outcome.
- [ ] The decision is informed by an actual **build attempt**, not by reading manifests — `cargo build
      -p connector-pack --target wasm32-unknown-unknown`, with the error output quoted. A guess about
      what compiles is worth nothing here.
- [ ] If **yes**: follow-up stories for whatever must be shed, and coordination with flux's C-274
      rather than a parallel effort.
- [ ] If **no**: record what the explorer does instead, and whether the TypeScript path from
      [C-147](C-147-explorer-runs-an-operation.md) is the permanent answer or an interim one.

## Notes

- **Do not start a wasm build before C-147 ships.** The cheap version has to prove the interaction is
  worth having; a wasm toolchain is a large, permanent maintenance surface to take on for a demo
  nobody has used yet.
- A browser-executed connector raises questions this repository has never had to answer: CORS (a
  vendor API will not have the site's origin in its allow-list), and where a credential would live in
  a page. **If the answer to either is unsatisfying, that alone settles the decision** — and it is
  cheaper to notice now than after a toolchain lands.
- Same discipline as [C-34](C-34-decide-proxy-charter.md) and
  [C-123](C-123-decide-connector-inference.md): a charter-sized question gets an explicit written
  answer before code, not a half-built implementation that decides it by accident.
