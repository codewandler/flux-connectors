---
id: C-202
title: "Bring flux-web into the graph and prove Egress over a real http.request"
pillar: Bridge
status: done
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

- [x] `codewandler-flux-web` is pinned in `[workspace.dependencies]` next to `flux-runtime`, with a
      comment stating why it is here and which crates may link it.
- [x] A test constructs a real `HttpRequestTool`, wraps it in `Egress::new`, and projects a shipped
      operation onto it — proving the seam accepts the concrete type, not just `dyn Tool`.
- [x] **Failing-first:** a test that sends one request to a loopback server and asserts the vendor
      received exactly the `{ method, url, headers, body }` the pack built. This is the first byte
      this repository has ever sent, and it is asserted against a server under test control, not a
      vendor.
- [x] `crates/connector-cli/tests/dependency_fence.rs` gains a `NETWORK_CRATES` allow-list with a doc
      comment stating why the exception exists and what bounds it, exactly as
      [connectors-app.md](../designs/connectors-app.md) §"The extension: allow the exception, visibly"
      specifies. The four compiler crates stay fenced, and now also against every name in that list.
- [x] A crate that is neither a compiler crate nor on the allow-list **fails** the test — that is what
      makes the list load-bearing rather than decorative.
- [x] `connector-pack` itself gains no network dependency. `Egress` stays `dyn Tool`; the concrete
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

## Progress

**2026-07-31 — the sixth item lands; the other five were already true and are now verified rather
than asserted.** Five of the six were satisfied by the C-200/C-201 work and never reconciled on this
file. Each was re-checked by running, not by reading:

| item | evidence |
|---|---|
| `flux-web` pinned with a comment | `Cargo.toml:79-93` — names why it is here, that `connectors-api` alone may link it, and why `0.41` rather than `0.42` |
| the seam accepts the concrete type | new: `tests/live_egress.rs::a_shipped_operation_projects_onto_a_real_http_request_tool` |
| the loopback round trip | new: `tests/live_egress.rs::the_vendor_receives_exactly_the_request_the_pack_built` |
| `NETWORK_CRATES` allow-list | `dependency_fence.rs:29-42`, enforced by `a_compiler_crate_cannot_reach_a_network_crate` |
| an unclassified crate fails | `every_workspace_member_is_classified` sorts each `[workspace] members` entry into one of three lists and fails on a remainder |
| the pack takes no network dependency | derived from `Cargo.lock`: `codewandler-connector-pack`'s closure contains no `codewandler-flux-web` |

### The loopback-versus-SSRF-guard tension, and how it was resolved

The host configures `PrivateNetAllow::None`, so its egress refuses exactly the addresses a
controlled vendor must live at. Resolved by **granting one host on one `App`** —
`PrivateNetAllow::Hosts(["127.0.0.1"])` through the already-existing `App::with_web_options` — and
leaving `WebOptions::default()` and `App::new` untouched. The grant is then proved to be
load-bearing rather than habitual: `the_default_egress_refuses_the_very_request_the_grant_admits`
runs the *same* projected operation under `App::new` and requires a refusal **with nothing recorded
by the vendor**.

The second half of the tension is that no shipped connector's `base_url` can name a loopback host,
and that is C-214's `Slot` guard working as designed. Rather than re-opening it, the test rewrites
**one string literal** — `https://api.openai.com` — in the operation's own emitted Flux, asserting
first that it appears exactly once. Everything downstream is the shipped operation's: method, path,
the module's `content-type`, the body's field set and canonical JSON encoding, and the credential's
header placement with its `Bearer ` prefix.

### Two things measured that are worth a successor story

1. **`the_default_egress_guards_the_private_network` did not test the host.** It asserts
   `flux_web::WebOptions::default().private_net`, a constant in a third-party crate. Editing
   `App::new` to pass `PrivateNetAllow::Any` leaves it **green** — measured, not predicted. The new
   behavioural test is what catches that; both are kept, and `tests/host.rs` now records the limit
   in place.
2. **`App::with_web_options` is public, unbounded, and the only thing standing between this host and
   `PrivateNetAllow::Any` is that nobody calls it that way.** Nothing refuses or records a widening.
   That is a fence worth having and is outside this story's Acceptance.
