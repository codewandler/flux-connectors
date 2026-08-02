---
id: C-477
title: "Align runtime API versions with vendored provider contracts"
pillar: Connector
status: ready
priority: 40
areas: [providers]
note: "compatibility follow-up — GitHub can send its dated API version; Stripe needs an explicit account-version decision"
---

# Align runtime API versions with vendored provider contracts

## Goal

Make the runtime version selected on the wire agree deliberately with the API version whose schemas
the connector vendors, without quietly moving established operation bytes in a coverage story.

## Acceptance

- [ ] GitHub's `X-GitHub-Api-Version` compatibility header is evaluated against all established
      operation renderings and added with an explicit migration note if accepted.
- [ ] Stripe records whether to send a connector-pinned `Stripe-Version` or remain account-pinned;
      the response-schema consequences and upgrade policy are tested either way.
- [ ] Rehearsals assert the decided version headers and no caller can override them.
- [ ] Any established Flux movement is called out as an intentional compatibility change rather than
      hidden inside a source-coverage release.
