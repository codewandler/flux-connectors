---
id: C-195
title: "Publish to crates.io from CI on a version tag, never from a laptop"
pillar: Build
status: in-progress
priority: 1
design: docs/designs/crates-io-publishing.md
epic:
areas: [build]
note: "C-190 wants three crates published; this is HOW. Modelled on ../flux's crates-io.yml — tag-triggered, idempotent, one secret. A publish is the one irreversible action in this repo, so it belongs behind a reviewed workflow and a pushed tag rather than in someone's shell history"
---

# Publish to crates.io from CI on a version tag, never from a laptop

## Goal

Make publishing a reviewed, repeatable, tag-triggered CI job, so that the one irreversible action
this repository can take is never a hand-typed command.

## Why this, and why before C-190

[C-190](C-190-publish-catalog-pack-secrets.md) says the three consumable crates must be published.
This story says **how**, and it comes first: a burned version number cannot be unburned, so the
mechanism should be reviewed in a pull request before it is ever used, not improvised at the moment
of first release.

`../flux` already solved this and the pattern is directly copyable —
`../flux/.github/workflows/crates-io.yml`. Its properties are the ones that matter here:

- **Tag-triggered** on `v[0-9]+.[0-9]+.[0-9]+`, alongside the binary release workflow, so publishing
  is a consequence of tagging rather than a separate ritual.
- **One secret**, `CARGO_REGISTRY_TOKEN`, with an explicit pre-flight check that fails with a
  human-readable `::error::` when it is absent. Nothing half-publishes because a secret was missing.
- **Idempotent** — a `crate@version` already live is skipped, so a run that dies to a crates.io
  new-crate rate limit can simply be re-run. This is the property that makes a multi-crate publish
  survivable.
- **`workflow_dispatch`** as an escape hatch, so a resumed publish can use an updated script.
- **Publish order in a script**, not in YAML, so the dependency order is testable and reviewable.

## Acceptance

- [x] `.github/workflows/crates-io.yml` publishes on a `v*` tag, modelled on flux's, with the same
      pre-flight secret check and the same idempotent skip.
- [x] Publish **order** lives in `scripts/publish-crates-io.sh` and is derived from the actual
      dependency graph, not hand-listed. Today that order is `connector-catalog` (no dependencies),
      then `connector-secrets`, then `connector-pack`. A test or a `--dry-run` mode proves the order
      is a valid topological sort rather than a guess.
- [x] **The workflow is proven without publishing anything.** `cargo publish --dry-run` for each
      crate runs in ordinary CI, so a packaging error (a missing `description`, `license`, or a file
      excluded from the package) is caught on every PR rather than at the moment of release.
- [x] Every published crate carries the metadata crates.io requires and this repo's own conventions
      want: `description`, `license`, `repository`, `readme`, `keywords`. **Verify each crate has
      them before claiming this** — they are easy to miss and impossible to fix in a published
      version.
- [ ] The **crate names** are settled and recorded. `catalog`'s package name is already
      `connector-catalog` while its directory is `crates/catalog`; check what name is actually
      reserved and whether a `codewandler-` prefix is wanted for consistency with the flux family.
      A name, once published, is permanent.
      → **Recorded, deliberately not settled.** Measured against the live crates.io API:
      *nothing is reserved* — `connector-catalog`, `connector-spec`, `connector-secrets` and
      `connector-pack` are all free, and `connector-cli` is already **taken** by an unrelated crate.
      The evidence and the `codewandler-` trade are in
      [docs/designs/crates-io-publishing.md](../designs/crates-io-publishing.md) §3. Choosing a
      permanent name is the owner's call, so this box stays open until they make it.
- [ ] `docs/roadmap.md` and `AGENTS.md` say that publishing is CI-only, so nobody runs it by hand.
      → **AGENTS.md done** (`## Publishing contract`). `docs/roadmap.md` is coordinator-owned and
      was not edited; the paragraph it needs is in the Progress note below.

## Progress

The mechanism is in place and exercised as far as anything can be without publishing. Two things are
deliberately left open — see the unchecked boxes above.

**Landed**

- `.github/workflows/crates-io.yml` — tag-triggered on `v[0-9]+.[0-9]+.[0-9]+` plus
  `workflow_dispatch`, `concurrency: crates-io-publish`, `permissions: contents: read`, the
  `CARGO_REGISTRY_TOKEN` pre-flight `::error::` as the *first* step, and a tag-vs-workspace-version
  check. Mirrors `../flux/.github/workflows/crates-io.yml`; the two divergences (pinned 1.97.0 rather
  than `stable`, `ubuntu-latest` rather than `ubuntu-22.04`) are argued in the design.
- `scripts/publish-crates-io.sh` — `ROOTS` lists only the *consumable* crates; the closure and the
  order are computed from `cargo metadata` by topological sort. Modes: publish (idempotent, with the
  429 wait-and-retry loop), `--dry-run`, `--print-order`.
- `crates/connector-cli/tests/publish_closure.rs` — 6 tests: the script's order equals an
  independent Rust recomputation, every crate follows its dependencies, the closure covers
  everything the roots reach, every published crate has its metadata, `ROOTS` has not drifted, and
  the sort itself finds a dependency two edges away.
- A `package` job in `.github/workflows/ci.yml` running `--dry-run` over the whole closure on every
  pull request.
- `readme`, `keywords`, `documentation` and `categories` on all four published crates, plus a
  per-crate `README.md` for each. Before this, all four had **no `readme` and no `keywords`** — and
  `cargo publish --dry-run` does not object to that, which is why the metadata check is a test over
  the manifests rather than only a dry run.
- `AGENTS.md` § Publishing contract; `docs/designs/crates-io-publishing.md`.

**The finding: the closure is four crates, not three.**

`connector-secrets` re-exports `CredentialRef` from `connector-spec`, so `connector-spec` is in its
public API and must be published too — a consumer outside this workspace cannot resolve
`connector-secrets` without it. Derived order:

    connector-catalog → connector-spec → connector-secrets → connector-pack

`connector-cli` and `connector-flux` stay unpublished. **This is C-190's arithmetic to redo**: four
new crates against the crates.io new-crate rate limit, and four permanent names rather than three.

**For the coordinator — `docs/roadmap.md` is fenced, so this was not written:**

> Publishing to crates.io is CI-only. A release is a consequence of pushing a `vX.Y.Z` tag;
> `.github/workflows/crates-io.yml` publishes the four-crate closure
> (`connector-catalog` → `connector-spec` → `connector-secrets` → `connector-pack`) idempotently
> from a single `CARGO_REGISTRY_TOKEN` secret. Nobody runs `cargo publish` by hand. See
> [AGENTS.md § Publishing contract](../AGENTS.md) and
> [docs/designs/crates-io-publishing.md](designs/crates-io-publishing.md).

**Not proved by any check here: the YAML.** Nothing parses `crates-io.yml` or exercises the tag
trigger, the secret pre-flight, or the 429 retry. Every non-trivial line lives in the script, which
*is* exercised. Before the first real tag, do one `workflow_dispatch` rehearsal — against an
already-published closure it exercises the whole path and skips every crate.

## Notes

- **This story does not decide *when* to publish, only *how*.** The milestone question — which
  stories must land before the first publish — belongs with C-190. The current reading is that
  `connector-catalog` is publishable early (it has **zero dependencies**, links no flux crate, and
  its API is generated tables), while `connector-pack` and `connector-secrets` should wait for
  [C-192](C-192-flux-0-41-bump.md) (a consumer must link exactly one flux-runtime; two versions are
  two incompatible types), [C-193](C-193-templated-hosts-never-resolve.md) (which changes how the
  pack is constructed) and at least one proven live call.
- **Do not add a `cargo publish` step to any existing workflow.** It gets its own file for the same
  reason flux gave it one: a publish must be greppable, and a concurrency group must keep two runs
  from racing.
- Worth checking while here: whether `connector-spec` and `connector-flux` also need publishing. The
  consuming repo asked for three crates, but `connector-secrets` re-exports `CredentialRef` from
  `connector-spec`, so the dependency graph may force a fourth. If it does, that is a finding for
  C-190 rather than a decision to take here.
