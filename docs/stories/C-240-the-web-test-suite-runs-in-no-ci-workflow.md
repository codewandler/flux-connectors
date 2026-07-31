---
id: C-240
title: "The site's 32-test explorer suite runs in no CI workflow, so it guards nothing that a push can see"
pillar: Build
status: in-progress
priority: 1
design:
epic:
areas: [build, web]
note: "found while planning the host explorer 2026-07-31 and verified: .github/workflows/pages.yml runs `npm ci` and `npm run build` and stops. ci.yml is Rust-only. The suite is green today at 32/32 — locally, on whoever remembers"
---

# The web test suite runs in no CI workflow

## Goal

Make the explorer's 32 assertions a gate rather than a habit.

## What was measured

`.github/workflows/pages.yml` runs exactly two npm commands:

```
:44   run: npm ci
:48   run: npm run build
```

There is no `npm test`. `.github/workflows/ci.yml` is Rust-only. So
`web/test/explorer.test.mjs` — **1,631 lines, 32 tests** — runs only when a person remembers to run
it locally.

It is green today: I ran `cd web && npm ci && npm run build && npm test` → **32 passed, 0 failed**.
That is the point. Nothing would have told us otherwise.

## Why it matters more than its size suggests

`AGENTS.md` documents the web gate as `npm ci && npm run build && npm test`, so the repository
already believes this runs. What the suite guards is not cosmetic:

- **the honesty rules** — that an operation owning a defect says so wherever it appears, and that one
  merely inheriting a wider condition is *not* presented as broken;
- **the no-JavaScript guarantees** — every operation has a deep-linkable pre-rendered page;
- **the architecture guards** — `no_component_imports_the_site_framework`, and that nothing about the
  catalogue is hand-maintained in the explorer sources;
- **layout regressions**, asserted against the emitted stylesheet rather than screenshots.

Every one of those is a property a future change can break silently.

There is a second-order reason to fix it now:
[C-239](C-239-a-test-harness-for-the-host-page.md) adds a JS harness for the host's page, and
[C-238](C-238-the-host-mounts-the-explorer-components.md) makes the console a second consumer of
these components. A new harness wired into nothing would inherit exactly this state.

## Acceptance

- [x] **Failing-first:** a deliberately broken assertion in `web/test/explorer.test.mjs` fails the
      workflow. It does not today. Show the run.
- [x] `npm test` runs in CI after `npm run build`, in that order — the suite asserts against
      `.vitepress/dist`, so running it first reports **10 spurious failures**. That ordering is the
      whole trap and should be stated where the step is added, not just obeyed.
- [x] Decide whether it belongs in `pages.yml` (which already has the Node setup and the built site)
      or in `ci.yml` (which is where a *gate* lives, and which runs on pull requests rather than only
      on a push to `main`). Record the reason. A test that only runs on the deploy path does not
      block a bad merge.
- [x] `AGENTS.md`'s web gate section says where it runs, so the documented gate and the enforced gate
      are the same thing.

## Progress

Landed as the `web` job of `.github/workflows/ci.yml`, plus `web/test/ci_gate.test.mjs` and the
`AGENTS.md` §Validation rewrite. Three measurements taken here correct the text above; they are
recorded rather than silently absorbed.

**The count in the second item is wrong: it is 19, not 10.** Measured 2026-08-01 at `cecf320`, in a
worktree with no `.vitepress/dist`: `npm test` reports 13 pass, **19 fail** of 32. The first is
`.vitepress/dist/operations/airtable-record-get.html was not built`. The number presumably drifted
upward with the catalogue as more pages became deep-linkable; 19 is what the workflow comment and
`AGENTS.md` now state. The acceptance is ticked on the *ordering*, which is what it is about.

**The third item's premise about `pages.yml` is also wrong, and it does not change the outcome.**
`pages.yml` does *not* run only on push to `main` — it has had `pull_request:` since it was written
(`:19`), with a comment saying the build job is a gate; only `deploy` is push-only, via
`if: github.event_name != 'pull_request'`. So siting the step there would in fact have blocked a bad
merge today. `ci.yml` was still chosen, for reasons that survive that correction: `pages.yml`'s PR
trigger is a documented add-on to a workflow that exists to publish, so the gate would inherit the
deploy path's trigger list and any future narrowing of it; a red step in its build job does not
distinguish a test failure from Pages plumbing; and `AGENTS.md` §Validation describes exactly two
gates, which now both live in `ci.yml`. The full argument is in the job comment. Cost accepted: the
site builds twice per PR. `pages.yml` is untouched.

**A guard test was added beyond the literal acceptance.** The defect this story fixes was
documentation running ahead of enforcement — `AGENTS.md` described this gate while nothing ran it —
so `web/test/ci_gate.test.mjs` (3 tests, no new dependency, hand-rolled reader for the workflow YAML
subset) asserts that some workflow a pull request triggers runs the suite, that it builds before it
tests, and that the gate `AGENTS.md` documents is one a workflow enforces. It rides in the suite it
guards; that limitation is commented at the top of the file. A non-circular version belongs in the
Rust gate and is a wider surface than this story owns.

## Notes

- Small and independent. It does not need the host explorer epic and should not wait for it.
- The Rust gate is unaffected: `ci.yml` already runs build, test, clippy and fmt across the
  workspace.
- Do not take the opportunity to add a second Node dependency here. The site's *"exactly one
  dependency"* property is deliberate and worth keeping — see `web/package.json` and
  `SchemaBlock.vue`'s comment on hand-rolling a 40-line highlighter rather than pulling one in.
