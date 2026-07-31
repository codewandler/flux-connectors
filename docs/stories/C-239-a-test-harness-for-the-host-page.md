---
id: C-239
title: "A behavioural change to the host's page cannot ship with a failing-first test, because nothing can test it"
pillar: Build
status: ready
priority: 2
design: docs/designs/host-explorer.md
epic: host-explorer
areas: [host, build]
note: "recorded debt, not a new finding: C-234 could not close its mutation M15 for this reason and said so in its Acceptance. AGENTS.md requires a failing-first test for a behavioural change, and index.html is the one surface where that is impossible"
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

## Notes

- Sequencing: this is cheapest **with** [C-238](C-238-the-host-mounts-the-explorer-components.md),
  which brings a build step anyway — but it is not blocked by it. A harness over the single file as
  it stands is worth having on its own, and would let
  [C-237](C-237-the-host-explorer-is-a-console.md) ship with a real failing-first test rather than a
  described one.
- **Do not let this become a browser-driver dependency.** `web/` tests 32 assertions against built
  HTML with zero dependencies; the same approach covers everything named above. A headless browser
  is a different decision with its own cost and belongs in its own story if it is ever wanted.
- Related open gap, deliberately not folded in: no real-browser verification exists for **any** of
  this host's cookie behaviour — `SameSite=Lax` delivery on the provider's cross-site redirect, and
  Safari's handling of `Secure` on `http://localhost`, are both unconfirmed because every probe so
  far has been curl or reqwest. That needs a browser and a real Google registration, and it is not
  this story.
