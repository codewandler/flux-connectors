---
id: C-199
title: Fence connector-pack against linking an HTTP client
pillar: Build
status: ready
priority: 2
design:
epic:
areas: [build]
note: "filed from C-192 2026-07-31 — `connector-pack` is in neither list in dependency_fence.rs, so adding an HTTP client to it trips no test. The lock-based walk that fences the compiler cannot state this one"
---

# Fence connector-pack against linking an HTTP client

## Goal

Make `connector-pack`'s central claim — it holds no HTTP client, resolves no host and opens no
socket — an assertion over the dependency graph rather than a comment, the way the compiler's
`connector-secrets` fence already is.

## Why now

`crates/connector-pack/src/tool.rs:18-37` states the guarantee as the reason `Egress` is a newtype
over `Arc<dyn Tool>` rather than a concrete `flux_web::http::HttpRequestTool`: *"It keeps this crate
from linking `flux-web` — a whole HTTP client, a DNS resolver and an SSRF guard — into a library
whose entire claim is that it opens no socket, so the claim stays structural rather than merely true
today."* AGENTS.md's ownership table says the same in its "Must never" column.

Nothing tests it. `crates/connector-cli/tests/dependency_fence.rs` fences four crates —
`connector-cli`, `codewandler-connector-spec`, `connector-flux`, `codewandler-connector-catalog` —
against reaching `codewandler-connector-secrets`. **`connector-pack` is in neither list**, so adding
`flux-web` to it today trips no test at all. The temptation is concrete and close: C-192 bumped the
engine to 0.41, and `codewandler-flux-web` publishes on the same version line.

## Why it is not a two-line addition to the existing fence

This was scouted during C-192 and deliberately not folded into it. The existing fence reads
`Cargo.lock`, and that choice is load-bearing and documented: the lock records the resolved graph
**including optional dependencies**, so an edge added behind a feature flag trips it too.

That instrument cannot state this guarantee. `connector-pack` depends on `connector-secrets`
(legitimately — it is where a credential is resolved), `connector-secrets` declares an optional
`reqwest` behind its `vault` feature, and so the lock walk already reports a path. Measured against
the committed lock at C-192:

```
codewandler-connector-pack -> codewandler-connector-secrets -> reqwest
codewandler-connector-pack -> codewandler-connector-secrets -> reqwest -> hyper
```

The guarantee is nonetheless **true**: `connector-secrets` declares `default = []`, and
`cargo tree -p codewandler-connector-pack -e normal` — with default features, and with
`--all-features` — contains no `reqwest`. So a fence written in the existing file's idiom would go
red on a correct build, and the only ways to make it green are design decisions rather than edits:

- assert over a **feature-resolved** graph (`cargo metadata` with feature resolution, or
  `cargo tree -e normal`) instead of the lock — which gives up the optional-dependency coverage that
  is the current fence's whole point, and so must be argued for, not assumed; or
- special-case the `connector-secrets` edge — which needs a stated rule for *why* that one optional
  HTTP client is allowed through and a future one is not.

Either answer needs to be written down. Picking one inside a version-bump story would have buried it.

## Acceptance

- [ ] A test asserts `codewandler-connector-pack` links no HTTP client — `codewandler-flux-web`,
      `reqwest`, `hyper`, `ureq`, `isahc`, `curl` — under the feature sets a *host* can select.
- [ ] The evidence source is chosen explicitly and the choice is justified in the test's own module
      comment, in the idiom of `dependency_fence.rs`: which graph is read, why that one, and what it
      consequently cannot see.
- [ ] The `connector-secrets` edge is addressed head-on rather than skirted — the test states why a
      dependency that *can* carry an optional HTTP client does not put one in the pack.
- [ ] The fence is non-vacuous: it is proved to catch a real edge, either against a synthetic graph
      the way `the_walk_finds_an_edge_that_is_not_direct` already does, or by a recorded manual run
      with the edge temporarily added.
- [ ] `crates/connector-cli/tests/dependency_fence.rs` and `no_network.rs` still hold unchanged; this
      story adds a fence, it does not weaken one.

## Notes

- The pack's guarantee is asymmetric with the compiler's. `connector-cli` must reach **no** socket-
  capable crate; `connector-pack` already has `tokio` and `socket2` in its closure through
  `flux-runtime`, and that is fine — it is handed a transport, and never constructs one. So the
  assertion is about **HTTP clients specifically**, not about socket capability, and writing it the
  other way would fail immediately and for the wrong reason.
- Related: [C-192](C-192-flux-0-41-bump.md), which moved the engine pin to 0.41 and scouted this.
