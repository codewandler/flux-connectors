---
id: C-43
title: Scaffold the VitePress site and deploy to GitHub Pages
pillar: Surfaces
status: ready
priority: 5
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
- [ ] A VitePress site under `web/` that builds locally with a documented command.
- [ ] A GitHub Actions workflow builds and deploys it to GitHub Pages on push to `main`.
- [ ] The site is reachable at `codewandler.github.io/flux-connectors` (Pages must be enabled on the
      repo — if that needs a settings change you cannot make, say so plainly rather than guessing).
- [ ] Landing page states what the project is **and what does not work yet** — the README's *Known
      limits* is the model. A site that oversells is worse than no site.
- [ ] The Node toolchain is contained: everything under `web/`, its own lockfile, and it must not
      interfere with `cargo build`/`test`/`clippy`/`fmt` at the repo root.
- [ ] CI fails when the site fails to build, so a broken site cannot publish silently.

## Progress
- (not started)

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
