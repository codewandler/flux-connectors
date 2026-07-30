---
layout: home

hero:
  name: flux-connectors
  text: SaaS operations for Flux
  tagline: Browse typed operations, understand what each call needs, and see safety and availability information before you use it.
  image:
    src: /brand/mark.svg
    alt: flux-connectors
  actions:
    - theme: brand
      text: Browse connectors
      link: /explorer
    - theme: alt
      text: Current availability
      link: '#availability'

features:
  - title: Find the right operation
    details: Explore connectors by service, then filter operations by risk, idempotency, method, and known limitations.
  - title: Know what a call needs
    details: Every operation page shows its typed parameters, request path, credentials, destination hosts, and exact Flux source.
  - title: Limits are part of the contract
    details: Shared availability constraints and operation-specific issues are shown alongside the capability they affect.
---

<script setup>
import { data as catalog } from './data/catalog.data.mts'
</script>

## The connector catalogue

flux-connectors is a growing catalogue of SaaS operations designed for
[Flux](https://github.com/codewandler/flux). It gives people and agents a consistent way to discover
what a service can do, what inputs an operation accepts, how risky it is, whether it is safe to
retry, which credentials it requires, and where the request goes.

<CatalogSnapshot :catalog="catalog" />

## What you can evaluate today

The catalogue is useful before live execution is enabled. For every operation you can inspect:

- a stable operation name and plain-language description;
- HTTP method, request path, typed parameters, and published schemas;
- risk and idempotency metadata for approval and retry decisions;
- required credentials and destination hosts;
- the exact Flux operation source; and
- shared constraints plus any limitation specific to that operation.

Open the [connector and operation explorer](/explorer) to compare the current surface or deep-link
directly to one operation.

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
