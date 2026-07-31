---
id: C-195
title: "Publish to crates.io from CI on a version tag, never from a laptop"
pillar: Build
status: ready
priority: 1
design:
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

- [ ] `.github/workflows/crates-io.yml` publishes on a `v*` tag, modelled on flux's, with the same
      pre-flight secret check and the same idempotent skip.
- [ ] Publish **order** lives in `scripts/publish-crates-io.sh` and is derived from the actual
      dependency graph, not hand-listed. Today that order is `connector-catalog` (no dependencies),
      then `connector-secrets`, then `connector-pack`. A test or a `--dry-run` mode proves the order
      is a valid topological sort rather than a guess.
- [ ] **The workflow is proven without publishing anything.** `cargo publish --dry-run` for each
      crate runs in ordinary CI, so a packaging error (a missing `description`, `license`, or a file
      excluded from the package) is caught on every PR rather than at the moment of release.
- [ ] Every published crate carries the metadata crates.io requires and this repo's own conventions
      want: `description`, `license`, `repository`, `readme`, `keywords`. **Verify each crate has
      them before claiming this** — they are easy to miss and impossible to fix in a published
      version.
- [ ] The **crate names** are settled and recorded. `catalog`'s package name is already
      `connector-catalog` while its directory is `crates/catalog`; check what name is actually
      reserved and whether a `codewandler-` prefix is wanted for consistency with the flux family.
      A name, once published, is permanent.
- [ ] `docs/roadmap.md` and `AGENTS.md` say that publishing is CI-only, so nobody runs it by hand.

## Progress

- (not started)

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
