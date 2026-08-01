# Design: a connector's presentation — a logo, and resources a listing can show

**Status:** proposed · **Pillar:** Surfaces · **Stories:** [C-436](../stories/C-436-connector-resources.md) · [C-437](../stories/C-437-decide-the-logo.md) · [C-438](../stories/C-438-lift-the-comment-urls.md) · [C-439](../stories/C-439-render-connector-presentation.md)
**Epic:** `connector-presentation`

## Why

Owner-stated 2026-08-01: *"a connector should always come with a logo, multiple potential websites or
resources (link, title, description) — having this metadata can help to individualize the listing
later."*

Measured before designing. A provider publishes fifteen keys into `catalog.json` today — `id`,
`vendor`, `description`, `authority`, `api_version`, `base_url`, `hosts`, `runtime`, `services`,
`operations`, `operation_count`, `auth`, `config_choices`, `channels`, `events` — and **not one of
them is a link or an image**. A listing rendering 54 connectors has a name, a sentence, and nothing
else to tell them apart.

**The datum that decides the shape of this epic: 35 of the 54 provider files already carry a vendor
documentation URL — in a comment.** The information exists, an author already went and found it, and
it is trapped in prose where no artifact can reach it. This is not a research problem; it is a
declaration problem.

There is also already a precedent for the field, twice over, and at the wrong level:
`ConfigField::docs_url` (`config.rs:778`) and `ManualSetup::docs_url` (`inbound.rs:366`). A *field*
can say where its documentation lives, and a *webhook setup* can. **A connector cannot.**

## Approach

### Resources are a typed list, not a bag of URLs

The request names the shape: *link, title, description*. What makes it useful rather than decorative
is a **kind** — a listing wants to render "API reference" differently from "status page", and a host
building a settings screen wants the docs link specifically, not the third entry.

A small closed set beats free text, for the reason `Format` and `Risk` are closed sets here: an open
one becomes 54 spellings of "docs". Candidates the shipped files already gesture at — homepage, API
reference, developer portal, status page, pricing, support. The set is decided in C-436 against what
the 35 existing comments actually point to, rather than guessed.

**`docs_url` must not become a third spelling.** Two levels already carry one. Either the connector
level reuses that vocabulary, or all three become one resource list at three scopes — but the
repository must not end up with a field, a setup and a connector each saying "documentation"
differently.

### The logo is a licensing question wearing a technical one

A vendor logo is a **third-party trademark**, and this repository is public. Vendoring 54 of them is
the same *class* of decision as C-415's spec vendoring — where the resolution turned out to be a split
nobody had articulated (the pulled bytes are publishable, the pull configuration is not). It is not a
question an implementor should answer by adding files, and C-437 is a decision story for that reason.

The technical constraints are real but secondary, and they narrow the choice:

- **Artifacts here are text and byte-reproducible.** SVG fits that; PNG is a binary blob in a diff
  nobody reviews. `assets/` today holds *this project's* brand — `banner.svg`, `icon-*.png` — not any
  vendor's.
- **Nothing here can fetch.** `build`, `diff` and `check` are offline by contract and
  `flux-connectors fetch` (C-14) is unbuilt, so a remote logo URL cannot be validated by the gate and
  a vendored file cannot be refreshed by it.
- **A hotlinked logo is a third-party request from a consumer's page**, which is a privacy fact a
  consumer should get to decide about, not inherit.

So the options are: vendor the mark, reference it by URL, or declare neither and let a listing supply
its own. C-437 picks one **with the licensing question answered first**, not after.

### Absent must not read as poor

Stated because this repository has now hit it four separate times — C-235, C-408, C-430 and C-433 all
had to distinguish *unstated* from *stated badly*. A connector with no logo is not a worse connector,
and a listing must not render it as one. Whatever ships, the artifact says "this connector declares no
logo", never nothing at all.

### Links rot, and this repository cannot notice

54 connectors times several resources each is a few hundred URLs that will decay silently. The gate
cannot check them — it is offline by contract. Two honest responses: a **separate, opt-in, networked**
check that is not part of `build`/`diff` (the shape C-14 already needs for spec drift), or accepting
the rot and saying so. What must not happen is a check that lives in the gate and quietly stops
running, or one that makes the build non-hermetic.

## Alternatives considered

- **A single `logo_url` and `homepage` field.** Rejected: the request is explicitly for *multiple*
  resources with titles, and two scalars would be back-filled into a list within a release.
- **Free-text resource kinds.** Rejected for the reason every other closed set here is closed —
  "docs", "documentation", "api-docs" and "reference" would all appear, and no consumer could switch
  on any of them.
- **Deriving resources from the vendored OpenAPI document.** Tempting, since `externalDocs` and
  `info.contact` exist — but only babelforce is spec-backed today, so it would cover 1 of 54 and
  leave the other 53 needing the declaration anyway.
- **Scraping the 35 comments automatically.** Rejected as the *mechanism*; a comment is prose written
  for a human and its URL may be an example, a citation, or a caveat. C-438 lifts them by review, and
  the comments are the input rather than the source.

## Risks & open questions

- **The logo licensing question is genuinely open** and is the epic's only real blocker. Everything
  else here is small.
- Whether a resource list belongs on the connector, the service, or both. A multi-service connector
  like `google` has one vendor and several products with their own documentation — the same shape
  `[[services]]` already handles for `base_url` and `api_version`.
- Whether `title` and `description` on a resource are worth the drift they invite: a title that
  restates the kind (`"Documentation"` on a `docs` resource) is noise, and a description that
  duplicates the connector's own is worse. The value is in the ones that *differ*.
- This is presentation metadata, and it must not become load-bearing. Nothing in the compile path,
  the credential path or the egress allowlist may ever read it.

## Acceptance / done

A listing can show a connector as itself: its mark, and the handful of links a person actually wants —
each typed, each declared rather than inferred, each distinguishing *unstated* from *stated*. The 35
documentation URLs already sitting in comments are declarations instead. And the logo question is
answered in the open, with its licensing settled before any file lands.
