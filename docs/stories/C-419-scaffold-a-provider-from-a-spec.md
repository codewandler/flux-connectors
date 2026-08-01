---
id: C-419
title: "A helper writes the patch set, so referencing a spec is cheaper than hand-authoring"
pillar: Build
status: done
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [connector-cli]
note: "the missing half of the front-end. C-411/C-412/C-414 make one statement cover many operations; this writes those statements FROM the document, so a 397-operation connector is generated, reviewed and committed rather than typed"
---

# A helper writes the patch set, so referencing a spec is cheaper than hand-authoring

## Goal
Give the CLI a command that reads a vendored document and emits the provider TOML that references it
— the `[spec]` block, the selectors, the naming pins, the per-operation blocks it cannot infer — so
pointing a connector at a spec is a review of generated text, not an authoring job.

## Acceptance
- [x] `connector-cli scaffold <provider>` reads the vendored document(s) and writes provider TOML to
      **stdout** (never over a file in place — the author diffs and pastes, so a bad run costs
      nothing). A failing-first test scaffolds the babelforce manager document and asserts the output
      loads through `provider::load` without hand-editing.
      → `crates/connector-cli/src/scaffold.rs`, wired at `crates/connector-cli/src/lib.rs:123`;
      `tests/scaffold.rs::scaffolding_the_manager_documents_reads_produces_a_provider_that_loads`
      and `::scaffold_writes_no_file`. Loaded through `load_with_spec` rather than `load` — see
      Progress.
- [x] Selection is an argument, not a guess: `--select` by path prefix and method, matching C-411's
      selector grammar, so the emitted TOML is the selector the author would have written.
      → `scaffold::parse_select`, `<service>:<path_prefix>:<METHOD,METHOD>`;
      `cli.rs::select_is_repeatable_and_scaffold_only`,
      `scaffold::tests::a_select_argument_drops_fields_from_the_right`.
- [x] **Everything the document cannot state is emitted as a hole, not a guess.** `risk` and
      `idempotency` come out as an explicit `TODO` the loader refuses (C-414) rather than a plausible
      default — a scaffold that silently declares every DELETE `low` is worse than no scaffold.
      → `scaffold::Claim`;
      `tests/scaffold.rs::a_document_nobody_has_claimed_emits_a_hole_the_loader_refuses` and
      `::one_unclaimed_operation_makes_the_selector_a_hole_and_loses_no_reviewed_claim`.
- [x] The output is deterministic and canonically formatted: scaffolding twice gives byte-identical
      text, and the emitted TOML round-trips through the loader unchanged.
      → `tests/scaffold.rs::scaffolding_twice_is_byte_identical` plus the round trip above.
- [x] It reports what it could not carry, per operation and by count — a body encoding the IR cannot
      express, a parameter position that is dropped, an operation with no description. Silence about
      a dropped operation is the failure mode this command exists to avoid.
      → `scaffold::Notes` / `write_notes`; `tests/scaffold.rs::it_reports_what_it_could_not_carry`.
- [x] `--diff` compares the document against the connector as it stands and reports what upstream
      added, removed or changed. That is the thing that makes a **re-build** cheap rather than a
      one-off migration.
      → `scaffold::Plan::to_diff`;
      `tests/scaffold.rs::diff_reports_the_document_against_the_connector_as_it_stands`.

## Progress
- Landed as `crates/connector-cli/src/scaffold.rs` (~1,100 lines) plus `tests/scaffold.rs`. Gate
  green: 160 test binaries, `cargo run -p connector-cli -- diff` still reports
  `948 artifacts up to date (53 providers checked)` — the emitted TOML is not an artifact and moves
  nothing.
- **The Acceptance says `provider::load` and the test calls `load_with_spec`.** C-421 landed after
  this story was written and made the pure entry point *refuse* a file that pins a `[spec]`, so a
  spec-backed file can only be compiled through the cache-taking one. That is the same call
  `seam::load` makes for every shipped provider.
- **A claim a human already made is carried forward; a claim nobody has made is a hole.** This is the
  one design decision the story did not spell out, and it is what makes C-420 affordable: a
  `[[patch.select]]` restates `risk`/`idempotency` only when *every* operation it matches is one this
  connector already publishes and all of them agree. The moment it reaches one unclaimed operation it
  states neither, every claimed sibling keeps its own `[[patch.operations]]` block, and C-414 refuses
  over the gap alone. Nothing is ever derived from an HTTP method.
- **`expose` is decided, and only in the withholding direction.** It is the one field where the
  conservative direction is not also the flattering one, so a selector reaching an operation nobody
  has curated states `expose = false` and the existing tools keep `expose = true`. The reasoning is
  at `scaffold::Plan::selector_expose`.
- **An authentication endpoint is never selected**, which is `AGENTS.md`'s owner ruling of
  2026-08-01 (`5de2908`, one commit before this branch point) applied to the one command that
  *proposes* a selection. `/oauth`, `/oauth2`, `/openid` and `/.well-known` are withheld by not being
  selected — the ruling names `expose = false` as not the mechanism — and each is reported with the
  rule quoted, so an exclusion reads as a decision rather than as a coverage gap. A path merely
  *ending* on `token`/`authorize`/`revoke`/`introspect` is **reported and not withheld**: only the
  vendor's prose settles it, and a heuristic deciding what a connector offers is the thing this
  repository does not do. Pinned by `tests/scaffold.rs::an_authentication_endpoint_is_never_selected`.
- Measured on the five vendored babelforce documents by `scaffold babelforce`: **5** operations
  dropped (all `multipart/form-data` uploads), **23** narrower diagnostics, **23** operations with no
  description, **3** authentication-flow endpoints withheld plus **1** ambiguous one reported, and
  **1** operation the connector does not publish (`POST /api/v1/webhook/zendesk`).
  `scaffold babelforce --diff` reports `2 added, 0 removed, 2 changed, 389 unchanged`.
- **Not done, and deliberately:** `[[auth]]` is carried from an existing file and never derived —
  ingest reads a document's operations and not its `securitySchemes`, which is C-5. A new provider
  gets a `TODO(auth)` rather than a guessed credential.

## Notes
- **This is what makes the goal reachable.** 397 operations is not an authoring task at any level of
  manifest ergonomics; C-411/C-412/C-414 reduce the statements, and this writes them.
- It is also what makes C-420's suite-wide rebuild affordable — 53 providers is 53 scaffold runs and
  53 diffs, not 53 authoring jobs.
- **Where it must not go:** no network (`connector-cli` may hold IO but `build`/`diff`/`check` stay
  offline, and this reads vendored bytes like everything else), and it must not write a provider file
  in place. Generated-then-reviewed is the whole safety argument.
- The emitted TOML is **input to a human**, not an artifact. It is not hashed, not in
  `connectors.lock`, and `diff` says nothing about it.
