---
id: C-437
title: "Decide how a connector carries a logo — the licensing question comes before the file"
pillar: Build
status: done
design: docs/designs/connector-presentation.md
epic: connector-presentation
areas: [build, providers]
note: "DECIDED 2026-08-01 — NEITHER. No vendor mark is vendored and no logo_url is declared; a listing derives a monogram from the published vendor+id, or brings its own asset pack. A brand guideline grants identification use to the DISPLAYER, revocably — this repo's MIT/Apache-2.0 grants copy, modify and sublicense to everyone, irrevocably, and git makes that unwithdrawable. Also answers C-40"
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
- [x] **A decision, recorded with its reasoning** in `docs/designs/connector-presentation.md`: vendor
      the mark · reference it by URL · declare neither and let a listing supply its own. Not deferred,
      not implied by absence.
      → **neither**; `docs/designs/connector-presentation.md` § *The logo decision (C-437, 2026-08-01)*
- [x] **The licensing position is stated before any image lands** — under what terms a vendor's mark
      may sit in this repository, or why none will. Many vendors publish brand guidelines that permit
      identification-use; some do not; and "many do" is not a policy.
      → § *The licensing position, which comes first*. **None will**, on terms rather than on count.
- [x] **If vendoring:** the format is decided (SVG, for the reviewability reason above), the source
      and its terms are recorded per file the way `specs/babelforce.provenance.toml` records a
      document's origin, and a refresh path is named even if unbuilt.
      → answered **conditionally**, as the shape of the one door left open: § *The one door left open,
      and its shape* — SVG, `assets/vendor/<id>.svg`, `assets/vendor.provenance.toml` with `source_url`,
      `obtained_at`, `sha256`, `grant_url` and a verbatim `grant`; refresh by hand and by hash until
      C-14's `fetch` exists. Nothing is vendored today, so no file carries it yet.
- [x] **If referencing by URL:** the third-party-request consequence is documented for consumers, and
      the URL is validated as a URL without the gate fetching it.
      → the consequence is documented as the **reason the option is refused** (§ *Why not a `logo_url`
      either*, point 1). No URL is declared, so there is none to validate; C-436 still owes URL-shape
      validation for `[[resources]]`, which is its item, not this one.
- [x] **If neither:** say what a listing should do instead, so the question is closed rather than
      re-asked in three months.
      → § *What a listing should do instead* (generated monogram · own asset pack · build-time
      resolution, never render-time) and § *What this forecloses*.
- [x] **A connector with no logo is not a worse connector**, and whatever ships says so — the
      distinction C-235, C-408, C-430 and C-433 each had to make separately.
      → § *Absence stops being a per-connector state at all*. Under this decision **no** connector
      declares a mark, so there is no per-connector absence to render badly; the trap cannot arise from
      marks. `Published<T>` stays the mechanism for the resource list, where it does.

## Progress
- **Done — decided, not deferred. The answer is `neither`.** No behavioural change: this story writes
  the design doc, its own record, and the one place in the tree that a person actually asks the
  question (`assets/brand/README.md` § *Not covered*, which had been pointing at C-40 since it was
  written). No Rust, no provider TOML, no artifact moved.
- **The licensing position, in one line.** A brand guideline grants *identification use* — revocably,
  non-transferably, non-sublicensably, conditioned on not modifying — to the party **displaying** the
  mark. `LICENSE-MIT` and `LICENSE-APACHE` grant *copy, modify and sublicense* — perpetually, to
  **everyone** — over everything in this repository. Vendoring puts those two in direct contradiction
  over bytes the project does not own, and `git` history means a revocation could not be honoured even
  if we wanted to. Apache-2.0 § 6 does not save it: it leaves the file's status undefined, not safe.
- **Why C-415's split does not transfer**, which is the part worth keeping. An OpenAPI document is
  published *in order to be* implemented against — copying it is its purpose, so a scrub of the
  material that must not travel makes the bytes publishable. A trademark exists *in order not to be*
  copied. There is nothing in the file to scrub, because the file is the problem. Refusal, not split.
- **`logo_url` was refused on the privacy fact, not the legal one** — legally a URL is cheap. C-439
  already rules a resource link is never fetched, prefetched or previewed; an `<img src>` is that rule
  inverted, and 54 of them fire from every visitor's browser before anyone chooses anything.
- **This answers [C-40](C-40-provider-icons.md) too.** It has been `backlog` since it was filed, noting
  *"blocked on a licensing answer, not a technical one"*. That answer is now written, and it refuses
  the story as worded — C-40 needs closing or rewriting by whoever owns the board. Its *shape*
  survives: a mark ships beside the `.flux`, never base64 inside it.
- **Two follow-ups this decision creates, not filed here** (ID allocation and the board are
  coordinator-owned, and other implementors are running):
  1. **Render the generated monogram** — deterministic glyph from `vendor` and hue from `id`, the
     *same* function reachable by `web/` and `crates/connectors-api` so the two cannot drift (C-236's
     concern). Fold into [C-439](C-439-render-connector-presentation.md) or file beside it.
  2. **Close or rewrite C-40** against this answer.
- **Gate run in full despite no code**, to prove the tree was left green: `cargo test --workspace
  --no-fail-fast`, `cargo fmt --all --check`, `cargo run -p connector-cli -- diff`. Output in the
  handoff.

## Notes
- **This is the epic's only real blocker.** [C-436](C-436-connector-resources.md) and
  [C-438](C-438-lift-the-comment-urls.md) are small and independent of it; a listing gets most of its
  individuality from the links alone.
- Precedent for the shape of this story: [C-402](C-402-whole-host-template-allowlist.md) and
  [C-132](C-132-decide-ivr-templates.md) both closed by recording a reasoned answer rather than by
  shipping code, and [C-415](C-415-vendor-babelforce-specs.md) is the closest analogue for the
  vendoring half specifically.
- If the answer is "vendor", the honest scope is **not** 54 logos in one change. One provider, the
  policy, and the provenance shape — then the rest as a mechanical follow-up. *(Moot: the answer is
  not "vendor". The provenance shape was recorded anyway, as the shape the one exception would take.)*
- **Small correction, measured while deciding.** This story and the design both say `assets/` holds
  `banner.svg` and `icon-*.png`. They live in **`assets/brand/`**; `assets/` itself holds only the
  three README-snippet files. The distinction matters here because `assets/brand/README.md` already
  carried a *"Not covered"* section naming the trademark question and pointing at C-40 — it was the
  one place in the tree the question was asked out loud, and it is now the place the answer is stated.
- **Nothing in the tree references a logo today**, checked rather than assumed: no `logo`/`icon` key
  in any of the 54 provider files, and none of the fifteen keys a provider publishes into
  `web/public/catalog.json` is an image or a link. So this decision forecloses rather than removes.
