---
id: C-433
title: "A credential cannot say how long it lives or whether it can be rotated"
pillar: Spec
status: ready
priority: 2
design: docs/designs/connector-security-posture.md
epic: connector-security-posture
areas: [connector-spec]
note: "the axis the owner's example turns on, and the one the catalogue does not carry at all. `Acquisition` has two variants and its own docs say `Minted` read as placement IS `Static` — so a 30-day rotating token and a permanent one are indistinguishable across all 54 providers"
---

# A credential cannot say how long it lives or whether it can be rotated

## Goal
Let a connector state its credential's lifetime and rotation story, so "a static token nobody can
rotate" and "a short-lived token with a refresh path" stop being the same declaration.

## The gap, measured

Owner-stated 2026-08-01: *"Twilio's HMAC is quite safe compared to something using static tokens
which cannot easily be changed or rotated."* The comparison is sound and **the catalogue cannot make
it**. Measured across 54 providers on the day this was filed:

- `Acquisition` has exactly two variants, `Static` and `Minted`, and its own documentation says
  `Minted` *"read as a placement instruction is `Static`"* — the distinction it carries is about
  **which call mints**, not about **how long the value lasts**.
- 31 providers declare `bearer`. That one word covers a 30-day rotating token and a permanent one.
- `AGENTS.md`'s ownership table gives `connector-secrets` *"no expiry, refresh, rotation or
  revocation"*. Nobody owns the question from either side.

So a consumer asking the owner's question — *can this credential be rotated?* — gets no answer for
any of the 54, and cannot tell that no answer was given.

## Acceptance
- [ ] A credential can state its **lifetime** (long-lived static secret / short-lived with a refresh
      path / minted per session) and whether the vendor supports **rotation without downtime** and
      **revocation**.
- [ ] **Each is a stated claim, never inferred.** Deriving rotation from `AuthScheme` is exactly the
      plausible-and-wrong the `Risk` type already refuses to make from an HTTP method — `bearer` says
      nothing about longevity. A test asserts no inference path exists.
- [ ] **Silence is distinguishable from a poor answer**, and reads as neither good nor bad. C-235,
      C-408 and C-430 each hit this trap independently; the fourth time is not bad luck.
- [ ] The claim is **checkable in the sense C-186 established**: the machine checks a statement exists
      and says something; only review checks it is true. Follow `repeatable_because`'s precedent
      rather than inventing a second convention.
- [ ] At least three shipped providers adopt it and are visibly different from one another — a
      long-lived API key, an OAuth2 refresh flow, and one that genuinely cannot be rotated. A field
      every connector fills identically has measured nothing.
- [ ] It reaches the manifest and `catalog.json`, or a consumer cannot ask the question the story
      exists to answer.

## Progress
- (not started)

## Notes
- **This gates the grade.** The design records that a composed rating must not ship before this
  exists, because the first grade would then be computed over every axis except the one the request
  was about.
- Watch the boundary with `connector-secrets`: it resolves an address to a value and explicitly does
  not do expiry or refresh. This story declares a **property of the credential**, not a mechanism —
  it must not smuggle a refresh implementation into the compiler.
- `OAuth2Spec` exists and C-88 records that **no shipped provider uses it**, so the refresh-path case
  has no live instance yet. Say so rather than implying coverage.
