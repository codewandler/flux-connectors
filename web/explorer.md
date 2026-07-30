---
title: Provider & operation explorer
outline: [2, 2]
---

<script setup>
import { data as catalog } from './data/catalog.data.mts'
</script>

# Connector & operation explorer

Browse the available connectors, filter operations by provider, risk, and idempotency, then open any
operation to inspect its parameters, request path, credentials, hosts, safety metadata, and Flux
source. Every operation has a stable page you can share directly.

> [!WARNING]
> **Live API calls are not available yet.** Shared availability constraints are stated once below;
> an operation is highlighted only when it also has a limitation of its own.

<CatalogExplorer :catalog="catalog" />
