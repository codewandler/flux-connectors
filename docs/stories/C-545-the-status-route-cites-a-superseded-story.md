---
id: C-545
title: "The status route serves a machine-readable pointer to a superseded story"
pillar: Host
status: done
priority: 3
epic: catalog-artifact
areas: [connectors-api]
note: "status.rs serves Issue { story: \"C-10\" } behind CREDENTIALS_REACH_THE_REQUEST = false; C-535 closed C-10 as superseded, so a machine-readable consumer is pointed at work that is never coming — found by C-542's implementor"
---

# The status route serves a machine-readable pointer to a superseded story

## Goal

Stop the host's status surface from directing consumers to C-10. `crates/connector-cli/src/status.rs`
serves an `Issue` whose `story` field is `"C-10"` (measured at `status.rs:290` on 2026-08-12, behind
`CREDENTIALS_REACH_THE_REQUEST = false` at `:86`); C-535 closed C-10 as superseded-never-implemented,
so the pointer sends a machine-readable consumer at work that is never coming. Unlike C-542's comment
header this is structured data a program may branch on, which is why it is its own story rather than
a rider.

## Acceptance

- [x] The served `Issue` names the honest successor — C-534's program (or the specific child story
      that delivers credentialed requests) — and no route serves `"C-10"` anywhere. *(C-534 the
      program, deliberately: no child delivers credentialed requests — C-538 leaves auth behaviour
      untouched, and the host path already has them via C-114/C-115/C-116; what clears the issue
      is the program replacing the credential-free emitted module. Matches C-542's header
      verbatim. A sweeping test refuses any issue naming a superseded story.)*
- [x] A failing-first test pins the new pointer; the old `"C-10"` value is the seeded failure.
      *(`status::tests::no_issue_names_a_superseded_story`, proven failing at base
      `108fd7a4` with `left: ["C-10"] / right: ["C-534"]`.)*
- [x] The flag's own comment and any related prose in the module agree with the new pointer, and
      the stale C-10 references C-542's implementor catalogued in source comments
      (`seam.rs:25-27`, `:442`, `:489`, `catalog.rs:426`) are reconciled in the same change — they
      are comments, so they ride here rather than earning a third story.
- [x] Full gate green; no artifact bytes change (the status surface is served, not emitted).
      *(Full build wrote nothing; `diff` clean at 1167; `Issue::story` is
      `#[serde(skip_serializing)]`, already pinned by site.rs's
      no-internal-references test.)*

## Progress

- 2026-08-12: Filed at C-542's integration from its implementor's adjacent findings.
- 2026-08-12: Implemented on `impl/C-545` (`577b90ee`), merged `b1dccb65`. Two deliberate
  restraints recorded by the implementor: `Issue.summary` is left as written because it IS
  serialized into `web/public/catalog.json` (826 occurrences) and rewording it is a
  whole-catalogue regeneration with its own story; `CREDENTIALS_REACH_THE_REQUEST` keeps its
  value — flipping it is behaviour. The ~45 remaining stale C-10 comments across five crates are
  filed as C-548.

## Notes

- Write set: `crates/connector-cli/src/status.rs`, `crates/connector-cli/src/seam.rs` (comments
  only), `crates/connector-cli/src/catalog.rs` (comment only), plus tests. Collides with any
  connector-cli story; do not share a wave with C-538, C-543 or C-544.
