# Design: publishing to crates.io

**Status:** approved, with one open decision (crate names) · **Pillar:** Build · **Stories:**
[C-195](../stories/C-195-crates-io-release-workflow.md), [C-190](../stories/C-190-publish-catalog-pack-secrets.md)

> Facts below were measured on 2026-07-31 against this workspace at `0.5.0` and against the live
> crates.io API. The name availability in §3 is the part most likely to have changed — re-check it
> before the first tag.

## Why

Publishing is the **one irreversible action** this repository can take. A version cannot be
withdrawn, a crate name cannot be reclaimed, and a wrong `description`, `readme` or `keywords` is
fixable only in the next version. Everything else here — a bad artifact, a wrong `.flux` — is a
revert away.

That asymmetry is the whole design: the mechanism is reviewed in a pull request *before* it is used
([C-195](../stories/C-195-crates-io-release-workflow.md)), the decision of when to use it is
separate ([C-190](../stories/C-190-publish-catalog-pack-secrets.md)), and the thing that would
otherwise only be discovered at release time is moved onto every pull request.

`../flux` solved the same problem and its `crates-io.yml` is the template. This mirrors it.

## 1. The closure is four crates, not three

C-190 names three consumable crates. The dependency graph says four.

```
connector-catalog                             (no dependencies at all)
connector-spec   → connector-secrets          (CredentialRef, Layout, TenantLayout)
connector-catalog, connector-secrets → connector-pack
```

`connector-secrets` does not define credential addressing; it re-exports `CredentialRef` from
`connector-spec` (deliberately — C-90 says there is one addressing scheme, not two). That type is in
`connector-secrets`' *public API*, so `connector-spec` is published or nothing outside this
workspace resolves. The `path` dependency that makes it work here does not travel in a `.crate`.

Derived publish order:

| # | crate | why it is here |
|---|---|---|
| 1 | `connector-catalog` | consumable; zero dependencies |
| 2 | `connector-spec` | **not requested — forced** by `connector-secrets`' public API |
| 3 | `connector-secrets` | consumable; needs `connector-spec` live |
| 4 | `connector-pack` | consumable; needs `connector-catalog` and `connector-secrets` live |

Not published: `connector-cli` (this repository's own build tool) and `connector-flux` (reachable
only from it). Neither is in the closure of any consumable crate, so neither is forced.

**This is a finding for C-190, not a decision taken here.** It changes that story's arithmetic:
four new crates against the crates.io new-crate rate limit rather than three, and one more permanent
name to settle. It also means `connector-catalog` is no longer quite the "publishable on its own"
crate the notes assume — it still is, but `connector-secrets` is not, and `connector-pack` needs
three predecessors live.

### The order is derived, not listed

`../flux` hand-lists 29 crates because its graph carries ordering constraints a manifest does not
state (an optional feature dependency; a protocol line versioned independently of the runtime). This
workspace has four crates and one non-obvious edge, so a topological sort over the manifests is
*exact* — and a sort cannot go stale the way a list does.

`scripts/publish-crates-io.sh` therefore lists only **ROOTS** (which crates are consumable — a
policy choice) and computes the closure and its order from `cargo metadata`.
`crates/connector-cli/tests/publish_closure.rs` recomputes the same sort independently in Rust and
requires the two to agree, so the script cannot be hand-edited into a wrong order and a new edge
cannot enlarge the closure unnoticed.

## 2. The mechanism

Mirrors `../flux/.github/workflows/crates-io.yml`, property for property:

| property | why it is load-bearing |
|---|---|
| **Idempotent** — `crate@version` already live is skipped | The crates.io new-crate rate limit is a burst then ~1 per 10 minutes. A four-crate first release can die halfway, and the crates already up cannot be withdrawn. Without a resumable script that state is unrecoverable. |
| **Tag-triggered** on `v[0-9]+.[0-9]+.[0-9]+` | A release is a consequence of tagging, not a separate ritual someone remembers. |
| **`workflow_dispatch`** | Resumes from a branch, so a fix to the script is usable without re-tagging. |
| **One secret, checked first** | `CARGO_REGISTRY_TOKEN`, with an `::error::` naming exactly where to set it — checked before a toolchain is installed, so a missing secret is never discovered after crate #1 is live. |
| **Order in a script, not YAML** | Reviewable and testable. A `--print-order` mode is what the test drives. |
| **`concurrency` group** | Two runs racing on the same closure would interleave uploads. |

Deliberate divergences from flux's file, both small:

- **Pinned toolchain (1.97.0), not `stable`.** The dry-run gate in `ci.yml` proves this closure
  packages; that proof is only about this release if both runs use the same cargo. `ci.yml` already
  pins for the same class of reason.
- **`ubuntu-latest`, not `ubuntu-22.04`.** flux pins the image because it also builds release
  binaries whose glibc floor matters. Nothing here ships a binary.

## 3. Crate names — open, and it must be closed before the first tag

**Measured against the live crates.io API on 2026-07-31:**

| name | status |
|---|---|
| `connector-catalog` | free, **not reserved** |
| `connector-spec` | free, **not reserved** |
| `connector-secrets` | free, **not reserved** |
| `connector-pack` | free, **not reserved** |
| `connector-flux` | free, not reserved |
| `connector-cli` | **TAKEN** — v0.12.0, "CLI for interacting with Tauri apps via tauri-plugin-connect" |
| `codewandler-connector-*` (all four) | free, not reserved |
| `codewandler-flux-lang` | taken by us — v0.41.0 |

Two facts follow, and they point in the same direction:

1. **Nothing is reserved.** The assumption that `connector-catalog` is already ours is wrong. Any of
   these names can be taken by anyone up to the moment we publish.
2. **Bare `connector-*` names already collide.** `connector-cli` is gone to an unrelated project.
   That is what a generic name in a crowded flat namespace does.

The flux family is `codewandler-flux-*` — the vanity prefix on the *package*, the plain name on the
`[lib]` (`codewandler-flux-lang` / `flux_lang`). This repository already uses that split once:
`crates/catalog` is package `connector-catalog`, library `catalog`. Extending the org prefix
(`codewandler-connector-catalog` / `catalog`) would be consistent, collision-proof, and legible as
"same authors as flux".

**This design does not choose.** The trade is real in both directions — `codewandler-connector-pack`
is a mouthful, and `connector-pack` is shorter and still free — and a name is permanent. It is the
repository owner's call, and the only thing that must not happen is it being decided implicitly by
the first `git tag`.

Whichever way it goes, the mechanism is name-agnostic: the workflow, the script and the test all
read names from the manifests, so a rename is an edit to four `[package] name =` lines and nothing
else. **Rename before the first publish; after it, both names exist forever.**

## 4. Metadata

`description`, `license`, `repository`, `readme`, `keywords` on all four, plus `documentation` and
`categories` following flux's convention. `license` and `repository` are inherited from
`[workspace.package]`; `readme` names a per-crate `README.md`, because a crates.io front page that
says "see the monorepo" is a worse first impression than three paragraphs.

Before C-195 all four crates had `description`, `license` and `repository` and **none** had `readme`
or `keywords` — which `cargo publish --dry-run` does not object to, so it would have shipped. That
is precisely why the check is a test over the manifests and not only a dry run.

## 5. What is proved, and what is not

**Proved on every pull request** — the `package` job in `.github/workflows/ci.yml` runs
`cargo publish --dry-run` over the whole closure: that each crate packages, that the packaged
contents build in isolation, that no dependency lacks a version, that `readme` resolves, that
`include`/`exclude` did not drop a needed file. This is the check that would have caught
`connector-catalog`'s 293 `ops/*.flux` files being excluded, or `connector-pack` failing to build
against a packaged `connector-catalog` rather than the path one.

**Proved in the ordinary test gate** — `publish_closure.rs`: the closure, the order, the metadata.

**Not proved by anything in this repository: the YAML itself.** No test parses
`.github/workflows/crates-io.yml`, and nothing exercises the tag trigger, the secret pre-flight, the
tag/version comparison, or the 429 retry path short of a real release. The mitigations are that
every non-trivial line of it lives in the script (which *is* exercised, by `--print-order` in the
test and by `--dry-run` in CI), and that a `workflow_dispatch` run against a closure that is already
fully published is a safe end-to-end rehearsal — it exercises checkout, the secret check, the
toolchain, the script and the crates.io API calls, and skips every crate. **Do that once before the
first real tag.**
