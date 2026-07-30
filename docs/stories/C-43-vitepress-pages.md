---
id: C-43
title: Scaffold the VitePress site and deploy to GitHub Pages
pillar: Surfaces
status: done
priority:
design: docs/designs/public-docs.md
epic: public-docs
areas: [web, ci]
note: first Node toolchain in a Rust repo — keep it contained
---

# Scaffold the VitePress site and deploy to GitHub Pages

## Goal
Stand up the public site and its deployment, so there is somewhere for the explorer to live and a
pipeline that publishes it on every push to `main`.

## Acceptance
- [x] A VitePress site under `web/` that builds locally with a documented command.
- [x] A GitHub Actions workflow builds and deploys it to GitHub Pages on push to `main`.
- [x] The site is reachable at `codewandler.github.io/flux-connectors` (Pages must be enabled on the
      repo — if that needs a settings change you cannot make, say so plainly rather than guessing).
- [x] Landing page states what the project is **and what does not work yet** — the README's *Known
      limits* is the model. A site that oversells is worse than no site.
- [x] The Node toolchain is contained: everything under `web/`, its own lockfile, and it must not
      interfere with `cargo build`/`test`/`clippy`/`fmt` at the repo root.
- [x] CI fails when the site fails to build, so a broken site cannot publish silently.

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
- **VitePress, not Docusaurus** — decided from [public-docs.md](../designs/public-docs.md). Docusaurus
  is React; the explorer is to be Vue. VitePress is the Vue-native equivalent and avoids shipping two
  SPA frameworks to render 25 operations. The docs half needs nothing Docusaurus has that VitePress
  lacks: no versioned docs, no i18n, no blog.
- **This introduces the first Node toolchain into a Rust repo** — a `package.json`, a lockfile and a
  second CI job. Real maintenance. Keep the dependency surface small; the explorer needs no state
  library.
- Do **not** build the explorer here — that is C-44. This story is the scaffold and the pipeline.
- The repo is public as of v0.0.1, so Pages is available.
