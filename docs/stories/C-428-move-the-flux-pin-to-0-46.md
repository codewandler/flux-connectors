---
id: C-428
title: "Move the flux pin from 0.45 to 0.46 — and it is blocked on flux-web, not on us"
pillar: Build
status: blocked
priority: 1
epic: connectors-api
note: "flagged by review 2026-08-01: flux released 0.46.0 today and this repo pins 0.45, so ^0.45 will not resolve it. BLOCKED UPSTREAM — codewandler-flux-web has no 0.46.0 on crates.io (verified against the sparse index), and connectors-api needs it for the Egress. Five of six flux crates are ready; the sixth is not"
---

# Move the flux pin from 0.45 to 0.46 — and it is blocked on flux-web, not on us

## Goal
Track the current flux engine line, so this repository picks up flux's credential-boundary and
vendor-disclosure work instead of silently sitting on the previous minor.

## Why now

Raised in review on 2026-08-01: flux released **0.46.0** the same day. flux treats the **minor**
position as the breaking signal at `0.y`, so `^0.45` does **not** resolve `0.46.0` — this repository
is pinned to the previous flux, and will not pick up flux's `C-312` credential boundary, `C-311`
vendor disclosure at approval, or `C-403`'s broker fix until the pin moves.

The reviewer's point about *which* seam makes it urgent is the right one: [C-407](C-407-extract-the-credential-address-crate.md)
has just reshaped the credential address vocabulary here, and flux's C-312 is the same seam from the
other side. Two repositories moving the same boundary while pinned apart is how they disagree.

## The blocker, measured

Queried against the crates.io **sparse index** (`https://index.crates.io/...`, which is what cargo
itself reads — the `api/v1` endpoint answers `403` from this environment and must not be used to
conclude a crate is absent):

| crate | 0.45.0 | 0.46.0 |
|---|---|---|
| `codewandler-flux-lang` | ✓ | ✓ |
| `codewandler-flux-core` | ✓ | ✓ |
| `codewandler-flux-runtime` | ✓ | ✓ |
| `codewandler-flux-system` | ✓ | ✓ |
| `codewandler-flux-credentials` | ✓ | ✓ |
| **`codewandler-flux-web`** | ✓ | **absent** |

`crates/connectors-api/Cargo.toml:40` takes `flux-web`, and it is not incidental: it supplies the
`HttpRequestTool` this repository uses as the `Egress` — the thing that makes a live vendor call
possible at all. So the engine line cannot move wholesale today, and moving five of six would split
the line, which is the exact defect `crates/connector-cli/tests/flux_engine_line.rs` exists to refuse.

## Acceptance
- [ ] `codewandler-flux-web` **0.46.0** is published upstream. Until then this story stays `blocked`
      and the pin stays at 0.45 — a split line is worse than a stale one.
- [ ] All six pins move together, in one commit: `flux-lang`, `flux-core`, `flux-runtime`,
      `flux-web`, `flux-system`, `flux-credentials`.
- [ ] `crates/connector-cli/tests/flux_engine_line.rs` passes — it records the engine line once and
      requires every requirement to agree with it. That test is the acceptance, not a formality.
- [ ] **The emitted artifacts are re-verified, not assumed.** `flux-lang` owns the formatter this
      repository emits through, so a minor bump can move generated text. Run
      `cargo run -p connector-cli -- build` and report whether any of the artifacts moved and why.
- [ ] Whatever flux's C-312 changed about the credential boundary is read against this repository's
      own credential-address work (C-407) and any disagreement is filed rather than absorbed.

## Progress
- 2026-08-01 — Filed from an external review. Verified the upstream gap the same day: five of six
  crates have 0.46.0, `flux-web` does not.

## Notes
- **Precedent: [C-403](C-403-move-the-flux-pin-to-0-45.md)**, which moved 0.41 → 0.45 and is the
  model for this — including that it is one commit that moves every pin, never a partial bump.
- Deliberately **not** folded into the `v0.9.0` release. That release already regenerates every
  artifact for babelforce's widening; changing the emit target in the same commit would put two
  independent reasons behind any artifact that moved. Ship the release on 0.45, bump immediately
  after.
- The right upstream ask is narrow: publish `codewandler-flux-web` 0.46.0. Everything else is ready.
