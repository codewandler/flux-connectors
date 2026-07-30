---
title: Connector & core explorer

# The explorer is the one page that is a grid and a table rather than paragraphs, and it is the one
# page that leaves the prose column. That column is 688px wide, which is two provider cards, and the
# width is not a variable a page can raise: the theme sets it in a rule keyed on the outline aside
# (`.VPDoc.has-aside .content-container`). Dropping the aside removes the cap at its cause and keeps
# everything else the doc layout gives a Markdown page — typography, padding, the footer.
#
# What the outline is not missed for: it listed exactly two entries. Both headings are still rendered
# and still carry `#core`, `#providers`, and `#operations`, which are linked from elsewhere.
#
# `layout: page` is the shorter diff and the wrong one — it drops the `vp-doc` class, so this page's
# heading, paragraph and warning callout would lose the site's prose styling and have to be rebuilt
# in CSS. Prose pages keep the aside deliberately; widening them would make them harder to read.
aside: false
---

<script setup>
import { data as catalog } from './data/catalog.data.mts'
</script>

# Connector & Flux core explorer

Browse Flux-owned built-ins alongside the available SaaS connectors. Core operations, language nodes,
and network capabilities each link to a versioned JSON specification. Connector operations remain
filterable by connector, service, risk, and idempotency, and every entry has a stable detail page.

> [!WARNING]
> **Live API calls are not available yet.** Shared availability constraints are stated once below;
> an operation is highlighted only when it also has a limitation of its own.

<CatalogExplorer :catalog="catalog" />
