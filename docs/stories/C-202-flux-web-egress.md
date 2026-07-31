---
id: C-202
title: "Bring flux-web into the graph and prove Egress over a real http.request"
pillar: Bridge
status: ready
priority: 1
design: docs/designs/connectors-api.md
epic: connectors-api
areas: [bridge, host]
note: "measured 2026-07-31: codewandler-flux-web 0.41.1 IS published and matches the pinned flux 0.41, so the Egress seam has a shipping implementation after all — connectors-app.md recorded this as unknown and worth finding out loudly"
---

# Bring `flux-web` into the graph and prove `Egress` over a real `http.request`

## Goal

Put a real `http.request` in the dependency graph and construct an `Egress` from it, so that
`connector-pack`'s request path stops being a proposition asserted against stubs and becomes
something that can send a byte.

## Why this is first

It is the smallest change that removes the gap every other story inherits.
[connectors-app.md](../designs/connectors-app.md) records the state precisely: *"`codewandler-flux-web`,
which owns `HttpRequestTool`, is **absent from `Cargo.lock`**… the one concrete implementation of
`Egress` this repository names in its own doc-comments cannot be constructed from this workspace at
all."* Every `connector-pack` test passes a stub, and says so.

That design also said what to do if the crate turned out not to be published at a compatible version:
record it loudly. It is published, at **0.41.1**, against a workspace pinned to flux **0.41**. So the
finding is the good one.

## Acceptance

- [ ] `codewandler-flux-web` is pinned in `[workspace.dependencies]` next to `flux-runtime`, with a
      comment stating why it is here and which crates may link it.
- [ ] A test constructs a real `HttpRequestTool`, wraps it in `Egress::new`, and projects a shipped
      operation onto it — proving the seam accepts the concrete type, not just `dyn Tool`.
- [ ] **Failing-first:** a test that sends one request to a loopback server and asserts the vendor
      received exactly the `{ method, url, headers, body }` the pack built. This is the first byte
      this repository has ever sent, and it is asserted against a server under test control, not a
      vendor.
- [ ] `crates/connector-cli/tests/dependency_fence.rs` gains a `NETWORK_CRATES` allow-list with a doc
      comment stating why the exception exists and what bounds it, exactly as
      [connectors-app.md](../designs/connectors-app.md) §"The extension: allow the exception, visibly"
      specifies. The four compiler crates stay fenced, and now also against every name in that list.
- [ ] A crate that is neither a compiler crate nor on the allow-list **fails** the test — that is what
      makes the list load-bearing rather than decorative.
- [ ] `connector-pack` itself gains no network dependency. `Egress` stays `dyn Tool`; the concrete
      client lives in the host crate. The ownership row in `AGENTS.md` is unchanged and still true.

## Notes

- flux-web's description: *"flux native web capabilities: arbitrary HTTP (`http.request`),
  readable-markdown fetch, and non-visual browser use — all under one scoped `web` egress policy."*
  The scoped egress policy is the point: the host configures it, and every connector inherits it.
- Version skew to watch: `codewandler-flux-{core,system,provider,markdown,a2a,credentials}` are at
  **0.42.x** on crates.io while `flux-runtime` and `flux-lang` are at **0.41.x**. Pulling flux-web
  0.41.1 must not drag a 0.42 core into the lock alongside the 0.41 one. If it does, that is a
  finding for [C-192](C-192-flux-0-41-bump.md)'s successor, not something to paper over.
- The mirrored network gate must be re-proved with the real tool in place:
  `crates/connector-pack/tests/network_gate.rs` asserts `permission_subjects`/`intents` for every
  shipped operation, and those exist precisely because delegating to `execute` bypasses
  `Executor::dispatch`.
