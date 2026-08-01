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
  nobody reviews. `assets/brand/` today holds *this project's* brand — `banner.svg`, `icon-*.png`,
  `mark.svg` — not any vendor's, and its README already says so.
- **Nothing here can fetch.** `build`, `diff` and `check` are offline by contract and
  `flux-connectors fetch` (C-14) is unbuilt, so a remote logo URL cannot be validated by the gate and
  a vendored file cannot be refreshed by it.
- **A hotlinked logo is a third-party request from a consumer's page**, which is a privacy fact a
  consumer should get to decide about, not inherit.

So the options are: vendor the mark, reference it by URL, or declare neither and let a listing supply
its own. **C-437 decided, below, with the licensing question answered first.**

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

## The logo decision (C-437, 2026-08-01)

**Neither. A connector declares no mark — not vendored bytes, not a `logo_url`.** What the catalogue
declares in its place is the vendor's canonical **homepage**, as one of C-436's `[[resources]]`, and a
listing obtains or generates a mark from that on its own terms. This repository declares *facts about
vendors*; a mark is not a fact about a vendor, it is the vendor's property, displayed under terms that
run to whoever displays it.

### The licensing position, which comes first

**No vendor's mark may sit in this repository, and no vendor's brand guideline can change that** —
because a guideline grants the wrong right, to the wrong party, on terms this repository cannot honour.

A mark file is **two rights at once**: a **trademark**, whose *identification use* is the thing a brand
guideline permits, and a **copyrighted graphic work**, whose *redistribution* the guideline almost never
addresses. Vendoring engages both. A guideline answers one.

And the terms a guideline typically grants are exactly the three this repository has already given away
to every recipient, in `LICENSE-MIT` and `LICENSE-APACHE`:

| what a brand guideline typically grants | what this repository's licences already grant everyone |
|---|---|
| revocable at the vendor's discretion | perpetual |
| non-transferable, non-sublicensable | "sublicense" is in `LICENSE-MIT`'s first paragraph |
| conditioned on not modifying the mark | "modify" is in both |

So the moment a vendor's SVG lands under `assets/`, this repository's own licence files purport to grant
every recipient the right to copy, **modify** and **sublicense** bytes the project does not own and
cannot license. Apache-2.0 § 6 does not rescue that: withholding trademark rights from the grant leaves
the file's status **undefined** rather than safe, and a recipient reading `LICENSE-MIT` has no way to
tell which files it does not cover.

**Revocability fails first, and it fails hardest.** `git` history makes any grant permanent in practice:
a vendor asking for its mark back cannot be satisfied by deleting a file. A revocable permission is
therefore one whose revocation this repository would be *unable to honour* — the same
"expensive to undo once pushed" fact C-415's scrub is built around, arriving here as a reason not to
push in the first place.

**"Many vendors permit identification use" cannot become policy**, and the reason has nothing to do
with how many do. It is 54 separately-worded documents, each amendable unilaterally by its author, none
of which anything in this repository can read, check, or re-check when it changes. Every other rule
here is enforced by a test that fails on a hit — the scrub by
`crates/connector-spec/tests/vendored_specs.rs`, the offline invariant by
`crates/connector-cli/tests/no_network.rs`. A rule whose only enforcement is "somebody read 54 legal
documents once" is not a policy; it is a standing liability with no owner and no expiry.

**C-415 is the near analogue, and it settles this by contrast rather than by precedent.** Its split was
available because an OpenAPI document is *published in order to be implemented against*: copying it is
the use it exists for, and what had to come out was material identifying people and internal systems. A
trademark is the exact inverse — it exists to distinguish one party's goods from another's, and copying
it is the use it exists to prevent. There is no scrub that makes a mark safe to redistribute, because
there is nothing *in* it to remove: the file is the problem, not its contents. That is why C-415's shape
does not transfer, and why the answer here is a refusal rather than a split.

### Why not a `logo_url` either

A URL is a fact, and referencing one carries none of the above — which is what makes it the tempting
answer. It fails on three other grounds.

1. **An `<img src>` is not a link.** C-439 already rules that a resource link is rendered as a link and
   **never fetched, prefetched or previewed**, because a listing that hits 54 vendors when a page opens
   is a third-party-request surprise for whoever deployed it. A `logo_url` is that rule inverted: an
   image URL in the catalogue *is* a fetch, made by every visitor's browser, before anybody chose
   anything. Declaring one makes a privacy decision on behalf of every consumer that renders this
   catalogue — and the position above is that a consumer decides about a third-party request rather
   than inherits it.
2. **The gate cannot see it rot.** `build`, `diff` and `check` are offline by contract and `fetch` is
   unbuilt (`crates/connector-cli/src/cli.rs:103`). A vendor CDN path is not a documented interface,
   carries no compatibility promise, and moves without notice; 54 of them decay silently into 54 broken
   images and nothing here would ever notice.
3. **It is still a use of the mark, only relocated.** Hotlinking serves the vendor's bytes from the
   vendor's bandwidth into a page the vendor never saw. That is a weaker exposure than vendoring, not an
   absent one — and it is one this repository would be manufacturing for its consumers rather than
   accepting for itself.

Referencing by URL is the option that looks free because its cost is paid downstream.

### What a listing should do instead — so this is closed, not re-asked

Three answers, in increasing order of effort. A listing may take any of them without needing anything
from this repository beyond what C-436 already declares.

1. **Generate a monogram.** One or two letters from `vendor`, on a hue derived deterministically from
   `id`. No third-party bytes, no network request, no rights question, byte-reproducible like every
   other artifact here, and identical in `web/` and `crates/connectors-api` because both derive it from
   the same two published fields. **Every** connector gets one, which is the individualisation the epic
   was asked for.
2. **Supply an asset pack.** A listing that wants real marks keys them by connector `id` — stable,
   published and drift-checked — and obtains them under its own agreement with each vendor. That
   agreement binds the party actually displaying the mark, which is the only party identification-use
   permission ever ran to.
3. **Resolve at build time, never at render time.** A listing deriving a mark from the homepage's origin
   (favicon, `og:image`) does it into its own asset pack, in its own build, having decided about the
   third-party request deliberately. Doing the same thing in the browser is (1)'s problem with none of
   (1)'s properties.

This project's own two surfaces take (1). C-439 renders it.

### Absence stops being a per-connector state at all

Because **no** connector declares a mark, there is no per-connector state for a listing to render as
missing, and the trap C-235, C-408, C-430 and C-433 each hit separately — *unstated* reading as *stated
badly* — cannot arise from marks. Absence is uniform, carries no signal, and every connector gets the
same generated monogram, so a connector without a vendor mark is not visibly a connector without
anything. `Published<T>` (`web/data/catalog.mts:32`) remains the mechanism for the **resource list**,
where absence *is* per connector and *does* need distinguishing.

### What this forecloses

Stated so each is a refusal with a reason rather than an omission somebody re-proposes:

- No `logo`, `logo_url`, `icon`, `icon_url` or `mark` key on a provider TOML, on the IR, in a manifest,
  or in `catalog.json`.
- No vendor bytes under `assets/`. `assets/brand/` is this project's own brand and stays that; its
  README already parked the question, and this is the answer it was waiting for.
- **C-40 ("Ship provider icons as bundle assets") is answered `no` in its vendored form.** It has sat
  `backlog` since it was filed, with the note *"blocked on a licensing answer, not a technical one"*.
  This is that answer. What survives is C-40's *shape* — a mark ships as a file beside the `.flux`,
  never base64 inside it, drift-checked like any artifact — which stays correct for a generated
  monogram if one is ever emitted as a file rather than derived by the renderer.
  `docs/designs/connector-bundle.md`'s open question *"Where do icons come from?"* is closed with it:
  not from vendors.

### The one door left open, and its shape

A **specific** vendor giving a **written, transferable, irrevocable** grant to redistribute its mark
under this repository's terms. That opens per vendor, never per policy, and it does not reopen the
decision — it adds one exception to it. The shape is settled now so the first grant does not relitigate
the format:

- **SVG**, for the reason every artifact here is text: it reviews as a diff, and a PNG is a blob nobody
  reads. No `<style>` and no `<script>` — GitHub's sanitiser strips both (`assets/brand/README.md`), and
  a mark that needs script is not a mark.
- **`assets/vendor/<connector id>.svg`**, keyed by the catalogue's own id so the join is the one
  consumers already have.
- **Provenance per file** in `assets/vendor.provenance.toml`, in the shape
  `specs/babelforce.provenance.toml` established: `source_url`, `obtained_at`, and `sha256` of the
  committed bytes, plus two of its own — `grant_url` and `grant`, the granting sentence quoted verbatim.
  A grant that cannot be quoted and linked is not a grant; it is somebody's recollection of a web page.
- **Refresh is by hand and by hash until `fetch` exists.** `flux-connectors fetch` (C-14) is the only
  command that may ever reach the network. Until it is built, a mark is re-obtained by hand and the
  change shows up as a `sha256` that moved — exactly how a vendored spec works today.

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
- **Vendor only the marks whose terms are unambiguously compatible.** The narrow reading of the
  licensing position above, and it survives it — a written, transferable, irrevocable grant is exactly
  the door left open. Rejected as a *general* answer for two reasons. Almost no vendor publishes one: a
  brand guideline is a permission, not a licence, and it addresses use rather than redistribution. And a
  catalogue where six of 54 carry a mark is the "absent reads as poor" trap at its worst — six
  connectors would look endorsed and 48 would look unfinished, from a difference that is purely about
  each vendor's legal department.
- **A `logo_url` that consumers are told not to render eagerly.** Rejected: a documented request not to
  fetch is not a mechanism, and the field's only purpose is to be put in an `<img src>`. The rule would
  be broken by the first consumer that did the obvious thing, and this repository would have no way to
  know.

## Risks & open questions

- ~~**The logo licensing question is genuinely open** and is the epic's only real blocker.~~ **Closed
  by C-437**, above: no mark is declared, and the epic's remaining work is the resource list. Everything
  left here is small.
- **The generated monogram is a recommendation, not yet a rule.** It is what the decision points a
  listing at, and this project's own surfaces should take it — but it is C-439's to build and nothing
  yet asserts that `web/` and `crates/connectors-api` derive the same glyph and the same hue from the
  same two fields. Two surfaces computing it independently is exactly the drift C-236 exists to close.
- Whether a resource list belongs on the connector, the service, or both. A multi-service connector
  like `google` has one vendor and several products with their own documentation — the same shape
  `[[services]]` already handles for `base_url` and `api_version`.
- Whether `title` and `description` on a resource are worth the drift they invite: a title that
  restates the kind (`"Documentation"` on a `docs` resource) is noise, and a description that
  duplicates the connector's own is worse. The value is in the ones that *differ*.
- This is presentation metadata, and it must not become load-bearing. Nothing in the compile path,
  the credential path or the egress allowlist may ever read it.

## Acceptance / done

A listing can show a connector as itself: a mark it derives rather than one this repository ships, and
the handful of links a person actually wants — each typed, each declared rather than inferred, each
distinguishing *unstated* from *stated*. The 35 documentation URLs already sitting in comments are
declarations instead. And the logo question is answered in the open, with its licensing settled before
any file landed — the answer being that none will.
