---
id: C-192
title: Move the flux pin from 0.39 to 0.41
pillar: Build
status: in-progress
priority: 1
design:
epic:
areas: [build]
note: "filed from ai-agent-platform 2026-07-31 — a downstream host must link ONE flux-runtime, and connector-pack hands it Arc<dyn Tool>. Two engine versions are two incompatible types"
---

# Move the flux pin from 0.39 to 0.41

## Goal

Track the published flux engine line so a host that consumes both flux and `connector-pack` can link
them, which today it cannot.

## Why now — a downstream host is blocked on it

`~/babelforce/projects/ai-agent-platform` is folding a Connectors service into its own image and
registering this repository's operations through `connector_pack::pack(…)`. That call hands
`Arc<dyn flux_runtime::Tool>` into the host's `ToolRegistry`. **Two `flux-runtime` versions are two
different types and will not link**, so the host, this repository and the vendored service must all
land on one engine line before any of it compiles.

State on 2026-07-31:

| Tree | Engine pin |
|---|---|
| this repository | **0.39** |
| ai-agent-platform | 0.24.1, with 0.36 in flight (its C-57/C-61) and 0.41.0 as the agreed target (its C-62) |
| crates.io `max_stable` | **0.41.0** (`codewandler-flux-*`); the local flux tree is at an unreleased 0.41.1 |

## Acceptance

- [x] `flux-lang`, `flux-core`, `flux-runtime` and the `flux-system` dev-dependency move 0.39 → 0.41 in
      the workspace manifest.
- [x] The protocol tier (`flux-spec`, its own 1.x line) is **re-checked against crates.io**, not
      assumed to stay at 1.2 — the manifest's own comment says it moves independently.
- [x] `cargo run -p connector-cli -- diff` still reports every artifact up to date. Generated Flux is
      built as `flux_lang` AST and formatted by flux's formatter, so a formatter change upstream shows
      up here as artifact drift — if it does, regenerate and review the diff rather than pinning back.
- [x] Gate green: `cargo build --workspace`, `cargo test --workspace`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- [x] `crates/connector-cli/tests/dependency_fence.rs` and `no_network.rs` still hold — the offline
      guarantee and the `connector-secrets` fence must not be a casualty of a version bump.
- [x] Any `ToolSpec` / `Effect` / `Risk` / authority-layer changes between 0.39 and 0.41 are recorded
      here, because `connector-pack`'s `spec.rs` projection reads those types directly.

## Notes

- **Scout this before committing to a date.** Do the bump in a scratch worktree first and report
  whether the delta is mechanical; the downstream sequence is planned around the answer.
- flux 0.36 introduced an authority layer that validates every registered tool's contract
  (`ToolRegistry::register` panics on an invalid one). This repository is already past it at 0.39, but
  the downstream host is not — its C-61 characterised 39 failures. Worth knowing when reading their
  reports against ours.
- Prerequisite for [C-190](C-190-publish-catalog-pack-secrets.md); nothing downstream can consume the
  crates until they build against the host's engine version.

## Progress

**The delta is mechanical.** No API this repository consumes changed shape between 0.39 and 0.41 —
not `flux_runtime::Tool`, `ToolContext`, `ToolResult`, `ToolRegistry` or
`authority_requirements_from_declaration`; not `flux_lang::ast`, `opspec`, `program`, `parser`,
`format`, `format_cst` or `highlight`; not `flux_spec::{ToolSpec, Effect, Risk, Idempotency,
AccessKind}`. Not one call site needed editing. The scout the story asked for is done and the answer
is yes: the downstream sequence can be planned on it.

### What moved

**crates.io, re-checked 2026-07-31.** `codewandler-flux-{lang,core,runtime,system}` max at
**0.41.0**; `codewandler-flux-web` likewise 0.41.0 (not added — see below). The `v0.41.1` git tag in
the local flux tree is unpublished; nothing here pins it.

**`flux-spec` did move, and this repository deliberately did not follow it.** crates.io is at
**1.3.0**, and both `flux-lang` and `flux-runtime` 0.41 require it as plain `"1"`, so a fresh resolve
anywhere selects 1.3.0 — the committed lock now does too, and the workspace builds and passes against
it. The manifest *requirement* stays `"1.2"`: 1.3.0 is purely additive over 1.2.1 (it adds
`JsonSchema` derives to `Effect`, `Risk`, `Idempotency`, `AccessKind` and `ToolSpec`, and changes
nothing else), so raising the floor would narrow what a consumer may unify with and buy nothing.

**`codewandler-flux-runtime` 0.41.0 does not compile against the `flux-evidence` it asks for.** It
declares `flux-evidence = "1"` and calls `EvidenceLog::{set_max_payload_bytes,
retained_payload_bytes, compaction_notice}`, which exist only from **1.1.0**. Against this
repository's inherited lock (evidence 1.0.0) the bump failed with three `E0599`s *inside flux's own
source*. Fixed here by moving the lock to `codewandler-flux-evidence` 1.1.0. **Worth reporting
upstream:** a fresh resolve picks 1.1.0 and is unaffected, so this bites exactly the trees that
already have a lock — which is every downstream host, ai-agent-platform included.

### The one artifact-visible behaviour change

flux-lang 0.41's AST printer changed how it renders modifier arguments on `retry` and `confirm`, from
juxtaposition to comma-separated colon-keyed pairs:

```diff
-  retry 3 backoff exponential -> read_result
+  retry 3, backoff: exponential -> read_result
-  confirm "Delete the swept thing?" risk destructive
+  confirm "Delete the swept thing?", risk: destructive
```

**No committed artifact moved.** `build` wrote nothing and `diff` reports `479 artifacts up to date
(44 providers checked)`, because no shipped provider declares a graph using `retry` or `confirm` —
the syntax appears nowhere under `connectors/` or `crates/catalog/ops/`. It surfaced only in
`crates/connector-flux/tests/golden/graph-nightly-sweep.flux`, which was re-recorded. The new
spelling is correct rather than merely accepted: `an_emitted_graph_parses_is_canonical_and_reloads_as_one_op`
covers that graph and asserts flux's own CST formatter is a fixed point on the emitted text, so the
printer and the formatter agree on the new form. The other golden is unchanged.

The rest of `format.rs`, `format_cst.rs` and `highlight.rs` changed upstream without moving anything
here — the whole shipped corpus is still a byte-identical fixed point of a 0.41 build.

### Deliberately not done

**`codewandler-flux-web` was not added.** `connector-pack` takes its transport as
`Egress(Arc<dyn Tool>)`, a constructor argument it never builds, precisely so the crate links no HTTP
client, no DNS resolver and no SSRF guard (`crates/connector-pack/src/tool.rs:18-45`). That belongs
to a host or a gated live test, never to a published library.

**Fencing `connector-pack` against an HTTP client was scouted and filed as
[C-199](C-199-fence-the-pack-against-an-http-client.md), not done here.** It is not contained: the
existing `dependency_fence.rs` reads `Cargo.lock` deliberately, because the lock records optional
dependencies — and over that graph the pack *already* reaches an HTTP client through
`connector-pack -> connector-secrets -> reqwest`, an edge that is optional, off by default, and
absent from `cargo tree -e normal` under every feature set the pack can select. Stating the guarantee
needs a different evidence source and a written rule for that edge; picking one inside a version bump
would have buried the decision.
