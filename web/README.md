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
```

`npm run build` is what CI runs, and it is a real gate: VitePress fails the build on a dead internal
link, so a broken site fails the workflow instead of publishing silently.

## Layout

| Path | What it is |
|---|---|
| `.vitepress/config.mts` | Site config — title, nav, sidebar, and the Pages **base path**. |
| `index.md` | Landing page: what the project is, and what does not work yet. |
| `explorer.md` | Placeholder for the provider & operation explorer. |
| `public/` | Served verbatim at the site root. Where the generated `catalog.json` will land. |

## Two things to keep right

**The base path.** A project Pages site is served from `/flux-connectors/`, so
`.vitepress/config.mts` sets `base: '/flux-connectors/'`. With the default `/` the deployed site
requests its own JS from the wrong path and renders blank — while building and previewing perfectly
on a dev server. Only a rename or a custom domain should change it.

**No hand-written catalogue data.** Everything the site says about providers and operations must come
from a generated file, not from markdown or a `.vue` component. A second, hand-maintained copy of the
catalogue is the exact failure this repository exists to correct.
