---
id: C-239
title: "A behavioural change to the host's page cannot ship with a failing-first test, because nothing can test it"
pillar: Build
status: ready
priority: 1
design: docs/designs/host-explorer.md
epic: host-explorer
areas: [host, build]
note: "LANDS FIRST — every item in C-237 is a behavioural change to a file nothing can test. Recorded debt, not a new finding: C-234 could not close its mutation M15 for this reason and said so in its Acceptance. AGENTS.md requires a failing-first test for a behavioural change, and index.html is the one surface where that is impossible"
---

# A test harness for the host's page

## Goal

Make a behavioural change to `crates/connectors-api/src/index.html` provable the way every other
behavioural change in this repository is.

## What was measured

`AGENTS.md` requires a failing-first test for a behavioural change. `index.html` is ~260 lines of
JavaScript that nothing can execute under test.

This is **recorded debt, not a discovery**. C-234's security review ran 16 mutations; three survived,
and the review named the reason for one of them:

> M15 — `index.html` draws the button unconditionally → **GREEN**. No JS harness exists for this file;
> the server half *is* pinned (M12 red).

C-234's own Acceptance records it as deliberately not done: *"needs a JS harness this crate does not
have — its own story."* Two of the three surviving mutations in that review were closed by tests once
the target was reachable from Rust; the third was not, because the target is in the page.

The two existing tests that touch `/` assert **no markup at all**:
`tests/host.rs::a_stored_credential_reaches_no_surface` (2xx, and none of three sentinels in the
body) and `without_a_google_registration_the_host_still_starts_and_explains_itself` (200 on an
unconfigured host). Nothing greps the HTML, so a redesign is free — and so is a regression.

## Acceptance

- [ ] **Failing-first test:** a mutation of the page's behaviour turns a test red. Demonstrate it
      against **C-234's M15** specifically — changing `if (status.dev)` to `if (true)` in
      `crates/connectors-api/src/index.html` must fail, which it does not today. That is the exact
      mutation a reviewer could not close, so it is the honest proof this story worked.
- [ ] The harness follows `web/test/explorer.test.mjs`'s shape rather than inventing one: `node
      --test`, zero dependencies, asserting against built output **and** the emitted stylesheet.
      That is how the site catches layout regressions without screenshots, and it already runs in
      this repository.
- [ ] It runs in the gate, and `AGENTS.md` records where — a harness nobody is pointed at is a
      harness nobody uses.
- [ ] The three sign-in states are each pinned: unconfigured → setup instructions; signed out →
      doors; signed in → catalogue. The first is a first-run path a Rust test already covers at the
      status-code level and nothing covers at the content level.
- [ ] The dev button's `status.dev` guard is pinned, closing C-234's M15 by name.

## Sequencing — this lands before C-237, not after

Every item in [C-237](C-237-the-host-explorer-is-a-console.md) is a behavioural change to
`index.html`, and `AGENTS.md` requires a failing-first test for each. Doing the console first means
either shipping it untestable or writing the harness under time pressure in the middle of it.

## The harness, concretely

`node --test` + `happy-dom`, driving the served page with a stubbed `fetch`. One dependency, matching
the repository's existing `node:test` norm, and it tests the bytes an operator actually receives —
the same reason `web/test/explorer.test.mjs` runs against the *built* site rather than the sources.
It works against today's single file (read it, inject, stub `fetch`) and keeps working unchanged
against [C-238](C-238-the-host-mounts-the-explorer-components.md)'s bundle.

Put it at `crates/connectors-api/ui/test/` so that directory exists from day one and C-238 only adds
a bundler beside it. **Not** under `web/` — the site's single-dependency property is deliberate, and
a harness for a credential-collecting page does not belong in the tree that is forbidden to collect
one.

## Three further guards the same harness makes cheap

Each pins a property currently held only by a comment:

- **No `innerHTML`, no `v-html`**, in the console or in any component it imports. Neither surface has
  this guard today; `SchemaBlock.vue:13` only promises it in prose.
- **Auth state changes are POSTs**, never an href — `/auth/signout` and `/auth/dev`. This is the
  `SameSite=Lax` property, currently a comment at `index.html:341`.
- **The wiring vocabulary cannot drift** — and this one is a **Rust** test in `connectors-api`, not a
  JS one: for each `Wiring` variant, serialise it and assert the token appears in the console source.
  Offline, no Node, and it is exactly the C-206 token identity the design calls non-negotiable.

## Notes

- Wire it into CI in this story. [C-240](C-240-the-web-test-suite-runs-in-no-ci-workflow.md) records
  what happens otherwise: the site's 32-test suite runs in no workflow at all.
- **Do not let this become a browser-driver dependency.** `web/` tests 32 assertions against built
  HTML with zero dependencies; the same approach covers everything named above. A headless browser
  is a different decision with its own cost and belongs in its own story if it is ever wanted.
- Related open gap, deliberately not folded in: no real-browser verification exists for **any** of
  this host's cookie behaviour — `SameSite=Lax` delivery on the provider's cross-site redirect, and
  Safari's handling of `Secure` on `http://localhost`, are both unconfirmed because every probe so
  far has been curl or reqwest. That needs a browser and a real Google registration, and it is not
  this story.
