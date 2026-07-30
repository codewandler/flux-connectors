---
id: C-109
title: Ship the Twilio connector
pillar: Spec
status: ready
priority: 5
design:
epic: provider-fleet-2
areas: [providers]
note: "third basic-join vendor, and the one whose username half is an account identifier rather than an email — so it tests whether the config model generalises past the zendesk/jira shape"
---

# Ship the Twilio connector

## Goal
Messaging, and a third variation on the basic-auth archetype.

## Acceptance
- [ ] A curated operation set: send a message, list messages, fetch a message, list calls.
- [ ] **Auth is basic-join with an account SID as the username half** — not an email, which is what
      both shipped basic-join connectors use. The `[[config]]` labels and help text must read
      correctly for an identifier a user copies from a console rather than one they already know.
- [ ] The account SID also appears **in the path** of most endpoints, so the same value is both a
      credential half and a path parameter. The file records how that is expressed without asking a
      user for it twice.
- [ ] `[[events]]` and a `webhook` binding for status callbacks, with Twilio's published signature
      scheme — a fourth row for C-60's matrix.
- [ ] A `[[config]]` surface, a `verify` operation, and a per-provider contract test.

## Progress
- Not started.

## Notes
- One value serving as both a credential component and a path parameter is genuinely new. If the
  model cannot express it without duplication, that is a finding for the configuration design, not a
  reason to hand-wave the connector.
