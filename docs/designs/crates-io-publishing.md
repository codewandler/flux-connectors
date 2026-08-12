# Design: publishing to crates.io

**Status:** approved · **Pillar:** Build · **Stories:**
[C-195](../stories/C-195-crates-io-release-workflow.md), [C-190](../stories/C-190-publish-catalog-pack-secrets.md)

> The original closure facts below were measured on 2026-07-31 against this workspace at `0.5.0`.
> The settled package names in §3 were re-measured against the live crates.io API on 2026-08-04.

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

## 1. The closure is five crates, not three

C-190 names three consumable crates. The dependency graph said four when this was written, and says
five since C-537 gave the catalogue a data crate to sit on.

```
catalog-reader                                 (no dependencies at all)
catalog-reader → connector-catalog             (the pack, re-exported as `catalog::reader`)
connector-address → connector-secrets          (CredentialRef, Layout, TenantLayout)
connector-catalog, connector-secrets → connector-pack
```

`connector-secrets` does not define credential addressing; it re-exports `CredentialRef` from the
crate that does (deliberately — C-90 says there is one addressing scheme, not two). That type is in
`connector-secrets`' *public API*, so whatever crate owns it is published or nothing outside this
workspace resolves. The `path` dependency that makes it work here does not travel in a `.crate`.

Derived publish order:

| # | crate | why it is here |
|---|---|---|
| 1 | `connector-address` | **not requested — forced** by `connector-secrets`' public API |
| 2 | `catalog-reader` | **not requested — forced** by `connector-catalog`'s public API (C-537); zero dependencies of its own |
| 3 | `connector-catalog` | consumable; needs `catalog-reader` live |
| 4 | `connector-secrets` | consumable; needs `connector-address` live |
| 5 | `connector-pack` | consumable; needs `connector-catalog` and `connector-secrets` live |

Not published: `connector-cli` (this repository's own build tool), `connector-flux` (reachable only
from it) and `connector-spec` (the compiler). None is in the closure of any consumable crate, so
none is forced.

**This is a finding for C-190, not a decision taken here.** It changes that story's arithmetic:
four new crates against the crates.io new-crate rate limit rather than three, and one more permanent
name to settle. It also means `connector-catalog` is no longer quite the "publishable on its own"
crate the notes assume — it was, until C-537 put `catalog-reader` underneath it; `connector-secrets`
never was; and `connector-pack` now needs four predecessors live.

### The forced crate used to be the compiler (C-407)

Until C-407 the forced crate at position 2 was **`connector-spec`**: the connector IR, provider TOML
and OpenAPI ingest, validation and the lockfile writer — 11,832 lines and 128 top-level `pub` items
— published permanently so that a 726-line address module resolved for `connector-secrets`.

The derivation was never wrong; it reported the edge faithfully. What was wrong was the edge. So the
vocabulary moved into `connector-address`, a crate whose whole content is how a provider, a service,
an operation and a tenant's credential are *named*, and the dependency **inverted**: `connector-spec`
is now a consumer of it, because `Connector::gid_of` and `Connector::credential_ref_for` derive
addresses from a loaded connector. Moving `credential` into `connector-secrets` instead was rejected
— it would have pointed the compiler at a host library.

An address vocabulary is a reasonable thing to have in a published API. A compiler is not, and
`publish_closure.rs::no_machinery_crate_is_published` is what states that as a property rather than
leaving it to whoever next reads a derived list.

### The order is derived, not listed

`../flux` hand-lists 29 crates because its graph carries ordering constraints a manifest does not
state (an optional feature dependency; a protocol line versioned independently of the runtime). This
workspace has five crates and two non-obvious edges — the `CredentialRef` re-export and the pack
re-exported as `catalog::reader` — so a topological sort over the manifests is
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

## 3. Crate names — settled public contract

The repository chose the organization-prefixed package names before first publication. Re-measured
2026-08-12 with `curl -s https://index.crates.io/co/de/<name> | tail -1`, which reports
`"vers":"0.22.0"` and `"yanked":false` for each of these five:

| package name | Rust library name |
|---|---|
| `codewandler-connector-address` | `connector_address` |
| `codewandler-connector-catalog-reader` | `catalog_reader` |
| `codewandler-connector-catalog` | `catalog` |
| `codewandler-connector-secrets` | `connector_secrets` |
| `codewandler-connector-pack` | `connector_pack` |

Those package names are permanent. Manifests, packaged README dependency examples, `cargo add`
commands, crates.io links and docs.rs metadata must use the `codewandler-connector-*` names. Rust
source continues to use the shorter `[lib]` names. The workflow and publisher still derive names
from manifests, while `publish_closure.rs` pins the install-facing and documentation pointers to
the same public package names.

## 4. Metadata

`description`, `license`, `repository`, `readme`, `keywords` on all five, plus `documentation` and
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
