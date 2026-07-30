---
title: Connector & operation explorer

# The explorer is the one page that is a grid and a table rather than paragraphs, and it is the one
# page that leaves the prose column. That column is 688px wide, which is two provider cards, and the
# width is not a variable a page can raise: the theme sets it in a rule keyed on the outline aside
# (`.VPDoc.has-aside .content-container`). Dropping the aside removes the cap at its cause and keeps
# everything else the doc layout gives a Markdown page — typography, padding, the footer.
#
# What the outline is not missed for: it listed exactly two entries. Both headings are still rendered
# and still carry `#providers` and `#operations`, which are linked from elsewhere.
#
# `layout: page` is the shorter diff and the wrong one — it drops the `vp-doc` class, so this page's
# heading, paragraph and warning callout would lose the site's prose styling and have to be rebuilt
# in CSS. Prose pages keep the aside deliberately; widening them would make them harder to read.
aside: false
---

<script setup>
import { data as catalog } from './data/catalog.data.mts'
</script>

# Connector & operation explorer

Browse the available connectors, filter operations by connector, service, risk, and idempotency, sort
the list, then open any operation to inspect its parameters, request path, credentials, hosts, safety
metadata, and Flux source. Every operation has a stable page you can share directly, and a filtered
list is in the address bar — copy the URL and it opens on the same view.

> [!WARNING]
> **Live API calls are not available yet.** Shared availability constraints are stated once below;
> an operation is highlighted only when it also has a limitation of its own.

<CatalogExplorer :catalog="catalog" />
