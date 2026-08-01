---
id: C-421
title: "No shipped provider can become spec-backed, because loading one is not spec-aware"
pillar: Spec
status: in-progress
priority: 1
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [connector-spec, connector-cli]
note: "found by C-416 on 2026-08-01 and it blocks the epic outright — `provider::load` takes no spec cache, so a spec-backed provider loads as a ZERO-OPERATION SKELETON. 91 files call it, 86 of them tests. C-416 and C-417 are both stuck behind this"
---

# No shipped provider can become spec-backed, because loading one is not spec-aware

## Goal
Make loading a provider file mean the same thing everywhere, so converting a shipped provider to
`[spec]` does not silently turn it into a connector with no operations for most of the workspace.

## Acceptance
- [x] **Plain `provider::load` on a spec-backed file no longer returns a skeleton.** It either
      resolves the spec cache or **refuses loudly** — a zero-operation connector that looks
      successfully loaded is exactly the "plausible but incorrect" outcome `AGENTS.md` refuses. Decide
      which, state the reasoning where a reader will find it, and make the failing-first test the one
      that proves the old behaviour was wrong.
      → **It refuses.** `crates/connector-spec/src/provider.rs:634` (`no_spec_cache`, reached from
      `load_inner`'s `None` arm at `:613`). The reasoning is on `load`'s own rustdoc
      (`provider.rs:438-465`) and in `docs/designs/spec-front-end.md` §"Loading a provider file, once
      the front-end is real". Failing-first test:
      `spec_backed_provider.rs::plain_load_refuses_a_spec_backed_file_rather_than_returning_a_skeleton`,
      which at the merge base failed with `operations: []` returned as `Ok`.
- [x] Every caller is accounted for. 91 files call `provider::load`; 86 are tests. Each either moves
      to the spec-aware entry point or keeps the pure one **deliberately**, and the split is
      explained once rather than per call site.
      → The rule is stated once, in `crates/connector-spec/tests/support/shipped_provider.rs`'s module
      doc: **bytes read from `providers/` go through the helper; TOML a test authored itself goes
      through `provider::load`.** 72 call sites in 66 files moved; the 59 that remain are all
      self-authored fixtures (`fixture.toml`, `acme.toml`, `okta-probe.toml`, `fuzz.toml`,
      `ZENDESK_TOML`), which are what the pure loader is *for*.
- [x] `cargo test --workspace --no-fail-fast` is green with `providers/babelforce.toml` spec-backed.
      That is the whole point: C-416 measured **53 failures across 18 binaries in 4 crates** the
      moment one shipped provider converts.
      → Reproduced exactly (53) at the merge base, then measured at **2** with this diff — see
      Progress. Both survivors are the fenced C-126 constants below, which an implementor may not move.
- [x] Tests that encode hand-authored babelforce shapes are rewritten against what the document
      actually declares, not deleted: `babelforce_ivr.rs::babelforce_nests_the_presence_label`,
      `::babelforce_sends_its_free_form_session_bodies`, and
      `connector-pack/tests/request.rs::a_free_form_body_travels_whole_in_either_spelling` (which only
      needs repointing at `babelforce-session-update`, still free-form).
      → All three rewritten so they hold in **both** front-ends, which they must: babelforce is still
      hand-authored on `main`. The first two live in
      `crates/connector-cli/tests/shipped_providers_build.rs:245-352` (not `babelforce_ivr.rs` — the
      story misnamed the file). The third is repointed as directed.
- [ ] Any C-126 response-schema figure that moves is re-baselined **with the new number stated**, not
      silently relaxed — babelforce goes from 0/9 response schemas to 9/9, which is the floor rising.
      → **Numbers stated, constants deliberately not moved.** `AGENTS.md`:"an implementor never raises
      it, the coordinator raises it at integration", of both `COVERED_FLOOR` and `ABSENCE_CEILING`.
      They are also green on this branch and only move when the conversion lands. See Progress for the
      exact figures.

## Progress
- 2026-08-01 — Filed from C-416's BLOCKED report. C-416's own branch (`impl/C-416`) is preserved and
  is the reproduction: it converts babelforce and shows exactly which binaries go red.

### 2026-08-01 — C-421 implemented. **53 → 2**, and both survivors are coordinator-owned.

**The reproduction was exact.** Taking `providers/babelforce.toml` from `impl/C-416` into a clean
worktree at the merge base and running the suite produced **53 failing tests in 17 binaries**, the
same 53 C-416 named. (C-416 counted 18 binaries; the eighteenth is `connector-pack`'s `request`, which
only goes red once the generated artifacts are rebuilt — `cargo run -p connector-cli -- build` after
the conversion regenerated 12 artifacts **byte-identical to C-416's**, and the eighteenth then
appeared.)

**Design decision: the pure entry point stays pure and refuses.** The two options the story posed are
not symmetric. Folding the cache into `load` as a `documents` parameter looks like it gives "load" one
meaning, but the callers who have no cache — most of them, and every test that authors its own TOML —
can only pass `&[]`, and `&[]` against a pinned `[spec]` *already* refuses one layer down in
`ingest_specs`. So the parameter buys one signature and two meanings, the second spelled `&[]`, plus a
vestigial argument on ~40 golden-error tests. Refusing costs one message and no argument, and leaves
all 53 hand-authored providers loading byte-identically. Full argument on `provider::load`'s rustdoc,
which is where someone choosing between the two entry points is standing.

**The part that actually makes conversion cheap is the test-side seam, not the refusal.** There was no
shared way to load a shipped provider — 18 binaries, each with its own loader — so one wrong
convention was replicated everywhere. `crates/connector-spec/tests/support/shipped_provider.rs` is now
the single one: it reads the definition *and* every document under `specs/<name>/` and calls
`load_with_spec`, passing the **whole** cache so the pin is resolved where the pin is read. It is
`#[path]`-included by `connector-spec`, `connector-flux` and `connector-cli` rather than being a
fourth crate, because a crate would mean `dev-dependencies` edges in three fenced manifests.
**Consequence for C-417 and C-420: a provider converting to `[spec]` now needs no test change at all.**

**The 2 that remain, with C-416 applied**, are `response_schema_coverage.rs`'s
`the_recorded_ceiling_is_the_measured_absence` and `a_connector_arriving_with_no_response_shapes_is_caught`.
Both read `ABSENCE_CEILING`, which `AGENTS.md` fences as coordinator-owned. Measured with the
conversion applied:

| constant | now | measured with babelforce spec-backed | action |
|---|---:|---|---|
| `ABSENCE_CEILING` | 33 | absence is **22 of 299** (was 31); slack is 2 | lower to **24** — the value that satisfies both directions of the ratchet |
| `COVERED_FLOOR` | 250 | coverage is **277 of 299**; slack is `299/10 = 29` | green either way; **277** is the honest re-baseline |

babelforce moves **0/9 → 9/9**, which is the whole of the change. Neither constant is touched here:
lowering `ABSENCE_CEILING` to 24 while babelforce is still hand-authored turns
`response_schema_coverage_does_not_fall_below_its_floor` red on this branch (absence is 31 there), so
it must move in the commit that lands the conversion — C-416's re-run — and not before.

**Three assertions were rewritten to hold in both front-ends**, because this branch must be green with
babelforce hand-authored *and* with it spec-backed:

- `babelforce_nests_the_presence_label` asserted the literal payload line
  `{ enabled, presence: { name: presence_name } }`. It now asserts the body's **root key set** is
  `["enabled", "presence"]`, which both spellings satisfy and which a flat root `name` — the mistake
  the test exists to catch, and the one babelforce answers 200 to without applying — still fails.
- `babelforce_sends_its_free_form_session_bodies` counted **two** `parse(body, as: "json")` bodies. It
  now names `babelforce-session-update` alone. The second was `babelforce-call-session-set`, whose
  body shape is an open *vendor* question (C-416 Progress §(a): bare map vs. `{"variables": …}`
  wrapper), and a test about the emitter should not have its verdict depend on that.
- `a_free_form_body_travels_whole_in_either_spelling` is repointed at `babelforce-session-update`, as
  directed.

**Two fixtures that were passing vacuously now do not.** `provider_toml.rs`'s spec-pointer tests and
`lockfile.rs`'s two `[spec]` fixtures went through plain `load`, which read no document — so
`select = "listAgents"` was accepted while matching nothing. Both now supply a cache and compile for
real; `provider_toml.rs` gained a two-operation fixture document and its patch set now actually
applies.

## Notes
- **This is the epic's critical path.** C-416 (reproduce the nine) and C-417 (full babelforce surface)
  are both blocked behind it, and C-420 multiplies it by every provider that converts.
- The design question the C-416 implementor surfaced, worth answering rather than routing around:
  `connector-spec` is deliberately pure (bytes → IR, no IO), and the spec cache is a **directory**.
  So either `load` grows a parameter carrying already-read documents — keeping purity — or the pure
  entry point stays and refuses a spec-backed file, with a second entry point for the full form. The
  first keeps one meaning for "load"; the second keeps one signature. Pick, and say why.
- Do **not** lift `validate_verify` to make the failures go away. 38 of the 53 trace to that one line,
  and the C-416 implementor tried it: `every_shipped_provider_loads` then correctly reports "declares
  no operations", which is the same defect one layer down.
