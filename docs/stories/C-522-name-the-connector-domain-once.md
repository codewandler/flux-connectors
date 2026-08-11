---
id: C-522
title: "Name the connector domain once across the Flux family"
pillar: Surfaces
status: done
design: docs/designs/connector-surfaces.md
areas: [docs, catalogue]
note: "Connector, Service, Operation, Event Type and Channel Binding need one definition; Provider remains only the published compatibility type"
---

# Name the connector domain once across the Flux family

## Goal

Define the connector-owned terms a host consumes and distinguish them from Exchange installations,
Flux model providers, identity providers and tenant datasources.

## Acceptance

- [x] `docs/concepts.md` defines Connector, Service, Operation, Event Type and Channel Binding and
      links each to the artifact that publishes it.
- [x] The page states that the Rust catalogue's `Provider` name is compatibility vocabulary, while
      family prose says Connector and qualifies other provider meanings.
- [x] Datasource and runtime gaps are explicit: the published closure carries no vendor-data
      datasource definition or channel runner, and a Channel Binding never installs itself.
- [x] The docs index links the vocabulary and the docs-only checks pass.

## Progress

- 2026-08-03: Raised from Flux Exchange X-104's audit of the published 0.16.0 closure. Written on
  `docs/connector-domain-vocabulary` and never merged.
- 2026-08-11: Recovered from that branch, re-filed under C-522 (the original C-493 was already taken
  on `main` by *Move the flux engine line to 0.54*), and re-verified against the tree at v0.20.0
  before landing. Every version-bound claim on the page was re-measured rather than carried over:
  `crates/catalog/src/lib.rs` publishes `Provider`, `Operation`, `Event`, `Channel`, `ConfigField`
  and `ConfigChoices` but **no** standalone `Service` type, and service identity still travels as the
  `service: &'static str` field on each of those values (`grep -n "pub service" crates/catalog/src/lib.rs`
  → 5 hits). No provider declares a `[[graphs]]` member (`grep -n 'graphs' providers/*.toml` → one
  comment in `providers/stripe.toml:27`), and `web/public/catalog.json` carries no `datasources` key.

## Notes

- This story changes no generated connector artifact and no public capability claim.
- **The recovered branch's `docs/vision.md` delta was deliberately dropped.** It rewrote the
  *Non-goals* section to say that Docker, Kubernetes, SQL, Prometheus, Loki, Vault and Asterisk AMI
  "stay Flux plugins" — which [C-495](C-495-all-integrations-are-connectors-epic.md) has since
  reversed. Landing it would have regressed the accepted charter to its pre-C-495 wording. Only
  `docs/concepts.md` and the `docs/README.md` index row were taken.
