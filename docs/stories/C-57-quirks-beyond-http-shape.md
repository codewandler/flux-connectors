---
id: C-57
title: Let the quirk model describe a success predicate and a body-carried cursor
pillar: Spec
status: ready
priority: 7
design:
epic: connectors-v1
areas: [connector-spec, connector-flux]
note: Slack answers `{"ok": false}` with HTTP 200; its cursor is a body field
---

# Let the quirk model describe a success predicate and a body-carried cursor

## Goal
Stop assuming the HTTP status is the failure signal and the query string is where a cursor lives —
two assumptions Slack breaks, and it will not be the last vendor to.

## Acceptance
- [ ] **`ErrorEnvelope` gains a success predicate** (`ok_pointer` / `success_when`), so "the failure is
      in the body of a 200" is declarable. `ErrorEnvelope`'s own doc comment currently scopes it to
      *"where a vendor hides the real error inside a **non-2xx** response body"* — that scope is the
      bug.
- [ ] `connector-flux`'s `description()` (`crates/connector-flux/src/op.rs:388-408`) stops appending
      *"A non-2xx response is returned as data, not a failure…"* to an operation whose failure is not
      signalled by status. Today that sentence is true in general and points a model at the wrong
      signal for Slack, which is why C-53 had to restate the contract in every operation's prose.
- [ ] **`message_pointer` stops being mandatory**, or admits a machine code: Slack publishes only a
      code at `/error` and no human-readable message, so the required field currently has to carry a
      code and misdescribe it.
- [ ] **`Pagination::Cursor` admits a body-carried cursor.** `cursor_param` is defined as *"The query
      parameter carrying the cursor"*, so cursor paging is unexpressible for any POST+JSON API —
      independent of C-30, which C-53's notes originally assumed was the blocker.
- [ ] `providers/slack.toml` declares its real envelope and retires the prose workaround;
      `slack-conversations-history` can page. Failing-first test named for each half.
- [ ] A published `status.rs` issue code exists for "the status code is not the failure signal", so the
      catalogue can say it rather than hiding it in a description. `crates/connector-cli/src/status.rs`
      derives every issue from a rule over the IR and refuses a hand-maintained list, so this needs a
      rule, not an entry.

## Progress
- Not started. Filed 2026-07-30 from C-53, whose story text records the gap in full.

## Notes
- The consumer risk is concrete and current: anything switching on HTTP status sees success on every
  Slack failure, and no machine-readable field warns it — only prose an LLM may or may not honour.
- Related but separate: `check_write_metadata` derives write-ness from the HTTP verb, so a POST read
  cannot declare `risk = "low"`/`idempotent`. That collapsed read/write risk on Slack. It is a
  candidate for this story or its own; decide and record which.
