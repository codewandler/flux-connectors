---
id: C-80
title: Let a connector declare more than one error shape
pillar: Spec
status: ready
priority: 3
design:
epic: connectors-v1
areas: [connector-spec, connector-flux]
note: five providers declared pointers that resolve to nothing against the vendor's other shape
---

# Let a connector declare more than one error shape

## Goal
Make `ErrorEnvelope` able to describe what vendors actually return, because five of the sixteen
shipped connectors had to declare a pointer that resolves to nothing against one of their vendor's
two error bodies.

## Acceptance
- [ ] `ErrorEnvelope` admits **alternatives**, not one `message_pointer` plus an optional
      `code_pointer`. Each alternative is selectable — by status class, by a discriminating pointer,
      or by first-match — and the rule is recorded.
- [ ] **`message_pointer` stops being mandatory, or admits a machine code.** Slack publishes only a
      code at `/error`; Airtable answers some 4xx with the bare string `{"error": "NOT_FOUND"}`
      against which a `/error/message` pointer resolves to nothing.
- [ ] **A success predicate lands** (this subsumes the first half of C-57): Slack returns HTTP 200
      with `{"ok": false, "error": …}`, so the status code is not the failure signal. Also stop
      `connector-flux`'s `description()` appending "A non-2xx response is returned as data…" to an
      operation whose failure is not signalled by status — it is true in general and points a model at
      the wrong signal there.
- [ ] **An array of errors is expressible.** Asana returns `{"errors":[{"message","help"}]}` and
      Google returns both a canonical `/error/status` and an `errors[]` array whose `reason` is the
      machine-readable cause; today only the first element is addressable.
- [ ] **The envelope is declarable once per connector or service**, not restated per operation. Asana
      restates it five times, Google eight — and a ninth operation added without it is silently wrong,
      caught only by that connector's own test.
- [ ] **The five affected providers are updated** and their per-file gap notes retired: slack,
      asana, sentry (`/detail` versus a field-keyed validation object), airtable, google.
- [ ] A published `status.rs` issue code exists for "the status code is not the failure signal", so
      the catalogue states it rather than leaving it to prose. `status.rs` derives every issue from a
      rule over the IR and refuses a hand-maintained list, so this needs a rule.

## Progress
- Not started. Filed 2026-07-30 at the close of the sixteen-provider fleet, where this was the
  single most-repeated gap.

## Notes
- Supersedes the first half of [C-57](C-57-quirks-beyond-http-shape.md); that story keeps the
  body-carried cursor. Decide whether to close C-57 into this one or split it cleanly.
- The evidence is five independent discoveries, not one theory: each connector's author hit it while
  authoring, recorded it, and declined to work around it.
