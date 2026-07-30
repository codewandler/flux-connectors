---
layout: home

hero:
  name: flux-connectors
  text: Operations and core contracts for Flux
  tagline: Browse SaaS connectors, built-in transformations, language nodes, and versioned network specifications.
  image:
    src: /brand/mark.svg
    alt: ''
  actions:
    - theme: brand
      text: Browse the catalogue
      link: /explorer
    - theme: alt
      text: Current availability
      link: '#availability'

features:
  - title: One explorer, distinct layers
    details: Explore Flux core operations and nodes separately from generated SaaS connector operations.
  - title: Know what a call needs
    details: Every operation page shows its typed parameters, request path, credentials, destination hosts, and exact Flux source.
  - title: Limits are part of the contract
    details: Shared availability constraints and operation-specific issues are shown alongside the capability they affect.
---

<script setup>
import { data as catalog } from './data/catalog.data.mts'
</script>

## The Flux catalogue

flux-connectors publishes a growing catalogue for [Flux](https://github.com/codewandler/flux). It
combines generated SaaS operations with Flux-owned core contracts while keeping their ownership and
execution models explicit.

<CatalogSnapshot :catalog="catalog" />

## What you can evaluate today

The catalogue is useful before live connector execution is enabled. You can inspect:

- built-in transformations and their complete tool schemas;
- language nodes and their anchored Flux AST schemas;
- available and planned network capabilities, including whether each is callable;
- a stable connector operation name and plain-language description;
- HTTP method, request path, typed parameters, and published schemas;
- risk and idempotency metadata for approval and retry decisions;
- required credentials and destination hosts;
- the exact Flux operation source; and
- shared constraints plus any limitation specific to that operation.

Open the [connector and Flux core explorer](/explorer) to compare the current surface or deep-link
directly to an entry. Canonical JSON specifications are published under
`https://flux.codewandler.org/v1/`.

## Availability {#availability}

> [!CAUTION]
> **The catalogue is preview-only. No connector can make a live API call yet.** Secure credential
> application and tenant configuration still need host support. Do not treat the current modules as
> production-ready integrations.

Some operations also have narrower limitations, including query values that cannot yet be encoded
safely. Freshdesk currently has no credential configuration because publishing the apparent one
would put a secret outside Flux's protection. These conditions are shown on the affected connector
or operation page, where they matter.

The project fails closed: a capability is marked unavailable rather than presented as usable with
an unsafe or incomplete request.

## Follow the project

The source, release history, local build instructions, and contribution workflow live in the
[GitHub repository](https://github.com/codewandler/flux-connectors). The public site stays focused
on the connector catalogue and its user-facing contract.
