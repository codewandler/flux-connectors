---
id: C-502
title: "Migrate SQL, Vault and 1Password into connectors"
pillar: Connector
status: backlog
design: docs/designs/all-integrations-are-connectors.md
epic: all-integrations-connectors
areas: [providers, runtime, credentials, data]
note: "database handles and secret-store operations need lifecycle and credential-result boundaries, not an exemption from the connector model"
---

# Migrate SQL, Vault and 1Password into connectors

## Goal

Represent SQL, Vault and 1Password as connectors while preserving database connection lifecycle and
the stronger rule that secret values never become ordinary model-visible operation results.

## Acceptance

- [ ] SQL covers the existing PostgreSQL/MySQL operation surface with host-resolved endpoints and
      credentials, scoped acquire/use/release and bounded cancellation; C-47's `db.open` decision is
      resolved rather than bypassed.
- [ ] Vault and 1Password declare secret-store capability contracts and return handles/references,
      never plaintext secret values, across the Exchange boundary.
- [ ] The design distinguishes a connector used *as a secret-store backend* from an ordinary
      operation that would disclose a secret and proves both remain fail closed.
- [ ] Runtime artifacts and connector declarations pass parity against the three native plugins.
- [ ] **No plugin whose manifest declares datasources is deleted until every declaration is mapped**
      — Decision 0006 rule 11, as a checkable gate rather than prose: this story carries a
      per-plugin mapping table listing each declared datasource by name, and each row resolves to a
      published `[[datasources]]` connector member proven through C-505's conformance harness or to
      an explicit reviewed removal recorded here. A wave with an unresolved row does not dispatch.
- [ ] Flux can remove the integration crates without removing its generic credential and datasource
      ports — rule 11's second half: the generic ports stay when the integration crates go.

## Progress

- (not started)

## Notes

- This is migration wave 4 after C-501. Flux keeps only its core credential/datasource abstractions;
  it does not execute these external integrations or receive their vendor credentials.
- **Amended 2026-08-04 by C-510:** flux-roadmap Decision 0006 rule 11 made the mapped-replacement
  requirement a program rule; the vendor datasource surface is the `vendor-datasources` epic
  ([C-511](C-511-vendor-datasources-epic.md),
  [vendor-datasource-declarations.md](../designs/vendor-datasource-declarations.md)) and precedes
  this wave for every datasource-declaring plugin.
