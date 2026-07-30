---
id: C-108
title: Ship the Microsoft Graph connector
pillar: Spec
status: ready
priority: 5
design:
epic: provider-fleet-2
areas: [providers]
note: "the second multi-service provider after Google — and the first where the services share a host, so it tests whether a service is a real level or just Google's URL problem"
---

# Ship the Microsoft Graph connector

## Goal
Mail, calendar and files behind one vendor — and a second, differently-shaped test of the service
level.

## Acceptance
- [ ] Services for the surfaces shipped — `mail`, `calendar`, `files` — each with its own curated
      operations, under one authority.
- [ ] **The services share `graph.microsoft.com`.** Google's three justified themselves partly by
      differing hosts (`gmail.googleapis.com` versus `www.googleapis.com`); these do not. So this
      connector answers a real question: is a service a genuine addressing level, or was it Google's
      host problem wearing a hat? Whichever the answer, the file records it — and if the honest
      answer is "one service", that is a finding about C-49 and not a failure here.
- [ ] Auth: OAuth2 bearer. Coordinate with [C-88](C-88-prove-oauth2.md) rather than duplicating it —
      if C-88 has landed, this connector uses the proven shape; if not, this is a candidate to be the
      provider that proves it.
- [ ] A `[[config]]` surface, a `verify` operation, and a per-provider contract test.

## Progress
- Not started.

## Notes
- Microsoft versions by path prefix (`/v1.0`, `/beta`), which interacts with how `base_url` and
  `api_version` divide — Shopify already hit the "API version lives in the path" shape and its file
  records the reasoning. Read it before deciding.
