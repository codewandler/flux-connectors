---
id: C-422
title: "A patch cannot drop a parameter, so one vendor endpoint becomes a 38-argument tool with 17 synonyms"
pillar: Spec
status: in-progress
priority: 2
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [connector-spec]
note: "measured by C-416 on 2026-08-01 — THE one place hand-authoring beat patching. babelforce-call-list goes from 11 curated query parameters to 38, of which 17 are exact synonyms the vendor documents alongside their own aliases"
---

# A patch cannot drop a parameter, so one vendor endpoint becomes a 38-argument tool with 17 synonyms

## Goal
Let a patch omit a parameter the document declares, so a curated operation stays curated when it
moves from hand-authored to spec-backed.

## Acceptance
- [x] A patch can omit named parameters of a selected operation, by name and position.
      → `ParamOmission` (`crates/connector-spec/src/provider.rs:353`) carries a name list per
      position and `OperationPatch::omit` applies it; `omit()` at `provider.rs:992` removes the
      match. Position is half the identity, proven by
      `omitting_a_parameter_from_the_wrong_position_is_refused`.
- [x] **Omission is explicit and never inferred.** A parameter the vendor declares and the connector
      drops is a decision, so it is written down and survives regeneration — the same argument
      `Patch` uses for having no `hide` at the operation level applies one level down and lands the
      opposite way *because the operation is already selected*: the author has stated intent about
      this endpoint, and is narrowing it rather than opting out of review.
      → the asymmetry is argued where the code is, in `ParamOmission`'s doc comment
      (`provider.rs:318-352`), and held by `nothing_is_dropped_unless_the_patch_says_so`: no
      heuristic thins the list, so all 38 survive until 24 names are written down.
- [x] Omitting a **required** parameter is refused — that composes a request the vendor rejects, and
      is the one case where silence would produce a runtime failure rather than a wide tool.
      → `provider.rs:1021`, proven by `omitting_a_required_parameter_is_refused` against
      `exportAgents`'s required `format`.
- [x] Omitting a parameter the document does not declare is a loud error, exactly as an unmatched
      `ParamPatch` correction already is.
      → `provider.rs:1006`, worded off `correct()`'s own refusal and proven by
      `omitting_a_parameter_the_document_does_not_declare_is_refused`.
- [x] `babelforce-call-list` comes back to a curated argument list, and the story records the before
      and after counts.
      → **38 → 14**, measured against `specs/babelforce/manager-2026-07-10.openapi.yaml` itself by
      `nothing_is_dropped_unless_the_patch_says_so` and
      `the_curated_argument_list_comes_back_when_the_patch_names_what_to_drop`.

## Progress
- 2026-08-01 — **Landed.** `[[patch.operations]]` grows `omit`, a `ParamOmission` of names grouped by
  position (`path`/`query`/`header`/`body`). All of `crates/connector-spec/tests/param_omission.rs`
  selects out of the **real vendored manager document**, not a fixture, because the thing under test
  is a real vendor's real synonym flood.
- **The counts, measured rather than quoted: 38 → 14.** The document declares 38 query parameters on
  `listReportingCalls`; 24 names in the patch bring it back to the exact set
  `providers/babelforce.toml` hand-authored — `from`/`to` (the connector publishes the
  `fromNumber`/`toNumber` aliases instead), the eighteen `filters.`-prefixed restatements, and
  `parentId`/`domain`/`source`/`anonymous`. **The story's header said 11; the hand-authored file
  declares 14.** Counted from `providers/babelforce.toml` at `ab2f2d3`; the header is off by three.
- **Cost: 7 lines of TOML** (one `omit.query` key plus six wrapped lines of names). The hand-authored
  equivalent — the `[[operations.params.query]]` region of `babelforce-call-list` in
  `providers/babelforce.toml` at `ab2f2d3` — is **124 lines, 108 of them non-blank and
  non-comment**. A per-name `[[patch.operations.params]]` block would have cost 3 × 24 = 72 lines and
  told a reviewer the same thing 24 times, which is why `omit` is a name list rather than a flag on
  `ParamPatch`.
- **Two refusals beyond the letter of the Acceptance, both the same sentence pointed elsewhere.**
  A path parameter is refused whatever its `required` flag says, because the path template keeps its
  placeholder and `/tickets/{id}` with nothing to fill it is the `PUT /tickets/` defect
  `ParamPatch::required` already exists to correct. And **corrections are applied before omissions**,
  so requiredness is judged as the connector states it: an author who believes the vendor's flag is
  wrong corrects it — a reviewable statement of its own — and is then free to drop the parameter.
  Without that ordering the required refusal would pin an argument into a tool with no way out.
- **A name listed twice is caught for free**: the second lookup finds nothing, so it reports as an
  omission the document does not declare. Same rot, same message.
- **Nothing regenerates.** No shipped provider is spec-backed yet, so `diff` reports
  `557 artifacts up to date`. The capability is proven against the vendored bytes and lands in
  `providers/babelforce.toml` with C-416, which owns that file.
- **This covers only one direction.** See the Notes below on C-6.

## Notes
- **Measured, not anticipated.** C-416's conversion of babelforce cost **293 → 54 declaration lines
  (32.6 → 6.0 per operation)** with **zero parameter patches needed** — the document's descriptions
  and schemas beat the hand transcription everywhere except here. This is the single counter-example,
  and it is a capability gap rather than a cost problem: an `omit` list is ~4 lines against 24.
- **Wanted before C-417.** At 397 operations the same shape recurs across the catalogue, and a tool
  surface full of synonym arguments degrades the model that has to choose between them — which is the
  same argument that motivated C-413's exposure tier, one level down.
- Sequenced with [C-6](C-6-overlay-layer.md), which owns the other open half of the overlay (adding a
  parameter the document omits). These are the two directions of one capability and may well land
  together.
- **They did not land together, and the shape here does not extend to C-6.** `ParamOmission` is a
  list of *names*, which is all a removal needs; an addition needs a whole `Param` — a position, a
  schema, a `wire` alias, a `required` flag — and that is the existing `ParamPatch` with its
  unmatched-name refusal relaxed rather than a second list beside `omit`. Concretely: C-6's half is
  `correct()` at `provider.rs:940` learning to *insert* when the lookup misses **and** the patch says
  so explicitly, which is a different edit in a different function. C-6 stands as cut.
