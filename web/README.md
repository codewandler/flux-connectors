# web — the public documentation site

The [VitePress](https://vitepress.dev) site published to
<https://codewandler.github.io/flux-connectors/> by
[`.github/workflows/pages.yml`](../.github/workflows/pages.yml) on every push to `main`.

**The Node toolchain is contained here.** `package.json`, `package-lock.json` and `node_modules` all
live under `web/`; nothing about the Rust workspace at the repository root knows or cares that this
directory exists.

## Build

Requires Node 22+.

```bash
cd web
npm ci          # or `npm install` on first setup, to create the lockfile
npm run build   # static site into web/.vitepress/dist
```

Other scripts:

```bash
npm run dev      # local dev server with hot reload
npm run preview  # serve the built output, base path included
npm test         # the explorer's contract with the catalogue — run after `npm run build`
```

`npm test` is Node's built-in runner over `test/*.test.mjs`; it adds no dependency. It reads
`public/catalog.json` and the built HTML in `.vitepress/dist`, so it must follow a build.

`npm run build` is what CI runs, and it is a real gate: VitePress fails the build on a dead internal
link, so a broken site fails the workflow instead of publishing silently.

## Layout

| Path | What it is |
|---|---|
| `.vitepress/config.mts` | Site config — title, nav, sidebar, and the Pages **base path**. |
| `.vitepress/theme/` | The default theme plus the explorer's Vue components, registered globally. |
| `index.md` | Landing page: what the project is, and what does not work yet. |
| `explorer.md` | The provider & operation explorer. |
| `operations/[operation].md` | One pre-rendered page per operation, enumerated from the catalogue. |
| `data/` | The catalogue's types, the questions the explorer asks of it, and the build-time loader. |
| `public/` | Served verbatim at the site root. Holds the generated `catalog.json`. |
| `test/` | The explorer's contract with the catalogue, over the built site. |

## Public content boundary

This site is for connector consumers. It explains available services and operations, their call
contracts, safety metadata, credentials, hosts, and current availability. Internal designs, roadmap
and story mechanics, crate architecture, and agent instructions belong in the repository docs and
must not be linked or reproduced on the public pages.

The navigation, hero, and favicon use `public/brand/{icon,mark}.svg`. They are published copies of
the canonical files in `assets/brand/`; `npm test` compares them byte for byte so the two locations
cannot drift.

## Two things to keep right

**The base path.** A project Pages site is served from `/flux-connectors/`, so
`.vitepress/config.mts` sets `base: '/flux-connectors/'`. With the default `/` the deployed site
requests its own JS from the wrong path and renders blank — while building and previewing perfectly
on a dev server. Only a rename or a custom domain should change it.

**No hand-written catalogue data.** Everything the site says about providers and operations must come
from a generated file, not from markdown or a `.vue` component. A second, hand-maintained copy of the
catalogue is the exact failure this repository exists to correct.

`public/catalog.json` is written by `cargo run -p connector-cli -- build` and committed — the same
plan and drift check as every other generated artifact (`crates/connector-cli/tests/site_catalog.rs`).
It is not copied here by the Node build, and it must not be edited. The last test in
`test/explorer.test.mjs` enforces the rule mechanically: it fails if any explorer source names a
provider, an operation, a credential, a host or an issue code.
