---
id: C-403
title: "Move the flux pin from 0.41 to 0.45"
pillar: Build
status: done
priority: 1
epic: connectors-api
note: "THE CRITICAL PATH TO flux-exchange. Seven crates pinned at 0.41 while flux is 0.45; connector-pack hands out Arc<dyn flux_runtime::Tool>, so no consumer can link the pack and current flux together"
---

# Move the flux pin from 0.41 to 0.45

## Goal

Bring this repository onto the flux engine line consumers actually use, so `connector-pack` can be
linked by a host built on current flux. Until this lands, **no downstream can execute a connector
operation at all.**

## Why this is the critical path

`connector_pack::pack` hands out `Arc<dyn flux_runtime::Tool>`. Two versions of `flux-runtime` are
two distinct traits, so a host on flux 0.45 cannot accept a pack built against 0.41. Measured on
crates.io 2026-08-01: `codewandler-connector-pack` 0.8.0 requires `codewandler-flux-runtime ^0.41`,
which Cargo reads as `>=0.41.0, <0.42.0` for a `0.x` crate.

flux-exchange's `X-11` is the same fact from the other side, and its whole `invoke` epic is blocked
behind it.

## The breaking changes to absorb, from flux's own CHANGELOG

| release | change | expected impact here |
|---|---|---|
| 0.42.0 | `SubAgents` gained a public field; `VoiceTurnHandler::turn` carries a `Speaker` | none — neither is used here |
| **0.43.0** | **`http.request` returns a `{status, headers, body}` record instead of one flat string** | **the substantive one.** `Egress` delegates to it and the result is returned to the caller unchanged, so the shape a consumer sees changes |
| 0.44.0 | (no breaking entries) | none |
| 0.45.0 | a non-loopback webhook with a `token` and no `verify` no longer loads | check the channel-binding emitter does not produce one |

## Acceptance

- [x] All seven `codewandler-flux-*` pins move to the current line, and `flux-spec` is re-checked
      against crates.io rather than assumed — it moves on its own `1.x` line.
- [x] `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets
      -- -D warnings`, `cargo fmt --all --check` all green.
- [x] `cargo run -p connector-cli -- diff` reports every artifact up to date. **A bump that silently
      changes emitted Flux is the failure mode to watch for** — if artifacts move, stop and explain
      why before regenerating.
- [x] **Failing-first test** — the response shape `connector-pack` hands back is asserted. Whatever
      it is after the bump, a test pins it, because a consumer parsing the old flat string gets no
      compile error and a silent behaviour change.
- [x] `docs/integrating-with-flux.md` Gap 9 is re-checked and corrected: it currently says the record
      change is "closed upstream and expires here on a flux-web bump". This *is* that bump.
- [x] The engine line is recorded in one place, so the next bump is a value change.

## Progress
- **Done.** Merged from `impl/C-403`; full gate green after the coordinator's artifact build.
- **The `diff` Acceptance item was met the hard way and is worth reading as a success.** The
  implementor stopped at `2 artifacts would change` and **did not regenerate**, exactly as the story
  demands. Characterised: both are the README snippet SVGs, every text node byte-identical, four
  `fill=` attributes moving because flux-lang 0.45 classifies a reference to a bound local as `Op`.
  No `.flux`, `.connector.toml`, catalogue table or `catalog.json` moved — the emitted Flux is
  unchanged by the bump. The coordinator's full build then wrote exactly those 2 of 557, and
  `diff` now reports all 557 up to date.
- Five tests were red on the stale SVGs, one of them
  (`every_highlight_class_in_the_snippet_has_its_own_colour`) outside `AGENTS.md`'s documented
  expected-red set — because that table enumerates the *new-provider* case and this was a
  highlighter-classification change. Same single cause; all five green after the build.
- **Found, and worth filing upstream against flux:** `flux-runtime` 0.45.0 declares
  `flux-secret = "1"` but calls `Redactor::try_add_secret`, which first exists in 1.1.0. Any resolve
  that legally selects a 1.0.x fails with E0599 — this repository's committed lock did. The
  requirement should be `"1.1"`. This forced the only lock movement that is not a mechanical
  consequence of the seven declared bumps.
- Also corrected at integration: `crates/connectors-api/Cargo.toml` restated the engine line as 0.41
  with C-204's spent exception. `flux_engine_line.rs` reads `[workspace.dependencies]` only, so a
  second copy of the version there would never have been caught — precisely what Acceptance's
  "recorded in one place" exists to prevent.
- **This does not unblock a downstream host yet.** Published `connector-pack` 0.8.0 still requires
  `flux-runtime ^0.41`; only the next release closes flux-exchange's X-11.

## Notes
- This unblocks two things at once. Beside the linking problem, the record return is the prerequisite
  the `Graph` → composite-operation lowering has been waiting on — see C-404.
- After this lands, a `vX.Y.Z` tag republishes the closure; CI does the upload, nobody runs
  `cargo publish` by hand.
