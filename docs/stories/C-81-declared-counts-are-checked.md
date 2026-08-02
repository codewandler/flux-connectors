---
id: C-81
title: Make the documented provider and artifact counts a checked claim
pillar: Build
status: done
priority: 5
design:
epic: connectors-v1
areas: [connector-cli, docs]
note: drifted five times in one session; every agent noticed and none could fix it
---

# Make the documented provider and artifact counts a checked claim

## Goal
Stop `AGENTS.md` and `README.md` stating provider, operation and artifact counts that nothing
verifies — the numbers drifted five times in a single session, and every implementor noticed while
none could safely fix them.

## Acceptance
- [x] A test compares the counts stated in `AGENTS.md` and `README.md` against a real build:
      providers in `providers/`, curated operations, and planned artifacts. It fails when they
      disagree. `crates/connector-cli/tests/readme_snippet.rs` is the existing home for
      README-derived claims.
- [x] The `diff` output line both documents quote (`N artifacts up to date (M providers checked)`) is
      checked as a string against the real command output, since that one is copy-pasteable and
      therefore the most likely to be trusted.
- [x] Failing-first: bump a stated number, watch the test fail, restore it.
- [x] The test names what to do when it fails — regenerate the number, do not relax the test — because
      the count legitimately changes with every provider.
- [x] `AGENTS.md`'s existing caveat that the artifact count is "not a permanent invariant" is
      reconciled with the test: not permanent, but *checked at every commit*.

## Progress
- 2026-08-02: implementation started alongside the coordinator-owned whole-catalogue integration;
  the check will be proven against the final catalogue counts before release.
- 2026-08-02: failing-first evidence — after the real-plan figures were stated, `README.md`'s
  provider count was deliberately bumped from 54 to 55. `cargo test -p connector-cli --test
  readme_snippet the_documented_catalogue_counts_match_the_build_plan -- --exact --nocapture`
  failed with `README.md does not state the full build's 54 providers, 65 services and 727 curated
  operations (997 artifacts total). Regenerate these stated numbers from pipeline::plan; do not
  relax this test.` The count was then restored to 54.
- 2026-08-02: `cargo fmt --all --check` and `cargo clippy -p connector-cli --test readme_snippet --
  -D warnings` pass. The restored focused test reaches the coordinator-owned integration boundary:
  `cargo run -p connector-cli -- diff | tail -1` reports `2 artifacts would change (54 providers
  checked)`, and the test names those two whole-catalogue artifacts as `connectors.lock` and
  `web/public/catalog.json`. C-81 does not regenerate either file.
- 2026-08-02: coordinator integration regenerated the full catalogue and remeasured 54 providers,
  65 services, 735 curated operations, and 1,005 artifacts. The focused checked-count test and exact
  `1005 artifacts up to date (54 providers checked)` assertion pass against the committed plan.

## Notes
- **This is the same failure C-54 removed one layer down.** C-54 deleted five hand-maintained provider
  lists in test code because duplication silently dropped coverage; these are hand-maintained provider
  counts in prose, and they silently drop accuracy. The fix is the same shape: derive, do not restate.
- Every provider story in the fleet flagged the staleness in its report and correctly declined to
  touch the file, because four concurrent siblings each invalidated whatever number the others wrote.
  A coordinator fixing it by hand once per wave is not a fix; it is the same manual step that already
  failed five times.

## Findings added 2026-07-31 (from the C-133 dispatch)

Two more documented claims that are false today, both in `AGENTS.md`, both found while a provider
implementor was following that file as instructions:

- **`AGENTS.md:35` says 43 providers.** `ls providers/*.toml | wc -l` is **44**, and
  `web/public/catalog.json` agrees: 44 providers, 51 services, 248 operations, 8 events, 2 channel
  bindings. `README.md` carries the same stale figures (43 / 242 / 470). This is the drift this
  story exists to end, now measured a third time.
- **`AGENTS.md:153` instructs a provider implementor to add `specs/<id>/v1.json`** as part of the
  scoped-build recipe. **No such directory exists for any vendor** — `specs/` contains only `flux/`,
  and no shipped provider declares a `[spec]` block at all. An implementor following the recipe
  literally would create a file nothing reads. This one is worse than a wrong count: it is a wrong
  *instruction*, in the section agents are told to follow exactly, and it silently misdirects every
  future provider story.

The second is arguably not this story's job — a count checker will not catch a stale sentence. But it
is the same failure with the same cause, so it is recorded here rather than lost: **the operating
contract is not checked against the repository it describes.**

## Drift observation, 2026-07-31 (coordinator, at C-165 integration)

**Sixth drift, and the first where the documents disagreed with each other rather than merely with
the build.** Three counts, three different answers, none correct:

| document | claimed | actual |
|---|---|---|
| `README.md` | 43 providers, 50 services, 242 operations, 470 artifacts | 45 / 52 / 254 / 488 |
| `docs/roadmap.md` | 43 providers, 50 services, 242 operations | 45 / 52 / 254 |
| `docs/integrating-with-flux.md` | 44 providers, 51 services, 248 operations | 45 / 52 / 254 |

Corrected by hand, again — which is the mechanism this story exists to replace, applied a sixth time.
Worth noting for whoever implements it: `integrating-with-flux.md` **already** carries the instruction
*"measured from `web/public/catalog.json`, not from prose … Re-measure before quoting"*, and it still
drifted. A prose instruction to re-measure is not a check; the numbers have to be generated or
asserted. The measurement is one `python3` pass over `web/public/catalog.json` plus
`connector-cli -- diff` for the artifact count, so the checkable version is cheap.
