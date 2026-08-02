---
id: C-480
title: Provision webhook subscriptions and generated signing secrets
pillar: Surfaces
status: ready
priority: 4
design: docs/designs/zendesk-suite.md
epic: zendesk-suite
areas: [connector-spec, connector-pack, connectors-api, secrets]
note: "subscription setup cannot yet bind selected events, required create fields, or capture a post-create signing secret"
---

# Provision webhook subscriptions and generated signing secrets

## Goal

Represent webhook setup workflows that turn selected events and a host callback into a complete
create request, then acquire and store a generated signing secret without exposing it to a model.

## Acceptance

- [ ] Subscription declarations can bind selected event wire values and required constant/setup fields
      in addition to the callback URL.
- [ ] A setup workflow can acquire a credential returned after creation and write it directly through
      the secret-store boundary without returning the value as ordinary operation output.
- [ ] Dry-run and audit output redact credential values while showing the setup steps and destinations.
- [ ] Zendesk webhook provisioning is exercised end to end from selection through verified delivery,
      including rollback or an explicit recoverable partial-setup state.
- [ ] Existing subscription declarations retain their generated and runtime behavior.

## Progress

- 2026-08-02: filed from C-465's fail-closed webhook implementation review.
