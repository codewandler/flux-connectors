---
id: C-429
title: "`build` cannot see a committed artifact it did not write, so a stale rendering ships indefinitely"
pillar: Build
status: done
areas: [connector-cli]
note: "hit THREE times in one story (C-417) — connectors/babelforce.flux, its manifest, and crates/catalog/ops/babelforce/babelforce-token.flux each survived a full `build` + `diff` reporting 'up to date' while belonging to no plan. All three were deleted by hand, which is not a mechanism"
---

# `build` cannot see a committed artifact it did not write, so a stale rendering ships indefinitely

## Goal
Make the build able to say which committed artifacts it did **not** write, so an artifact whose
source stopped existing is reported rather than left on disk looking current.

## The defect, measured

`cargo run -p connector-cli -- diff` compares each **planned** artifact against what is committed. It
has no view of the inverse: a tracked file under an artifact root that **no plan claims**. So a
rendering whose operation was deselected, renamed, or moved to another service is simply never
looked at again, and `diff` keeps reporting `N artifacts up to date` with a straight face.

C-417 produced three in one story:

| Orphan | Why it stopped being written |
|---|---|
| `connectors/babelforce.flux` | babelforce gained services, so the module is now `babelforce-<service>.flux` |
| `connectors/babelforce.connector.toml` | same |
| `crates/catalog/ops/babelforce/babelforce-token.flux` | the operation was withheld at review |

Each survived a full `build` **and** a `diff` reporting every artifact up to date. All three were
deleted by hand — which caught them this time because the diff was under review, and would not have
next time.

**Why it matters more now than a week ago.** Until this week every provider was hand-authored and its
artifact set changed only when a human edited the file. Selection is now declarative (C-411): one
edit to a `path_prefix` can deselect dozens of operations at once, and every one of their renderings
becomes an orphan silently. The catalogue is also 948 artifacts, so nobody is going to notice by
looking.

## Acceptance
- [x] `build` (and `diff`) enumerate what is **tracked** under each artifact root and compare it
      against what the plan claims. A tracked file no plan claims is reported, naming the file and
      the root.
- [x] **A failing-first test creates an orphan and asserts today's build reports it up to date**,
      then that the change catches it. The three from C-417 are the fixtures — reproduce one.
- [x] `diff` **exits non-zero** on an orphan, so CI fails. An orphan is drift in exactly the sense
      `connectors.lock` exists to catch, and vision principle 1 says drift is detected, not absorbed.
- [x] `build` removes it, or refuses and says what to remove. Decide which and record why — deleting
      a tracked file automatically is a real hazard if the root is ever mis-derived, and refusing is
      the conservative reading of "a loud compile-time refusal is better than plausible but
      incorrect" (`AGENTS.md` § Non-negotiable engineering rules).
- [x] The artifact roots are **derived**, never a hand-typed list — the same rule the publish closure
      already follows. A root missed by a list is an orphan class nobody will find.

## Progress

**Landed. `build` refuses and names; it never removes.**

*Remove vs refuse — the decision and why.* `build` refuses before writing anything and prints each
orphan with the root it sits under and the `git rm` that clears it. Three reasons, in order of
weight. (1) A root is *derived* from what the emitter says it writes, so an emitter bug is a
mis-derived root, and a build that deletes on a mis-derived root turns a bug into data loss —
`AGENTS.md` § Non-negotiable engineering rules already decides this shape of question. (2) Removal
is the cheap half: one `git rm`, reviewed in the same diff as the change that orphaned the file,
whereas *noticing* was the expensive half and is what this story automates. (3) `pipeline::apply` is
documented as the only function in the crate that writes an artifact; teaching it to delete would
retire that structural property in exchange for saving a command. The refusal is placed before any
write, so a refused build leaves the tree exactly as it found it — asserted by
`a_refused_build_writes_nothing_and_deletes_nothing`.

*How the roots are derived.* Every planned artifact now declares a `pipeline::Ownership`, and
`planned()` takes it as a required argument — an artifact cannot reach the tree without its author
answering "which directory does this family own?", which is the forcing function that keeps a root
from being forgotten. `Ownership::Family(root)` yields the four roots in play today —
`connectors/`, `crates/catalog/ops/`, `crates/catalog/src/generated/`, `web/public/v1/` — and
`Ownership::Singleton` marks the four files a full run always writes (`connectors.lock`,
`crates/catalog/src/generated.rs`, `web/public/catalog.json`, the two README SVGs). The distinction
is load-bearing rather than cosmetic: a singleton's *directory* is not a root, which is what keeps
`Cargo.lock` and `crates/catalog/src/lib.rs` out of the report.
`a_directory_holding_one_whole_catalogue_file_is_not_a_root` pins both.

*What counts as an orphan.* A file under a root, at any depth, sharing an extension with something
the plan writes into that root. The shape test is derived the same way the roots are, and it is what
keeps `crates/catalog/ops/README.md`, `assets/readme-snippet.flux` and `assets/brand/*.svg` out of
the report — a false positive in a gate is how a gate stops being read.

*"Tracked" is read as "on disk", deliberately.* The enumeration walks the working tree rather than
shelling out to `git ls-files`. `build` and `diff` are hermetic and process-free — nothing in this
crate spawns a subprocess, and the fixture trees the integration tests build are not repositories,
so a git-backed check could not be exercised where it matters. It also answers the better question:
a `.flux` file in `connectors/` that nothing writes is a problem whether or not it has been committed
yet. The narrowing that makes this safe is the shape test above, which leaves untracked scratch of
any other name alone.

*Scoping.* `Plan::orphans` is empty on a `--provider`/`--service` run by construction, since such a
run compiled a subset and every unvisited provider's artifacts would read as unclaimed. Same rule as
`connectors.lock` one layer down.

*Measured.* `cargo run -p connector-cli -- diff` → `943 artifacts up to date (53 providers checked)`,
exit 0: the committed tree carries no orphan, which is also the false-positive floor and is asserted
by `the_committed_tree_carries_no_orphaned_artifact`. With two files planted under real roots the
same command exits 1 and names both.

## Notes
- Found three times over by the C-417 implementor, which is the whole argument: a defect that bites
  three times in one story is not an edge case, and the fix is not "look harder next time".
- Related but distinct from C-189: `connectors.lock` records what the build **did** write and hashes
  it. The gap is the complement — what is on disk that the build did not write. The lockfile is
  probably the right place to answer it from, since it is already the complete list of claimed
  artifacts.
- Watch the interaction with `--provider`/`--service` scoping: a scoped build legitimately writes a
  subset, so orphan detection must run against a whole-catalogue plan or be skipped, never run
  against a partial plan. `lockfile.rs::a_scoped_build_leaves_the_lockfile_byte_identical` records the
  same hazard one layer down.
