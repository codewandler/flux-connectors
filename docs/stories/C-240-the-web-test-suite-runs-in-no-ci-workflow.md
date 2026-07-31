---
id: C-240
title: "The site's 32-test explorer suite runs in no CI workflow, so it guards nothing that a push can see"
pillar: Build
status: ready
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

- [ ] **Failing-first:** a deliberately broken assertion in `web/test/explorer.test.mjs` fails the
      workflow. It does not today. Show the run.
- [ ] `npm test` runs in CI after `npm run build`, in that order — the suite asserts against
      `.vitepress/dist`, so running it first reports **10 spurious failures**. That ordering is the
      whole trap and should be stated where the step is added, not just obeyed.
- [ ] Decide whether it belongs in `pages.yml` (which already has the Node setup and the built site)
      or in `ci.yml` (which is where a *gate* lives, and which runs on pull requests rather than only
      on a push to `main`). Record the reason. A test that only runs on the deploy path does not
      block a bad merge.
- [ ] `AGENTS.md`'s web gate section says where it runs, so the documented gate and the enforced gate
      are the same thing.

## Notes

- Small and independent. It does not need the host explorer epic and should not wait for it.
- The Rust gate is unaffected: `ci.yml` already runs build, test, clippy and fmt across the
  workspace.
- Do not take the opportunity to add a second Node dependency here. The site's *"exactly one
  dependency"* property is deliberate and worth keeping — see `web/package.json` and
  `SchemaBlock.vue`'s comment on hand-rolling a 40-line highlighter rather than pulling one in.
