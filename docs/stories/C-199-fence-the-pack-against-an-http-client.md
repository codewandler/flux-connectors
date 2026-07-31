---
id: C-199
title: Fence connector-pack against linking an HTTP client
pillar: Build
status: in-progress
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

- [x] A test asserts `codewandler-connector-pack` links no HTTP client — `codewandler-flux-web`,
      `reqwest`, `hyper`, `ureq`, `isahc`, `curl` — under the feature sets a *host* can select.
- [x] The evidence source is chosen explicitly and the choice is justified in the test's own module
      comment, in the idiom of `dependency_fence.rs`: which graph is read, why that one, and what it
      consequently cannot see.
- [x] The `connector-secrets` edge is addressed head-on rather than skirted — the test states why a
      dependency that *can* carry an optional HTTP client does not put one in the pack.
- [x] The fence is non-vacuous: it is proved to catch a real edge, either against a synthetic graph
      the way `the_walk_finds_an_edge_that_is_not_direct` already does, or by a recorded manual run
      with the edge temporarily added.
- [x] `crates/connector-cli/tests/dependency_fence.rs` and `no_network.rs` still hold unchanged; this
      story adds a fence, it does not weaken one.

## Notes

- The pack's guarantee is asymmetric with the compiler's. `connector-cli` must reach **no** socket-
  capable crate; `connector-pack` already has `tokio` and `socket2` in its closure through
  `flux-runtime`, and that is fine — it is handed a transport, and never constructs one. So the
  assertion is about **HTTP clients specifically**, not about socket capability, and writing it the
  other way would fail immediately and for the wrong reason.
- Related: [C-192](C-192-flux-0-41-bump.md), which moved the engine pin to 0.41 and scouted this.

## Progress

Landed as `crates/connector-cli/tests/pack_links_no_http_client.rs`, six tests. `dependency_fence.rs`
gained a cross-reference comment only; no assertion in it or in `no_network.rs` changed.

**Why a second file rather than two more tests in `dependency_fence.rs`.** The risk of splitting is
real — two guards asserting neighbouring properties are how two guards come to disagree — so the
split is on the one line where they *cannot* be merged: **they read different graphs, and each one
is wrong for the other's claim.** `dependency_fence.rs` reads `Cargo.lock` because optional
dependencies are exactly what it must catch; this file reads the feature-resolved graph because
optional dependencies are exactly what it must *not* count. Putting both in one module would leave
two `Lock`-shaped types a reader would reasonably assume were interchangeable, and the acceptance
also asks for the justification in "the test's own module comment" — which presupposes a module of
its own. They are kept from drifting by a cross-reference in each direction and by the fact that
neither restates the other's claim: `dependency_fence.rs` still says nothing about HTTP clients, and
this file says nothing about the compiler's offline guarantee.

**The instrument.** The lock walk was re-measured against the committed lock at `f3b4cec` and still
reports exactly the two chains this story quotes from C-192, so the story's premise holds unchanged:

```
codewandler-connector-pack -> codewandler-connector-secrets -> reqwest
codewandler-connector-pack -> codewandler-connector-secrets -> reqwest -> hyper
```

The file reads cargo's **feature-resolved** graph instead — `cargo metadata --format-version 1
--locked --offline`, whose `resolve.nodes[].deps` omit an optional dependency whose feature is off —
and covers each thing that instrument cannot see with a named test rather than leaving it implied.
The `--locked --offline` pair is deliberate: the test describes the committed lock and cannot change
it, and no test here may reach the network.

**Two design answers worth keeping.**

1. *"Under the feature sets a host can select"* is discharged by enumeration, not by sampling.
   `connector-pack` declares **no** `[features]` table, so its default feature set is its only one,
   and `the_pack_declares_no_features_so_the_default_graph_is_every_host_selectable_build` is what
   keeps that true. Measured: `--all-features` on the pack resolves the same graph as the default.
2. Cargo unifies features across a workspace, so the residual risk is not the pack's manifest but
   *any* member's. `the_optional_http_client_behind_the_pack_is_off_by_default_and_unrequested`
   therefore quantifies over `[workspace] members`, and catches the forwarded form
   (`kv = ["connector-secrets/vault"]`) as well as the direct one. What no test here can see is a
   *host outside this workspace* enabling `codewandler-connector-secrets/vault` for itself; that is
   named in the module comment as gap 1, and it is a property of the host's manifest, not the pack's.

**Every guard was shown red by mutation** (each reverted; `Cargo.lock` and every manifest verified
byte-identical to `f3b4cec` afterwards). `A` = `connector_pack_links_no_http_client`, `B` =
`the_packs_own_test_build_links_no_http_client_either`, `C` =
`the_optional_http_client_behind_the_pack_is_off_by_default_and_unrequested`, `D` =
`the_pack_declares_no_features_…`, `E` = `the_denylist_names_a_client_this_workspace_really_resolves`.

| mutation | red |
|---|---|
| `flux-web.workspace = true` in the pack's `[dependencies]` | A, B |
| `flux-web.workspace = true` in the pack's `[dev-dependencies]` | B only — A stayed green, which is what proves the normal/dev split is real |
| the pack asks `connector-secrets` for `features = ["vault"]` | A, B, C |
| `connectors-api` asks for `features = ["vault"]` — a *different* member | A, B, C — the unification claim above, measured |
| `connector-secrets` sets `default = ["vault"]` | A, B, C |
| `reqwest` made unconditional in `connector-secrets` | A, B, C |
| the pack forwards `kv = ["connector-secrets/vault"]` | C, D |
| the denylist renamed so nothing in it resolves | E |

`the_walk_finds_a_client_that_is_not_direct` carries its own control in-repo: the same graph with the
one offending edge severed reports no path.

**Out of scope, deliberately.** The pre-existing transitive `reqwest`/`hyper` in the *lock* is the
premise of this story, not a defect to remove — it is why the instrument changed. No manifest was
edited; `Cargo.toml` and `Cargo.lock` are untouched.
