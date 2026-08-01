---
id: C-425
title: "Check the vendor's own examples against the vendor's own schemas"
pillar: Build
status: backlog
priority: 3
design: docs/designs/generated-connector-tests.md
epic: generated-connector-tests
areas: [connector-spec]
note: "the one genuinely generatable test in the epic, because it is NOT derived from one thing twice — an example and a schema are two independent statements the vendor made, and nothing here has ever checked they agree. babelforce alone supplies 352 cases"
---

# Check the vendor's own examples against the vendor's own schemas

## Goal
Generate a per-operation check that the `example` a vendor publishes satisfies the `response_schema`
the same document declares — a test oracle this repository has never had, and the only assertion in
this epic that is not a restatement of something already known.

## Acceptance
- [ ] For every operation whose vendored document publishes both a 2xx `example` and a resolved
      response schema, a check validates the first against the second.
- [ ] **The check is not tautological, and the story says why in one sentence**: the example and the
      schema are two independent statements by the vendor, so disagreement is real information —
      unlike an assertion derived from the IR that produced the artifact.
- [ ] A failure is a **diagnostic naming the operation**, not a build failure. It is the vendor's
      defect, not ours, and C-4 already established that grade of failure for a bad endpoint. A
      connector must still compile against a document whose examples are wrong.
- [ ] The count is reported: how many operations carry both, how many agree, how many do not.
      babelforce's manager document alone publishes a 2xx schema for **352 of 356** operations.
- [ ] It catches a wrong `$ref` resolution, not only a vendor's typo — a schema resolved to the wrong
      definition will usually reject the example that was written for the right one. Prove that with a
      deliberately mis-resolved fixture.

## Progress
- (not started)

## Notes
- **This is additive, not boilerplate reduction.** It belongs to this epic because it is the honest
  answer to "what could generated per-connector tests actually buy" — the answer is not fewer lines,
  it is a check nothing performs today.
- Independent of C-423 and C-424: those are about removing duplication, this is about adding coverage.
  It can run whenever, and does not wait on the classification.
- The examples are already in the repository — C-415 vendored five babelforce documents and
  deliberately kept `example` blocks after scrubbing only credential and identity values, so the
  oracle's input is committed and hermetic.
- Watch for the reverse reading: a *request* example checked against a request schema is the same
  idea and probably free once the response side works, but 128 operations declare a body against 363
  that declare a response, so the response side is where the evidence is.
