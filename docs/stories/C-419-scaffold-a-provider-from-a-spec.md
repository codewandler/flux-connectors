---
id: C-419
title: "A helper writes the patch set, so referencing a spec is cheaper than hand-authoring"
pillar: Build
status: ready
priority: 1
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
- [ ] `connector-cli scaffold <provider>` reads the vendored document(s) and writes provider TOML to
      **stdout** (never over a file in place — the author diffs and pastes, so a bad run costs
      nothing). A failing-first test scaffolds the babelforce manager document and asserts the output
      loads through `provider::load` without hand-editing.
- [ ] Selection is an argument, not a guess: `--select` by path prefix and method, matching C-411's
      selector grammar, so the emitted TOML is the selector the author would have written.
- [ ] **Everything the document cannot state is emitted as a hole, not a guess.** `risk` and
      `idempotency` come out as an explicit `TODO` the loader refuses (C-414) rather than a plausible
      default — a scaffold that silently declares every DELETE `low` is worse than no scaffold.
- [ ] The output is deterministic and canonically formatted: scaffolding twice gives byte-identical
      text, and the emitted TOML round-trips through the loader unchanged.
- [ ] It reports what it could not carry, per operation and by count — a body encoding the IR cannot
      express, a parameter position that is dropped, an operation with no description. Silence about
      a dropped operation is the failure mode this command exists to avoid.
- [ ] `--diff` compares the document against the connector as it stands and reports what upstream
      added, removed or changed. That is the thing that makes a **re-build** cheap rather than a
      one-off migration.

## Progress
- (not started)

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
