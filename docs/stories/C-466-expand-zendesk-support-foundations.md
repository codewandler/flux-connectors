---
id: C-466
title: "Expand Zendesk Support foundations after the audit proof"
pillar: Spec
status: done
design: docs/designs/zendesk-suite.md
epic: zendesk-suite
areas: [providers, connector-flux]
note: "eight remaining inventory-approved Support reads; incomplete and secret-shaped write contracts remain withheld or deferred"
---

# Expand Zendesk Support foundations after the audit proof

## Goal
Grow the proven Ticketing-spec route into the remaining safe Support foundation surface without
sweeping in an upstream family or weakening a vendor contract to make it compile.

## Acceptance
- [x] The exact carried set is remeasured against the pinned document and reconciled with
      `zendesk-suite-inventory.md`; ticket creation remains deferred until required input,
      `Idempotency-Key`, and the documented response can all be represented.
- [x] Exactly eight user, organization, group, view, field, form, status and recent-ticket reads are
      selected one by one; every optional unsafe query input is named in `omit`.
- [x] `CreateOrUpdateUser` is negatively accounted because its nested input exposes `password` and
      admits a merge with no stable identity; no Support write is added by this story.
- [x] Response shapes come from the pinned document or an expressible reviewed correction, with no
      guessed required fields.
- [x] The original seven operations and C-461's audit operation remain byte-stable.
- [x] Scoped build/diff, request rehearsal, and the workspace gate prove the expanded provider.

## Progress

- 2026-08-02: implementation preflight corrected the inventory before selection. The pinned
  `CreateOrUpdateUser` body requires a top-level `user`, but that value is a union whose create and
  merge variants both expose ordinary `password`; the merge variant has no required member. Since
  `omit.body` cannot remove a nested field or strengthen the union, the operation is withheld. The
  corrected C-466 tranche contains eight reads.
- 2026-08-02: selected those eight operationIds individually and omitted all 33 optional query
  parameters. The count is command-derived from the C-466 tranche in `providers/zendesk.toml`:
  `awk '/# C-466 adds/{tranche=1} /# First-party Help Center/{tranche=0} tranche &&
  /^omit\\.query/{omission=1} tranche && omission {line=$0; count+=gsub(/"[^"]+"/, "", line);
  if ($0 ~ /\\]/) omission=0} END {print count}' providers/zendesk.toml` printed `33`. Focused
  accounting keeps `CreateTicket` deferred for its optional principal input, absent retry header,
  and incomplete response; keeps bodyless `CreateOrUpdateOrganization` deferred; and withholds the
  password-bearing, identity-free `CreateOrUpdateUser` merge.
- 2026-08-02: the first workspace rehearsal exposed the source integer-or-string `view_id` union
  lowering to Flux `Any` and composing `{}` in a path. The reviewed correction narrows it to the
  documented built-ins `incoming`, `my`, and `my_groups`; a focused rehearsal also refuses eight
  delimiter/segment escapes. `cargo run -p connector-cli -- build --provider zendesk` planned 44
  artifacts and wrote 11 on the first generation; `cargo run -p connector-cli -- diff --provider
  zendesk` then reported `44 artifacts up to date (1 provider checked)`. The focused spec suites
  pass 17 tests and the focused pack/request suites pass 21. `cargo build --workspace` and `cargo
  fmt --all --check` pass. The first `cargo test --workspace --no-fail-fast` run exposed the view
  issue among nine failing targets; after the correction, all three affected pack targets and both
  historical Zendesk suites pass, while the inventory-count test advances past Zendesk and stops on
  parallel GitHub accounting. A redundant final full rerun was stopped at compilation by the
  concurrent C-481 addition of `Operation::spec_source`; all C-466 targets had already passed.
  `cargo clippy --workspace
  --all-targets -- -D warnings` reaches the parallel OpenAI selection test and stops on its
  `type_complexity` warning, not on C-466 code.

## Notes
- This story follows C-461 and writes the same provider and generated files.
