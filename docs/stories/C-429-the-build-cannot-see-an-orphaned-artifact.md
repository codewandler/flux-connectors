---
id: C-429
title: "`build` cannot see a committed artifact it did not write, so a stale rendering ships indefinitely"
pillar: Build
status: ready
priority: 2
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
- [ ] `build` (and `diff`) enumerate what is **tracked** under each artifact root and compare it
      against what the plan claims. A tracked file no plan claims is reported, naming the file and
      the root.
- [ ] **A failing-first test creates an orphan and asserts today's build reports it up to date**,
      then that the change catches it. The three from C-417 are the fixtures — reproduce one.
- [ ] `diff` **exits non-zero** on an orphan, so CI fails. An orphan is drift in exactly the sense
      `connectors.lock` exists to catch, and vision principle 1 says drift is detected, not absorbed.
- [ ] `build` removes it, or refuses and says what to remove. Decide which and record why — deleting
      a tracked file automatically is a real hazard if the root is ever mis-derived, and refusing is
      the conservative reading of "a loud compile-time refusal is better than plausible but
      incorrect" (`AGENTS.md` § Non-negotiable engineering rules).
- [ ] The artifact roots are **derived**, never a hand-typed list — the same rule the publish closure
      already follows. A root missed by a list is an orphan class nobody will find.

## Progress
- (not started)

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
