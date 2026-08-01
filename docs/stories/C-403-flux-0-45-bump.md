---
id: C-403
title: "Move the flux pin from 0.41 to 0.45"
pillar: Build
status: ready
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

- [ ] All seven `codewandler-flux-*` pins move to the current line, and `flux-spec` is re-checked
      against crates.io rather than assumed — it moves on its own `1.x` line.
- [ ] `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets
      -- -D warnings`, `cargo fmt --all --check` all green.
- [ ] `cargo run -p connector-cli -- diff` reports every artifact up to date. **A bump that silently
      changes emitted Flux is the failure mode to watch for** — if artifacts move, stop and explain
      why before regenerating.
- [ ] **Failing-first test** — the response shape `connector-pack` hands back is asserted. Whatever
      it is after the bump, a test pins it, because a consumer parsing the old flat string gets no
      compile error and a silent behaviour change.
- [ ] `docs/integrating-with-flux.md` Gap 9 is re-checked and corrected: it currently says the record
      change is "closed upstream and expires here on a flux-web bump". This *is* that bump.
- [ ] The engine line is recorded in one place, so the next bump is a value change.

## Progress
- (not started)

## Notes
- This unblocks two things at once. Beside the linking problem, the record return is the prerequisite
  the `Graph` → composite-operation lowering has been waiting on — see C-404.
- After this lands, a `vX.Y.Z` tag republishes the closure; CI does the upload, nobody runs
  `cargo publish` by hand.
