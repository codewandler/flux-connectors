---
title: Provider & operation explorer
outline: [2, 2]
---

<script setup>
import { data as catalog } from './data/catalog.data.mts'
</script>

# Provider & operation explorer

Everything below is read from the generated catalogue — the same intermediate representation that
emits the `.flux` modules and the connector manifests, so it cannot drift from what actually ships.
Each operation has its own page and its own URL, quotable from an issue or a chat.

**Nothing here can make a live API call yet.** That is one condition affecting every operation
equally, stated once below rather than stamped on all of them; an operation is flagged only for a
defect it has of its own.

<CatalogExplorer :catalog="catalog" />
