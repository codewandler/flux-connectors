---
id: C-213
title: "The workspace MSRV is unenforced, and a dependency broke it without anything warning"
pillar: Build
status: done
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

- [x] **Failing-first test:** a check that fails when any resolved dependency declares a
      `rust-version` above the workspace's. Name it. It must read the resolved graph (`cargo
      metadata`), not a hand-kept list — a hand-kept list is the same defect one level up, and this
      repository has [C-81](C-81-declared-counts-are-checked.md) as the standing example of what
      hand-maintained numbers do.
      → `crates/connector-cli/tests/msrv_fence.rs::no_resolved_dependency_declares_a_rust_version_above_the_crate_that_reaches_it`
- [x] The decision on `resolver = "3"` is taken and recorded with its reason, including what it does
      *not* cover. → **taken**; reason and the four things it does not cover are in `Cargo.toml`
      `[workspace]`, and repeated under "Decision 1" below.
- [ ] The decision on raising `rust-version` to 1.88 is taken **by the owner** and recorded. If it is
      raised, `jsonwebtoken`'s `~10.3.0` pin in `[workspace.dependencies]` relaxes to a caret and the
      comment explaining the pin is removed rather than left to mislead.
      → **still open, by design.** Evidence and a recommendation are under "Decision 2" below; the
      decision itself is the owner's.
- [x] Whatever CI does about MSRV is stated. If CI builds only on stable, the declared MSRV is
      untested regardless of resolver, and saying so is better than implying coverage that does not
      exist. → **CI does nothing about MSRV.** Stated under "What CI does" below and asserted by
      `msrv_fence.rs::ci_pins_a_toolchain_far_above_the_declared_msrv`.

## Notes

- The existing `dependency_fence.rs` is the model: it walks the resolved `Cargo.lock` rather than
  trusting a declaration, which is why it catches an edge added behind a feature flag. An MSRV check
  should read the graph the same way.
- Do not confuse this with the flux version pins. Those are deliberate and documented; this is about
  the *Rust* version, and about the fact that nothing checks it.
- The `~10.3.0` pin is currently load-bearing. Do not relax it as a drive-by before decision 1 is
  taken — the build breaks on the workspace's own declared toolchain.

## Progress

### The fence, and a second breach it found on arrival

`crates/connector-cli/tests/msrv_fence.rs` walks the feature-resolved graph from
`cargo metadata --locked --offline` and compares each **workspace member's** declared `rust-version`
against every package in its closure. Per-member rather than against one global number, because
that is the shape of the promise: four crates answer to crates.io, `connectors-api` answers only to
this repository's gate.

It reads the graph cargo resolves *with features applied*, not `Cargo.lock`. That is a deliberate
divergence from `dependency_fence.rs` and the reason is the inverse of that file's: an MSRV is a
property of what is **compiled**, so a crate sitting in the lock behind an off-by-default feature
cannot break any build, and a lock-reading fence would be red on a correct tree.

**It was red on `main` for a breach nobody had filed**, not for the `jsonwebtoken` one this story
was written about — that had already been pinned:

```
connectors-api v0.7.0 declares rust-version 1.87.0 and reaches zip v8.6.0, which requires 1.88.0
  connectors-api v0.7.0 -> codewandler-flux-web v0.41.0 -> codewandler-flux-plugin v0.41.0 -> zip v8.6.0
```

`flux-plugin` requires `zip ^8`, and **every** published 8.x declares `rust-version = 1.88.0` —
measured by walking 8.6.0 down to 8.0.0 with `cargo update --precise`, each of which cargo reported
as `(requires Rust 1.88)`. So no pin fixes this one. The inherited 1.87 on `connectors-api` was
simply false, and had been since C-202 put `flux-web` in the graph.

`crates/connectors-api/Cargo.toml` therefore declares `rust-version = "1.88"` of its own instead of
inheriting. That is a **correction of a false declaration, not a bump**, and it is safe for this
crate specifically because it is `publish = false`. The four published crates keep 1.87, which their
own closures do honour.

### Decision 1 — `resolver = "3"`: **taken**

`[workspace] resolver = "3"`. It needs cargo 1.84+; the MSRV is 1.87, so every toolchain this
workspace claims to support has it. Proof it does the job, measured by relaxing the requirement back
to a caret and re-resolving:

```
$ cargo update -p jsonwebtoken --verbose
   Unchanged jsonwebtoken v10.3.0 (available: v10.4.0)
```

Cargo saw 10.4.0 and declined it. Under `resolver = "2"` that same requirement is what broke the
workspace.

**What it does not cover** — the reason the fence exists alongside it, not instead of it:

1. It is a **preference, not a constraint**. If a requirement's whole range declares too high an
   MSRV, cargo resolves it anyway and only warns. The resolver *avoids* a breach; the fence *fails*
   on one.
2. It **does not revisit an existing `Cargo.lock`**. Measured: switching the resolver moved zero
   bytes of the lock, and `cargo update -p zip` reported `Locking 0 packages` because cargo never
   downgrades an already-locked version without `--precise`.
3. It reads **declarations**. A dependency using a 1.88 feature without raising its own
   `rust-version` is invisible to it, as is such a feature written in this repository's `src/`.
4. It **builds nothing** on the declared MSRV — see below.

The `~10.3.0` tilde on `jsonwebtoken` stays. The resolver is overridable by
`--ignore-rust-version` and `--precise`, and a downstream consumer of the published crates resolves
under its own settings rather than this workspace's.

### Decision 2 — raise `rust-version` to 1.88: **open, and recommended**

Not taken here: it is semver-relevant for four crates published to crates.io, and this story's own
text puts it with the owner. The evidence, now stronger than when the story was filed:

- **1.87 is already unbuildable for one workspace member**, and unfixably so — `zip ^8` has no
  1.87-compatible release. The choice is not "1.87 or 1.88" but "1.88, or a per-crate override that
  says 1.88 in a quieter place".
- Two separate dependencies have now demanded 1.88 within one week (`jsonwebtoken` 10.4.0, `zip`
  8.x). This is the ecosystem's floor moving, not one vendor being aggressive.
- The cost of raising it is one `[workspace.package]` line, and it **deletes** two pieces of
  scaffolding rather than adding any: `jsonwebtoken`'s `~10.3.0` pin relaxes to `"10.3"` with its
  explanatory comment removed, and `crates/connectors-api/Cargo.toml`'s `rust-version` override goes
  away entirely. Both are commented to say so.
- The cost of *not* raising it is that every future 1.88 dependency is either pinned back or
  overridden crate-by-crate, and the published crates advertise a version no CI job compiles them
  on.

**Recommendation: raise it**, in a commit that does nothing else, and delete both pieces of
scaffolding in the same commit.

### What CI does about MSRV: **nothing**

Stated plainly because implying coverage that does not exist is the failure mode this story names.
All three workflows install one pinned toolchain via `dtolnay/rust-toolchain`:

| workflow | toolchain |
|---|---|
| `.github/workflows/ci.yml` (two jobs) | `1.97.0` |
| `.github/workflows/crates-io.yml` | `1.97.0` |

There is no `rust-toolchain.toml` in the repository and no job that builds on `rust-version`. **The
declared MSRV of 1.87 is compiled by nothing, anywhere** — not in CI, not locally (this worktree is
on 1.97.0), and `resolver = "3"` does not change that. It keeps a *newer* dependency out of the
graph, which is a different claim.

`msrv_fence.rs::ci_pins_a_toolchain_far_above_the_declared_msrv` asserts this gap rather than
letting it be assumed. It is deliberately weak — it checks the pin is above the MSRV, not that any
particular job exists — because `.github/workflows/` is coordinator territory and C-240 was editing
it concurrently.

**The follow-up this leaves open** is a CI job that runs `cargo check --workspace` on the declared
`rust-version`. It was not added here: it belongs in `.github/workflows/ci.yml`, which C-240 held
during this wave, and it would be red today for the `zip` reason above until decision 2 is taken.
Worth filing once decision 2 lands, as a **new job** rather than an edit to an existing one.
