---
id: C-431
title: "Move the flux pin from 0.46 to 0.47 — and the interesting result is that nothing here changed"
pillar: Build
status: done
epic: connectors-api
areas: [build]
note: "the successor C-428 asked for. Measured rather than assumed: every source file in all six engine crates this workspace links is BYTE-IDENTICAL between 0.46.0 and 0.47.1 — 0.47.0's C-404 and L-123 land in flux crates this repository does not consume. The bump exists to keep one engine line for the pack's `Arc<dyn Tool>`, not to pick up a fix"
---

# Move the flux pin from 0.46 to 0.47 — and the interesting result is that nothing here changed

## Goal
Put this workspace on flux's current engine line, so a `connector-pack` published from here links
into a host on 0.47 rather than resolving a second copy of the runtime.

## Why now

[C-428](C-428-move-the-flux-pin-to-0-46.md) closed with the successor written out: *"Not taken:
`flux-core` 0.47.0 is already on crates.io. … A 0.47 move is its own story when someone checks
`flux-web` has it."* It has it — the whole closure resolves at **0.47.1**.

The reason is the same one C-428 and C-403 give, and it is the only reason that has ever mattered
here: `connector-pack`'s public API hands a host `Arc<dyn flux_runtime::Tool>` and
`flux_core::Result`. A `0.x` requirement is `>=0.N.0, <0.N+1.0` to cargo, so a pack built against
0.46 and a host on 0.47 are **two unrelated traits in one graph**. Downstream, `flux-exchange`
cannot move to the 0.47 engine until a `connector-pack` release requires it.

## What 0.47 actually changed at this boundary — measured

0.47.0's headline entries are a credential-boundary fix for a host-dispatched plugin response
(upstream C-404) and an analyzer gate on three ungated production paths (upstream L-123). Both were
checked against the code rather than read off the changelog, by diffing the **vendored crate
sources** cargo resolved:

```
diff -rq ~/.cargo/registry/src/*/codewandler-flux-<crate>-0.46.0 …-0.47.1
```

| crate | files differing (excluding `Cargo.toml`, `Cargo.lock`, `.cargo_vcs_info.json`) |
|---|---|
| `flux-lang`, `flux-core`, `flux-runtime`, `flux-web`, `flux-system`, `flux-credentials` | **none** |
| `flux-provider`, `flux-config`, `flux-skill`, `flux-markdown` | **none** |
| `flux-plugin` | `src/bin/platform_plugin.rs` (a fixture binary, not the library) and `src/host/credential_boundary.rs` |

`credential_boundary.rs` gains **39 lines and every one of them is a comment** — the census C-404
records. The executable half of C-404, and all of L-123, live in flux's own binary, which this
repository does not link. So **0.47 is a no-op at this workspace's boundary**, and the bump is taken
to keep one engine line rather than to pick up a fix. That is worth stating plainly: an upgrade whose
value is compatibility, not behaviour.

## Acceptance
- [x] All six pins move together, in one commit — a split line is what
      `crates/connector-cli/tests/flux_engine_line.rs` exists to refuse. → `Cargo.toml` lines 92, 117,
      118, 140, 155, 213, plus `ENGINE_LINE = "0.47"`. `SPEC_LINE` stays `1.3`: the wire vocabulary a
      guest plugin compiles against is a different promise.
- [x] `Cargo.lock` resolves **one** engine line. `cargo update --workspace` moved all eleven
      `codewandler-flux-*` packages 0.46.0 → 0.47.1 and nothing else;
      `the_lock_carries_one_engine_line` passes.
- [x] The full gate is green: `cargo build --workspace`, `cargo test --workspace --no-fail-fast`
      (**1490 passed, 0 failed**, across 162 result sections), `cargo clippy --workspace
      --all-targets -- -D warnings`, `cargo fmt --all --check`, and
      `bash scripts/publish-crates-io.sh --print-order` / `--dry-run`.
- [x] **The emitted artifacts are re-verified, not assumed** — `flux-lang` owns the formatter this
      repository emits through. `cargo run -p connector-cli -- diff` reports
      `937 artifacts up to date (53 providers checked)` and exits 0. Byte-identical, which the source
      diff above predicts: `flux-lang` 0.47.1 *is* `flux-lang` 0.46.0.
- [x] The manifest comments explaining the engine line state 0.47 and why, rather than leaving a
      reader to date them from a version string.

## Progress
- 2026-08-01 — Landed on `impl/engine-0.47`. Six pins, `ENGINE_LINE`, `Cargo.lock`, three comment
  blocks in `Cargo.toml`, and this story.
- 2026-08-01 — **The measurement is the deliverable.** The gate says nothing broke; the source diff
  says nothing *could* have. Both are recorded because "green after a bump" and "the bytes did not
  move" are different claims and only the second explains the first.

## Notes
- **The disk, not the bump, was the only red seen.** The first `cargo test --workspace` run died with
  `ld terminated with signal 7 [Bus error]` while linking three `connector-cli` test binaries; `df`
  showed the root filesystem at **100%, 64 MB free**. `cargo clean` (20.6 GiB) and a full rebuild
  cleared it. Worth knowing because the diagnostic reads like a toolchain or dependency failure and
  is neither.
- Precedents, in order: [C-403](C-403-move-the-flux-pin-to-0-45.md) (0.41 → 0.45, and where
  `ENGINE_LINE` came from), [C-428](C-428-move-the-flux-pin-to-0-46.md) (0.45 → 0.46).
- **Still owed from C-428, and not closed here:** reading flux's credential-boundary work against
  this repository's own credential addressing (C-407). C-404 is the same seam moving again upstream,
  and this story establishes only that it did not move *in the crates we link* — which is a weaker
  statement than agreeing with it.
