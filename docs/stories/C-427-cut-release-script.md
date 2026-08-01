---
id: C-427
title: "Cutting a release is nine hand-run steps, and flux already has the script"
pillar: Build
status: ready
priority: 2
areas: [build]
note: "found while documenting the release process 2026-08-01 — AGENTS.md now spells out nine ordered steps that a human or an agent performs by hand, and step 2 is load-bearing: 120 generated manifests carry the version string, so a bump that forgets to regenerate leaves `diff` red. flux's scripts/cut-release.sh is 217 lines and transactional"
---

# Cutting a release is nine hand-run steps, and flux already has the script

## Goal
Make cutting a release one command that either completes or leaves the tree exactly as it found it,
so the version bump cannot half-happen and the artifact regeneration cannot be forgotten.

## Why now

`AGENTS.md` § Release process was written on 2026-08-01 and, in writing it down, made the cost
visible: **nine ordered steps**, every one by hand. The dangerous one is step 2. This repository is a
compiler whose output records the compiler's own version — **120 generated `connectors/*.connector.toml`
carry `generator = "flux-connectors <version>"`**, and since C-189 `connectors.lock` hashes them. So
bumping `[workspace.package].version` without running `connector-cli build` in the same commit leaves
the tree inconsistent with itself and `diff` red, and the failure surfaces after the commit rather
than during it.

flux solved this: `scripts/cut-release.sh` is 217 lines, takes `<version>` or `patch`/`minor`, and is
**transactional** — a red gate restores the tree, so a failed cut is safe to re-run. It also stages
**only** the release files, so concurrent uncommitted work from another session is never swept into
a release commit.

## Acceptance
- [ ] `scripts/cut-release.sh <version>` performs the sequence `AGENTS.md` § Release process
      documents: promote `[Unreleased]` in **both** `CHANGELOG.md` and `WHATS-NEW.md`, bump
      `[workspace.package].version` and `README.md`, **regenerate every artifact**, run the full gate,
      then commit and tag.
- [ ] It accepts `patch` and `minor` as well as an explicit version, and applies this repository's
      rule — pre-1.0, the minor position is the breaking signal.
- [ ] **Transactional.** A red gate restores the tree to exactly what it was; a failed cut is safe to
      re-run. A failing-first test drives the gate red mid-cut and asserts `git status` is unchanged
      afterwards.
- [ ] **It stages only the release files.** Another session's uncommitted work is never swept into a
      release commit — the property flux's script calls out explicitly and the one that makes this
      safe to run in a repository where agents work concurrently.
- [ ] **It refuses to bump without regenerating.** A test asserts that after a cut, `diff` reports
      every artifact up to date and every `generator =` string carries the new version — the failure
      this story exists to prevent, caught by the script rather than by CI.
- [ ] It does **not** push and does **not** tag-and-push in one motion without the gate green.
      Pushing the tag is the crates.io publication (§ Publishing contract); the script prepares that
      moment, it does not take it by accident.
- [ ] `AGENTS.md` § Release process is rewritten around the script, keeping the *decisions* a human or
      agent still owns — the bump size, and whether the customer changelog says the right thing.

## Progress
- (not started)

## Notes
- **Read `~/projects/flux/scripts/cut-release.sh` first.** This is a port with one repository-specific
  addition — artifact regeneration — not a design problem. Do not re-derive its structure.
- The version reaches more places here than in flux: `[workspace.package].version`, `README.md`, and
  transitively the 120 generated manifests plus `connectors.lock`. Enumerate them from the tree rather
  than from this list, which is hand-typed and will drift (§ Before you assert anything).
- Out of scope: publishing. That is CI's, triggered by the tag push, and the script must not run
  `cargo publish` in any form other than `--dry-run`.
