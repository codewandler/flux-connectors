---
id: C-453
title: "A release preflight skips both Node gates and can tag a red site"
pillar: Build
status: done
areas: [website, release]
note: "measured on v0.12.0: crates.io published green while CI's site job failed; scripts/cut-release.sh runs only the Rust block even though AGENTS.md names the site and host-page suites as required CI gates"
---

# A release preflight skips both Node gates and can tag a red site

## Goal

Make a locally cut tag mean that every required repository gate has passed, including the public
site and the host operator page, so pushing the tag cannot publish crates before CI discovers a red
consumer surface.

The failure is measured on 2026-08-02, not inferred: `npm ci && npm run build && npm test` under
`web/` ran 42 tests and reported 39 pass / 3 fail. One failure says `Provider` omits the generated
`config_choices` field. The other two are the same catalogue/prose collision: the shipped service
name `user` appears as an English word in `OperationDetail.vue`, and the hand-maintained-data guard
reads it as a literal. GitHub's `crates.io` run 30728487439 succeeded for v0.12.0 before CI run
30728486695 reported the site job red.

## Acceptance

- [x] `web/data/catalog.mts` declares the generated `Provider.config_choices` shape, and the
      catalogue type-agreement test is green.
- [x] The hand-maintained catalogue-data guard ignores catalogue words in prose while still catching
      a value rendered into a component; the real `user` collision and its focused mutation test are
      both covered.
- [x] `scripts/cut-release.sh` runs `npm ci`, build, and tests for `web/`, and `npm ci` plus tests for
      `crates/connectors-api/ui`, inside the transactional gate and before it creates a tag.
- [x] A failing Node gate restores the release tree byte-for-byte, commits nothing, and tags nothing;
      the release-script fixture proves the Node invocations rather than grepping the script.
- [x] `WHATS-NEW.md` has one topmost `[Unreleased]` section and release headings the cut script can
      promote in order; the script refuses a displaced section rather than minting another malformed
      customer changelog.
- [x] `cargo run -p connector-cli -- diff`, the full Rust gate, the public-site gate, and the host-page
      gate are green before the patch release is tagged.

## Progress

- Failing-first evidence captured from the committed v0.12.0 tree: the public-site suite reports the
  three failures named above. The release cut itself did not run that suite.
- **Done.** `cargo run -p connector-cli -- diff` reported `951 artifacts up to date (54 providers
  checked)`; the Rust workspace gate passed; `web/` passed 42/42; the host page passed 15/15; and
  `cut_release` passed 11/11 including a deliberately red Node gate and a displaced customer
  changelog.
