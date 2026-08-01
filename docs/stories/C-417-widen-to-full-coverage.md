---
id: C-417
title: "Widen babelforce to manager-sdk's full coverage, and gate it"
pillar: Spec
status: backlog
priority: 3
design: docs/designs/spec-front-end.md
epic: spec-front-end
areas: [providers, connector-cli]
note: "397 operations against a catalogue that holds 299 across 53 providers today — so this more than doubles it, and every one of the four bulk declarations has to be carrying its weight for the file to stay reviewable"
---

# Widen babelforce to manager-sdk's full coverage, and gate it

## Goal
Take `providers/babelforce.toml` from nine operations to the 397 `manager-sdk/COVERAGE.md` calls
canonical, and put a check under it so coverage cannot regress unnoticed.

## Acceptance
- [ ] **Scope is exactly what manager-sdk covers — owner-stated 2026-08-01: "only include things
      which are also included in manager-sdk itself".** That is `manager-sdk/COVERAGE.md`'s canonical
      **397**, which is the 398 operations the five documents declare minus
      `POST /api/v1/webhook/zendesk` (operationId `zendesk`) — a webhook *receiver* babelforce exposes
      for Zendesk to call, not a client-callable operation. Nothing beyond that set is added here.
- [ ] **No `/api/internal` surface, ever.** Measured 2026-08-01: `internal` appears in **zero** paths
      across all five documents, so there is nothing to exclude today — which is exactly why this is a
      **guard** rather than a filter. A check refuses any selected operation whose path carries an
      `internal` segment, so a future spec pull cannot introduce one silently. Owner-stated, and the
      cost of the guard is one assertion.
- [ ] All five services are selected and reach the IR; the emitted operation count matches the
      canonical scope, with every intentional exclusion named in the file beside its reason.
- [ ] A coverage test in this repo mirrors `manager-sdk/COVERAGE.md`: it reads the vendored documents,
      counts operations, compares against what the connector emits, and **fails on a gap that is not
      explicitly allowed**. An allow-list entry requires a reason string.
- [ ] The curated set stays exposed and the rest are callable-but-unexposed (C-413), so the tool
      catalogue does not grow by 388 entries. A test asserts the exposed count.
- [ ] `providers/babelforce.toml` stays reviewable — if it has not stayed well under the ~6,000 lines
      hand-authoring would have cost, say so in Progress with the number, because that is the epic's
      thesis failing in the open rather than quietly.
- [ ] The explorer and `catalog.json` still render at this size. If they do not, file the finding
      against the C-99 explorer epic rather than trimming coverage to fit the UI.
- [ ] 23 operations carry no `summary`/`description` in the documents. Each either gets one through
      the overlay or stays unexposed — a tool contract with no sentence in it does not ship.

## Progress
- (not started)

## Notes
- Blocked behind C-416: do not widen until nine known-correct operations reproduce.
- Multipart is inexpressible (`BodyEncoding` is `Json | Form`), so five file-upload operations cannot
  be emitted at all. They are allow-list entries here and a named gap in C-418, not a silent absence.
- Expect this to surface real defects in the documents — 4 of the manager document's 356 operations
  publish no 2xx schema, and `task-schedule` publishes none at all. That is what the overlay is for.
