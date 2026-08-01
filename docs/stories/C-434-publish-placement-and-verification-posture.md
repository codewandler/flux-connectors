---
id: C-434
title: "Publish where a credential is placed and whether inbound events are verified"
pillar: Build
status: ready
priority: 3
design: docs/designs/connector-security-posture.md
epic: connector-security-posture
areas: [connector-cli, providers]
note: "both facts are already declared and neither is published as a posture. trello puts its credential in a QUERY STRING — proxy logs, browser history — and nothing says so. Only 2 of 54 providers declare an inbound channel at all, which is a third state, not a bad score"
---

# Publish where a credential is placed and whether inbound events are verified

## Goal
Publish the two posture axes this repository can already answer, so a consumer can act on them
without diffing auth schemes by hand.

## What is already true and unpublished

Measured across the catalogue, 54 providers:

**Placement.** 31 bearer, 15 header, 3 basic, 2 bearer+signing, **1 query**, 1 basic+signing, 1 none.
That single `query` is `trello`, and it is a real posture difference rather than a stylistic one: a
credential in a URL is written to proxy logs, browser history and error trackers in a way a header is
not. One instance is enough to make the fact worth publishing, because a reader who does not already
know will not go looking.

**Inbound verification.** Only **2 of 54** providers declare an inbound channel at all — `slack` and
`twilio` — and 3 HMAC declarations exist across them. Since C-188, Twilio's is checked against the
vendor's own published signature.

## Acceptance
- [ ] The manifest and `catalog.json` publish, per connector: **where the credential is placed**, and
      **the inbound verification state**.
- [ ] **Inbound verification is three states, not two**: verified · declared-but-unverified ·
      **no inbound surface declared**. The third is not a bad score and must not render as one —
      exactly the distinction C-235 landed for credentials and C-408 landed for the explorer.
- [ ] **A query-string placement is published as the distinct fact it is**, with the reason attached
      rather than left to a reader to infer from the word `query`.
- [ ] Derived from existing declarations only. This story adds **no new provider-file key** — if it
      needs one, that is a finding to report rather than a field to invent.
- [ ] The published wording says what it is: *these are the connector's declarations*, not *this
      connector is safe*. The design's central risk is that a posture is published and believed.

## Progress
- (not started)

## Notes
- Sequenced after [C-433](C-433-credential-lifetime-and-rotation.md) only if a shared shape emerges;
  otherwise independent — these two axes are derivable today and rotation is not.
- The `signing` scheme appears twice (`bearer+signing`, `basic+signing`) and is a *stronger* posture
  than a bare credential, since a signature does not travel as a reusable secret. Do not flatten it
  into "has two schemes".
- `trello` is the only instance and its provider file should carry the reason, so the published fact
  and the file agree — the pattern C-430 used for its withheld operations.
