---
id: C-213
title: "The workspace MSRV is unenforced, and a dependency broke it without anything warning"
pillar: Build
status: ready
priority: 3
design:
epic:
areas: [build]
note: "found by the C-204 implementor 2026-07-31: a caret `10.3` on jsonwebtoken resolved to 10.4.0, which declares rust-version 1.88.0 against this workspace's 1.87. resolver = \"2\" does no MSRV-aware resolution, so nothing warned — it was caught by a person reading the lock"
---

# The workspace MSRV is unenforced, and a dependency broke it without anything warning

## Goal

Make the declared minimum supported Rust version a checked claim rather than a number in a manifest
that nothing consults.

## What was measured

While implementing [C-204](C-204-google-signin-accounts.md), a caret requirement `jsonwebtoken =
"10.3"` resolved to **10.4.0**, which declares `rust-version = 1.88.0`. This workspace's
`[workspace.package] rust-version` is **1.87**.

Nothing warned. `resolver = "2"` performs no MSRV-aware version selection, so cargo picked the
newest semver-compatible release and the incompatibility surfaced only because the implementor read
the lock. The immediate fix was a `~10.3.0` pin, which holds — but the mechanism that failed is
still exactly as it was, and the next caret dependency will do the same thing.

**This matters more here than in a typical workspace**, because `rust-version` is inherited by four
crates this repository *publishes* ([C-190](C-190-publish-catalog-pack-secrets.md)). A published
crate whose declared MSRV is lower than what it actually compiles under is a broken promise to
downstream consumers, and it is only fixable in a later version.

## The two decisions, which are separate

**1. Should `rust-version` be raised to 1.88?** Probably yes — it is the version the ecosystem is
moving to and the pin exists only to avoid it. But it is a **semver-relevant decision for four
published crates**, so it belongs to the owner rather than to a coordinator mid-wave. If it is
raised, the `~10.3.0` pin can relax back to a caret.

**2. Should the resolver move to `"3"`?** `resolver = "3"` performs MSRV-aware resolution: cargo
picks the newest version whose `rust-version` the workspace satisfies, so this class of drift is
caught by the tool rather than by someone checking the lock. It requires Rust 1.84+, which this
workspace already exceeds. This is the durable half of the fix and is independent of decision 1.

## Acceptance

- [ ] **Failing-first test:** a check that fails when any resolved dependency declares a
      `rust-version` above the workspace's. Name it. It must read the resolved graph (`cargo
      metadata`), not a hand-kept list — a hand-kept list is the same defect one level up, and this
      repository has [C-81](C-81-declared-counts-are-checked.md) as the standing example of what
      hand-maintained numbers do.
- [ ] The decision on `resolver = "3"` is taken and recorded with its reason, including what it does
      *not* cover.
- [ ] The decision on raising `rust-version` to 1.88 is taken **by the owner** and recorded. If it is
      raised, `jsonwebtoken`'s `~10.3.0` pin in `[workspace.dependencies]` relaxes to a caret and the
      comment explaining the pin is removed rather than left to mislead.
- [ ] Whatever CI does about MSRV is stated. If CI builds only on stable, the declared MSRV is
      untested regardless of resolver, and saying so is better than implying coverage that does not
      exist.

## Notes

- The existing `dependency_fence.rs` is the model: it walks the resolved `Cargo.lock` rather than
  trusting a declaration, which is why it catches an edge added behind a feature flag. An MSRV check
  should read the graph the same way.
- Do not confuse this with the flux version pins. Those are deliberate and documented; this is about
  the *Rust* version, and about the fact that nothing checks it.
- The `~10.3.0` pin is currently load-bearing. Do not relax it as a drive-by before decision 1 is
  taken — the build breaks on the workspace's own declared toolchain.
