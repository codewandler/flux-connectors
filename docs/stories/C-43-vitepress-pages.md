---
id: C-43
title: Stand up the VitePress site and its Pages deployment
pillar: Surfaces
status: in-progress
design: docs/designs/public-docs.md
epic: public-docs
areas: [web, ci]
note: scaffold + deploy only — the explorer is C-44
---

# Stand up the VitePress site and its Pages deployment

## Goal
Give `flux-connectors` a public site at `codewandler.github.io/flux-connectors`, built by CI from
`main`, whose landing page states plainly both what the project is and what does not work yet. This
is the scaffold and the pipeline; the operation explorer (C-44) and its generated `catalog.json`
(C-42) land on top of it.

## Acceptance
- [x] A VitePress site under `web/` that builds locally, with the command documented in the README or
      `web/README.md`.
- [x] A GitHub Actions workflow deploying to GitHub Pages on push to `main`.
- [x] The landing page states what the project is **and** what does not work yet — the root
      `README.md`'s *Known limits* section is the model, and its content is reused.
- [x] The Node toolchain is contained: everything under `web/` with its own lockfile, and it does not
      interfere with `cargo build` / `test` / `clippy` / `fmt` at the repo root.
- [x] CI fails when the site fails to build, so a broken site cannot publish silently.
- [x] `web/node_modules` and the build output are gitignored.

## Progress
- **Scaffold landed.** `web/` holds `package.json`, `package-lock.json`, `.vitepress/config.mts`,
  `index.md`, `explorer.md` and `README.md`. One dependency: `vitepress ^1.6.4` (126 packages).
  `cd web && npm ci && npm run build` → `build complete in 1.8s`, output in `web/.vitepress/dist`.
- **Base path** is `/flux-connectors/`; verified in the built HTML — assets resolve to
  `/flux-connectors/assets/…` and the explorer link to `/flux-connectors/explorer`.
- **The build is a real gate, not a formality.** `ignoreDeadLinks: false`, proved by adding a link to
  a nonexistent page: `[vitepress] 1 dead link(s) found` and a non-zero exit. `.github/workflows/
  pages.yml` runs that build on pull requests too, so a break is caught before it can reach `main`.
- **Landing page** reuses the root README's *Known limits* verbatim under "What does not work yet",
  plus a sixth bullet saying the explorer is not implemented.
- **Containment verified**: workspace `members` are enumerated explicitly, so `web/` is invisible to
  cargo; `cargo build/test/clippy/fmt --all --check` all green with the site in the tree.
- **`web/README.md` is `srcExclude`d** — it is contributor documentation, not a published page.
- ```` ```flux ```` fences log "language 'flux' is not loaded, falling back to 'txt'". Left as-is on
  purpose; see the comment in `config.mts` — `markdown.languageAlias` turns that warning into a hard
  build failure.

### Still open
- **GitHub Pages must be enabled by a human**: Settings → Pages → Build and deployment → Source →
  **GitHub Actions**. The workflow cannot set this for itself. Until it is set, `build` runs and
  gates correctly but `deploy` fails with "Get Pages site failed".
- The explorer (C-44) and its generated `catalog.json` (C-42) are not here. `web/explorer.md` is a
  deliberately empty placeholder that says so; it names `web/public/catalog.json` as the landing spot.

## Notes
- **Framework decision is made: VitePress, not Docusaurus.** Docusaurus is React and the explorer is
  to be Vue; VitePress is the Vue-native equivalent, so the site ships one SPA framework rather than
  two. Rationale in [docs/designs/public-docs.md](../designs/public-docs.md) (option A).
- **Base path matters.** A project Pages site serves from `/flux-connectors/`, so VitePress `base`
  must be set to match or every asset and link 404s.
- **Do not hand-write catalogue content.** The explorer reads a generated `catalog.json` (C-42);
  until that exists the site carries an explicitly-marked placeholder rather than invented data.
  Hand-maintained catalogue data on the site is the action-proxy failure this repo exists to correct,
  re-enacted in JavaScript.
- Keep the dependency surface small — no state-management library.
