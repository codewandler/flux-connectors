---
id: C-437
title: "Decide how a connector carries a logo — the licensing question comes before the file"
pillar: Build
status: ready
priority: 2
design: docs/designs/connector-presentation.md
epic: connector-presentation
areas: [build, providers]
note: "DECISION. A vendor logo is a third-party trademark and this repository is public — the same CLASS of question C-415's spec vendoring turned out to be, where the answer was a split nobody had articulated. Not one an implementor should settle by adding 54 files"
---

# Decide how a connector carries a logo — the licensing question comes before the file

## Goal
Settle how a connector's mark reaches a listing, with the licensing question answered **first**, so
whatever lands is a decision rather than 54 files someone downloaded.

## Why this is a decision and not a task

The owner asked for a logo and the technical part is easy. The part that is not easy: **a vendor logo
is a third-party trademark, and `github.com/codewandler/flux-connectors` is public.** Vendoring 54 of
them is the same *class* of question C-415 faced for vendored specs — where the resolution turned out
to be a split nobody had articulated until it was written down (the pulled bytes are publishable, the
pull configuration is not).

The technical constraints narrow the choice but do not make it:

- **Artifacts here are text and byte-reproducible.** SVG fits; a PNG is a binary blob in a diff nobody
  reviews. `assets/` today holds *this project's* brand — `banner.svg`, `icon-128.png` — and no
  vendor's.
- **Nothing here can fetch.** `build`, `diff` and `check` are offline by contract and
  `flux-connectors fetch` (C-14) is unbuilt, so a remote logo URL cannot be validated by the gate and
  a vendored file cannot be refreshed by it.
- **A hotlinked logo is a third-party request from a consumer's page** — a privacy fact a consumer
  should decide about rather than inherit from us.

## Acceptance
- [ ] **A decision, recorded with its reasoning** in `docs/designs/connector-presentation.md`: vendor
      the mark · reference it by URL · declare neither and let a listing supply its own. Not deferred,
      not implied by absence.
- [ ] **The licensing position is stated before any image lands** — under what terms a vendor's mark
      may sit in this repository, or why none will. Many vendors publish brand guidelines that permit
      identification-use; some do not; and "many do" is not a policy.
- [ ] **If vendoring:** the format is decided (SVG, for the reviewability reason above), the source
      and its terms are recorded per file the way `specs/babelforce.provenance.toml` records a
      document's origin, and a refresh path is named even if unbuilt.
- [ ] **If referencing by URL:** the third-party-request consequence is documented for consumers, and
      the URL is validated as a URL without the gate fetching it.
- [ ] **If neither:** say what a listing should do instead, so the question is closed rather than
      re-asked in three months.
- [ ] **A connector with no logo is not a worse connector**, and whatever ships says so — the
      distinction C-235, C-408, C-430 and C-433 each had to make separately.

## Progress
- (not started)

## Notes
- **This is the epic's only real blocker.** [C-436](C-436-connector-resources.md) and
  [C-438](C-438-lift-the-comment-urls.md) are small and independent of it; a listing gets most of its
  individuality from the links alone.
- Precedent for the shape of this story: [C-402](C-402-whole-host-template-allowlist.md) and
  [C-132](C-132-decide-ivr-templates.md) both closed by recording a reasoned answer rather than by
  shipping code, and [C-415](C-415-vendor-babelforce-specs.md) is the closest analogue for the
  vendoring half specifically.
- If the answer is "vendor", the honest scope is **not** 54 logos in one change. One provider, the
  policy, and the provenance shape — then the rest as a mechanical follow-up.
