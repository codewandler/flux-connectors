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
      never plaintext secret values, across local and Exchange placements.
- [ ] The design distinguishes a connector used *as a secret-store backend* from an ordinary
      operation that would disclose a secret and proves both remain fail closed.
- [ ] Runtime artifacts and connector declarations pass parity against the three native plugins.
- [ ] Flux can remove the integration crates without removing its generic credential and datasource
      ports.

## Progress

- (not started)
