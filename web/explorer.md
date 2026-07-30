# Provider & operation explorer

::: danger Not implemented yet
There is no explorer on this site. This page is a placeholder so the navigation, the route and the
deployment exist before the feature does — it deliberately shows **nothing** about the catalogue
rather than a hand-written approximation of it.

To see what actually ships today, read the generated artifacts in the repository:
[`connectors/`](https://github.com/codewandler/flux-connectors/tree/main/connectors) and
[`providers/`](https://github.com/codewandler/flux-connectors/tree/main/providers).
:::

## What goes here

A read-only, static browser over the whole catalogue — provider list, filterable operation list, and
an operation detail view with the generated Flux, an equivalent `curl` carrying a credential
*placeholder* (never a value), and the credentials and hosts the operation needs. Every operation
deep-links, so it can be referenced from an issue or a chat. Designed in
[docs/designs/public-docs.md](https://github.com/codewandler/flux-connectors/blob/main/docs/designs/public-docs.md).

The filter that earns its place is **whether the operation currently works**. Several do not, by
decision rather than by accident, and an explorer that hid that would be worse than no explorer.

## Why it is empty

The explorer renders a **generated** `catalog.json`, emitted from the connector IR by the same build
that emits the `.flux` modules and the manifests, and drift-checked like every other artifact. That
emitter is a separate change and does not exist yet.

Until it does, this page stays empty on purpose. Hand-maintaining catalogue data in a `.vue` file
would recreate, in JavaScript, exactly the hand-written-integration failure this project exists to
correct — and a site that overstates what works costs more credibility than a missing page does.

## For the implementor

When the generated catalogue lands, it belongs at `web/public/catalog.json` (VitePress serves
`public/` verbatim, so the built site fetches it from `/flux-connectors/catalog.json`). It must be
**copied or generated into place by the build**, never edited by hand, and the copy step must fail
loudly when the source is missing rather than shipping a stale or empty file.

The explorer components are plain Vue over that JSON. There is no state worth a store here.
