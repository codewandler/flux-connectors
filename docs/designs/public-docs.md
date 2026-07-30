# Design: a public docs site with a provider & operation explorer

**Status:** accepted — **option A (VitePress)** · **Pillar:** Surfaces ·
**Stories:** C-42 … C-45

## Why

`codewandler/flux-connectors` is public as of v0.0.1, and right now the only way to find out what a
connector does is to clone the repo and read TOML. The catalogue — 25 operations across three
providers, each with typed parameters, a risk tier, an idempotency class, the credentials it needs
and the hosts it reaches — is **completely invisible from outside**.

That matters more here than for a normal library, because the whole pitch is *"you don't write an
integration, you pick one."* Picking requires browsing. A public site with a **provider → operation
explorer** is what turns a repo of generated artifacts into something someone can evaluate in a
minute.

There is a second reason, less obvious and probably more important. This project's honesty about what
does **not** work is currently buried in a README section and a dozen story files. A site that shows,
per operation, *"this one is blocked on percent-encoding"* or *"this provider ships with no
credential"* makes the limits impossible to miss — which is the correct outcome, and much better than
someone discovering it after wiring a connector in.

## Approach

### The data comes from the IR — a fourth backend, not a new source of truth

We already emit three things from one IR: the Flux module, the connector manifest, and the
`connector-catalog` crate (C-38). The site is a **fourth** emitter producing a static
`catalog.json`:

```
                                  ┌─► connectors/<p>.flux          (installable)
providers/*.toml ─► Connector IR ─┼─► connectors/<p>.connector.toml (manifest)
                                  ├─► crates/catalog/…             (Rust consumers)
                                  └─► site/catalog.json            (the website)   ◄── new
```

**This is the load-bearing decision.** The site must never hand-maintain its own copy of the
catalogue — that is the action-proxy failure this whole project exists to correct, re-enacted in
JavaScript. `catalog.json` is generated, committed, and drift-checked exactly like every other
artifact.

It also means the site is **static**. Unlike `ai-agent-platform`'s console — which is a Vue app
talking to a live API — there is no backend here and no request path. Everything the explorer shows
is in a JSON file shipped alongside it. GitHub Pages serves it; nothing else runs.

### Framework: Docusaurus is React, and that is the real question

The request was "a Docusaurus website, but with an HTML+JavaScript+Vue-based explorer". Those two
halves pull against each other, so this needs deciding rather than assuming:

| Option | Docs engine | Explorer | Cost |
|---|---|---|---|
| **A — VitePress** | VitePress (Vue-native) | Vue components, directly in markdown | one framework; VitePress is less featured than Docusaurus |
| **B — Docusaurus + React explorer** | Docusaurus | React | one framework; not Vue |
| **C — Docusaurus + Vue island** | Docusaurus | Vue mounted into a custom page | **ships React *and* Vue** to every visitor |

**Recommendation: A (VitePress).** It is the Vue-native equivalent of Docusaurus, purpose-built for
exactly this — documentation with interactive Vue components embedded in markdown pages. It gives the
Vue explorer that was asked for *without* the two-runtime cost of option C, and the docs half of this
site is not doing anything Docusaurus offers that VitePress lacks (no versioned docs, no i18n, no
blog).

Option C is the literal reading of the request and it does work — but shipping two SPA frameworks to
render a catalogue of 25 operations is hard to justify, and it doubles the surface that can break.

Whichever is chosen, the explorer itself is plain components over a JSON file. There is no state
management problem here worth Pinia.

### What the explorer does

Modelled on the pattern already proven in `ai-agent-platform`'s console — list view → detail view,
with a picker and a command palette (`web/packages/console/src/views/`, `CapabilityPicker.vue`,
`CommandPalette.vue`) — but read-only and static:

- **Provider list** — vendor, operation count, auth scheme, and a status badge that does not flatter.
- **Operation list**, filterable by provider, group, risk, idempotency, and *whether it currently
  works*. The last filter is the one that earns its place.
- **Operation detail** — signature and typed parameters from the JSON Schema, the **generated Flux**
  verbatim, an equivalent **curl** with a credential placeholder (never a value), and the credentials
  and hosts it needs.
- **Deep links per operation**, which is what makes the site referenceable from an issue or a chat —
  and the natural consumer of C-37's `oip` addresses once they exist.

### Relationship to the markdown docs epic

C-31/C-32 (`provider-docs`) already generate one markdown page per provider with Flux and curl for
each operation. **The site should render those pages rather than re-implement them** — they become
the site's per-provider documentation, and the explorer is the interactive index over the same data.
Two views of one source. If `provider-docs` lands first this is nearly free; if the site lands first,
C-31 should target the site's content directory.

### Deployment

GitHub Actions builds and publishes to GitHub Pages on push to `main`. The site build joins the
existing gate, so a broken site fails CI rather than silently publishing.

## Alternatives considered

- **Docusaurus with a React explorer (option B).** Fewest moving parts and the most conventional
  choice; rejected only because Vue was explicitly asked for. If the Vue preference is soft, this is
  the option I would actually pick.
- **A hand-rolled static site, no docs framework.** Total control, and the explorer is the only real
  content anyway. Rejected: we would rebuild navigation, search and markdown rendering badly.
- **`cargo doc` / rustdoc as the public surface.** Free, but documents the *crates* rather than the
  *connectors*, which is not what a user browsing for an integration wants.
- **A live API behind the explorer.** Rejected outright: it would be a runtime, which `vision.md`
  lists as a non-goal, and there is nothing dynamic to serve.

## Risks & open questions

- **A site is a second place for the truth to rot.** Mitigated only by generating `catalog.json` and
  drift-checking it like every other artifact. If anyone hand-edits catalogue data into a `.vue`
  file, the project has lost the argument it was founded on.
- **Node toolchain in a Rust repo.** A `package.json`, a lockfile, and a second CI job — real
  maintenance the workspace does not have today. Worth it only if the site is actually published and
  kept current.
- **Publishing broken-by-design operations needs care.** `zendesk-ticket-search` is non-functional and
  Freshdesk has no credential. Showing them without their caveats would be worse than not shipping
  the site. The status badge is not decoration; it is the point.
- **Vendor names and marks.** The site will display "Zendesk", "Freshdesk" and "babelforce". Naming is
  fine; logos are the licensing question already parked in C-40.
- **Scope creep toward a registry.** A browsable catalogue is one short step from "install this
  connector", which is a product, not a docs site. Worth naming the boundary now.

## Acceptance / done

- A public site at `codewandler.github.io/flux-connectors` (or a custom domain), built and deployed
  from `main` by CI.
- Every provider and operation is browsable, filterable, and deep-linkable.
- All catalogue data on the site is **generated** from the IR and drift-checked; none of it is
  hand-maintained.
- An operation that does not currently work says so, prominently, wherever it appears.
- The site build is part of the gate, so it cannot silently break.

## Decision taken

**Option A — VitePress.** Docusaurus is React and the explorer is to be Vue; VitePress is the
Vue-native equivalent, so the site avoids shipping two SPA frameworks to render 25 operations, and
the docs half needs nothing Docusaurus offers that VitePress lacks.
