---
id: C-435
title: "Decide whether a composed security grade ships at all"
pillar: Spec
status: backlog
priority: 4
design: docs/designs/connector-security-posture.md
epic: connector-security-posture
areas: [connector-cli]
note: "DECISION, deliberately not assumed. A grade reads as a measurement and is an opinion with arithmetic on top — and this repo already found the defect class: Zoom documented its hazard precisely and returned the field anyway. Describing a property is not having it"
---

# Decide whether a composed security grade ships at all

## Goal
Settle, in the open, whether this repository publishes a single security rating over a connector — and
if so, under what constraints — rather than shipping one because it was asked for.

## The decision

The request that opened this epic was for a rating. The design records why the rating is the most
dangerous artifact the epic could produce, and the argument is not squeamishness:

- **It reads as a measurement and is an opinion with arithmetic on top.** Nobody audits the weighting.
- **It conflates independent axes.** Twilio's HMAC is *inbound*; a static token's rotation story is
  *outbound*. A connector can be excellent at one and hopeless at the other.
- **It is gameable in the wrong direction.** An author who can raise a grade by adding a declaration
  will add the declaration; the grade improves and the connector does not.
- **It inherits the defect this repository keeps finding.** C-430: Zoom and Postmark documented their
  hazard *precisely* — *"HOST-PRIVILEGED… treat it as a credential"* — and returned the field anyway.
  A grade computed from declarations grades the declarations.

The case *for* is also real, and should not be strawmanned: a person choosing between two connectors
wants an answer, not a table, and refusing to give one is its own kind of unhelpfulness.

## Acceptance
- [ ] A decision, recorded with its reasoning, in the design doc — **yes with constraints** or **no
      with what is published instead**. Not deferred, not implied by absence.
- [ ] **If yes**, all four hold: the grade is derived from published facts by a **stated function**;
      that function is **versioned** in the document; a consumer can **recompute** it; and the facts
      are **always published beside it** so a consumer who disagrees can ignore it.
- [ ] **If yes**, it does not ship before [C-433](C-433-credential-lifetime-and-rotation.md) — a first
      grade computed over every axis except rotation would be exactly the wrong artifact, since
      rotation is what the request was about.
- [ ] **If no**, the design says what a consumer should read instead and why that is more useful, so
      the question is closed rather than left to be re-asked.
- [ ] Either way, nothing published claims to describe a connector's *actual* security — only what it
      **declares**. The distinction goes in the artifact's own words.

## Progress
- (not started)

## Notes
- Sequenced last in the epic **by design**: the decision is better made once the facts exist and their
  shape is known than as an opening assumption.
- Precedent for the shape of this story: [C-132](C-132-decide-ivr-templates.md) and
  [C-402](C-402-whole-host-template-allowlist.md) are both DECISION stories that closed by recording a
  reasoned answer rather than by shipping code.
- If the answer is yes, the strongest available model is a **profile** rather than a letter — a named
  policy ("no static credentials", "verified inbound only") that a consumer selects and the catalogue
  answers against. That moves the weighting to the consumer, where it belongs, and is not a grade.
