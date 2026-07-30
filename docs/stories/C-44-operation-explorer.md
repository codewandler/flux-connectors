---
id: C-44
title: Build the provider and operation explorer
pillar: Surfaces
status: ready
priority: 4
design: docs/designs/public-docs.md
epic: public-docs
areas: [web]
note: the reason the site exists · needs C-42 and C-43
---

# Build the provider and operation explorer

## Goal
Let someone browse every provider and operation, filter to what they need, and deep-link to one —
turning a repo of generated artifacts into something evaluable in a minute.

## Acceptance
- [x] Provider list: vendor, operation count, auth scheme, and a status badge that does not flatter.
- [x] Operation list, filterable by provider, risk, idempotency, and **whether it currently works**.
- [x] Operation detail: signature and typed parameters from the JSON Schema, the **generated Flux**
      verbatim, and the credentials and hosts it needs.
- [x] **Deep links per operation**, so the site is referenceable from an issue or a chat.
- [x] An operation that does not work says so prominently, wherever it appears.
- [x] All data read from the generated `catalog.json` (C-42). **Nothing hand-maintained.**
- [x] Works without JavaScript for at least the operation content, or degrades to something useful.

## Progress
- **Unblocked** — C-42 (`site/catalog.json`) and C-43 (VitePress under `web/`) are both merged.
- **Coordinator decision: move the document to `web/public/catalog.json`.** Two top-level directories
  for one website is a smell C-42 flagged, and VitePress serves `web/public/` at the site root — so
  the explorer can fetch `/flux-connectors/catalog.json` with no copy step and no build plumbing.
  `SITE_DIR` in `crates/connector-cli/src/workspace.rs` is the single line; regenerate afterwards.
- **`works` is `false` for all 25 operations today**, correctly: no provider can make a live call.
  Do **not** render "0 of 25 working" — filter on `scope: "operation"` to show the 5 operations that
  own a defect (`zendesk-ticket-search`, `babelforce-agent-list`, `babelforce-call-list`,
  `freshdesk-ticket-list`, `freshdesk-contact-list`) and present provider- and catalog-scoped issues
  as banners rather than per-operation failures. C-42 put `scope` on each issue for exactly this.

### Done

- **The document moved to `web/public/catalog.json`.** `SITE_DIR` in `workspace.rs` is one line, but
  not the only one: `crates/connector-cli/tests/site_catalog.rs` pins the path in a `const`, so the
  move is two edits under `crates/` rather than one. Regenerated; a rebuild reports 35 artifacts up
  to date. Nothing copies the file — VitePress serves `public/` verbatim.
- **Pre-rendered, not fetched.** A VitePress data loader (`data/catalog.data.mts`) reads the document
  at build time, so every page is complete in static HTML: the explorer content and all 25 operation
  pages work with JavaScript off, and a missing or malformed catalogue fails the site build instead
  of rendering an empty explorer at runtime. The file still ships at `/catalog.json` for anything
  else that wants it.
- **Deep links are real pages.** `operations/[operation].md` + its `.paths.mts` enumerate one
  pre-rendered page per operation from the catalogue, at `/flux-connectors/operations/<id>`. The
  `<h1>` is injected through the `<!-- @content -->` marker so the document title is the operation's
  own name — a pasted link previews as the operation, not as the site.
- **`scope` drives the whole presentation**, as C-42 intended. An operation-scoped issue is a red
  `Known defect` badge on the row, a red border, the summary inline in the list, and a warning block
  directly under the title on its own page. A provider- or catalog-scoped issue is a banner — once
  over the catalogue, once per provider card — plus a neutral "Conditions this operation inherits"
  block at the *foot* of the detail page. Five operations are marked; twenty are not, and none of the
  twenty is dressed up as broken. The headline counts operations that own a defect; there is no
  "0 of 25", and a test asserts the string cannot appear.
- **Nothing hand-maintained, enforced mechanically.** The last test in `web/test/explorer.test.mjs`
  fails if any explorer source names a provider id, a vendor, a base URL, a host, a credential or an
  issue code. The filters, the facet options, the counts and the signature all derive from the data —
  the signature is the first line of the emitted Flux rather than a second renderer for it.
- **No new dependency.** The site still has exactly one (`vitepress`). The test suite is Node's
  built-in runner over the built site: `npm run build && npm test`, 8 tests.
- **The provider badge flips on its own.** It reads "Not live yet" from `works` being false across a
  provider's operations, and will read "Live" with no edit here once the auth seam lands.

### Still open

- **`ownIssues` is the only place `scope === 'operation'` is spelled out.** When C-37's `oip`
  addresses land, deep links should use the address rather than the local symbol —
  `operationHref()` in `data/catalog.mts` is the one function to change.
- The landing page (`web/index.md`) still carries a hand-copied Flux snippet and hand-written
  operation counts from C-43. Both are catalogue data on a page that is not the explorer; worth
  folding into the generated source, but out of this story's scope.

## Notes
- Modelled on the pattern in `~/babelforce/projects/ai-agent-platform/web/packages/console` — list →
  detail, `CapabilityPicker.vue`, `CommandPalette.vue` — but **read-only and static**. That console
  talks to a live API; this has no backend and must not grow one (`vision.md` lists a runtime as a
  non-goal).
- Plain Vue components over a JSON file. There is no state-management problem here worth Pinia.
- Once C-37 lands, deep links should use the `oip` address rather than the local symbol.
