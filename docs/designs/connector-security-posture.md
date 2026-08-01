# Design: a connector's security posture — publish the facts, and be careful about the grade

**Status:** proposed · **Pillar:** Spec · **Stories:** [C-432](../stories/C-432-mark-a-response-as-carrying-a-credential.md) · [C-433](../stories/C-433-credential-lifetime-and-rotation.md) · [C-434](../stories/C-434-publish-placement-and-verification-posture.md) · [C-435](../stories/C-435-decide-whether-a-grade-ships.md)
**Epic:** `connector-security-posture`

## Why

Owner-stated 2026-08-01: *"it would be great to have something like a security rating over a
connector — e.g. I could imagine Twilio's HMAC is quite safe compared to something using static
tokens which cannot easily be changed or rotated."*

The intuition is right, and **the gap is larger than the example**. Measured across the shipped
catalogue on the day this was written — 54 providers, 679 operations:

| Signal | What the catalogue can say today |
|---|---|
| How a credential is **placed** | Fully: 31 bearer, 15 header, 3 basic, 2 bearer+signing, 1 query, 1 basic+signing, 1 none |
| Whether a credential can be **rotated** | **Nothing.** No field expresses it |
| How long a credential **lives** | **Nothing** |
| Whether a credential can be **revoked** | **Nothing** |
| What a credential can **do** | Partly: 65 destructive + 188 high-risk operations, but no scopes (C-67 is unlanded) |
| Whether inbound events are **verified** | Yes, and the answer is thin: **2 of 54** providers declare a channel at all; 3 declare HMAC |
| Whether a response **leaks** a credential | Yes, since C-430 — a declaration the loader enforces |
| Whether an operation is **exposed** to a model | Yes, since C-413 |

`Acquisition` has exactly two variants, `Static` and `Minted`, and its own documentation says
`Minted` *"read as a placement instruction is `Static`"*. So **the axis the owner's example turns on
— rotatable versus permanent — is the one axis the catalogue does not carry at all.** That is not an
oversight in the rating; it is a missing declaration, and no rating can be computed over it until it
exists.

`connector-secrets` states the same boundary from the other side: `AGENTS.md`'s ownership table
gives it *"no expiry, refresh, rotation or revocation"*. Nobody owns the question.

One measured fact worth stating plainly because a rating would surface it immediately: **`trello`
places its credential in a query string.** A URL is logged by proxies, browsers and error trackers in
a way a header is not. That is a real posture difference and today nothing says so.

## Approach

### The thing to be careful about, stated first

**A single letter grade over a connector is the most dangerous artifact this epic could produce**,
and this repository has already found the defect it belongs to. C-430's finding was that Zoom and
Postmark *documented their hazard precisely* — *"HOST-PRIVILEGED… treat it as a credential"* — and
returned the field anyway. Describing a property is not having it.

A grade computed from declarations inherits exactly that weakness, and adds three of its own:

- **It reads as a measurement and is an opinion with arithmetic on top.** "B" invites action. Nobody
  audits the weighting that produced it.
- **It conflates independent axes.** Twilio's HMAC is an *inbound* property; a static token's
  rotation story is an *outbound* one. A connector can be excellent at one and hopeless at the other,
  and one letter cannot say so.
- **It is gameable in the wrong direction.** An author who can raise a grade by adding a declaration
  will add the declaration. The grade improves; the connector does not.

So the epic's first commitment: **publish the facts, per axis, each traceable to a declaration the
loader enforces.** A rating is a *view* over those facts, and if one ships it must be reproducible
from the published document by a consumer who disagrees with the weighting.

### The axes, and what each needs

**0 · Marking a response that carries a credential** (C-432) — added after an owner ruling the same
day: a token exchange **should** be a connector function that marks its response, and flux 0.47.1
already **refuses** an unmarked credential-shaped response rather than redacting it. So this axis is
not merely descriptive — an unmarked exchange fails. It comes first because it is the one axis where
the declaration is load-bearing at runtime.

**1 · Credential lifetime and rotation** (C-433) — the owner's example, and the one axis with no
declaration at all. A connector should be able to say whether its credential is a long-lived static
secret, a short-lived token with a refresh path, or one minted per session; whether the vendor
supports rotation without downtime; and whether revocation is possible. Each is a claim the connector
*states*, checkable against the vendor's own documentation by a reviewer, not inferred.

**2 · Placement exposure** (C-434) — derivable today and unpublished. A credential in a **query
string** ends up in proxy logs and browser history; a header does not. `trello` is the one instance,
and one instance is enough to make the fact worth publishing rather than leaving to a reader who
diffs auth schemes.

**3 · Inbound verification** (C-434, with the above) — the strongest axis this repo already has, and the thinnest
coverage: **2 of 54** providers declare an inbound channel. Twilio and Slack verify; the rest simply
have nothing to rate. The honest posture fact is "this connector declares no inbound surface", which
is different from "this connector's inbound surface is unverified", and different again from
"verified". Three states, as C-235 established for credentials.

**4 · Blast radius** — partly available: 65 destructive and 188 high-risk operations are declared per
operation, but **scopes are not** (C-67, unlanded). A posture that says "this credential can delete"
without saying "and it is scoped to one project" overstates the risk in one direction and understates
it in the other. This axis waits on C-67 rather than being approximated.

### Whether to ship a grade at all

Deliberately left as a **decision** (C-435) rather than assumed. The case for one is real — a person
choosing between two connectors wants an answer, not a table — and the case against is above. What
would make a grade defensible: it is derived from published facts by a stated function, that function
is versioned in the document, a consumer can recompute it, and the facts are always published beside
it so a disagreeing consumer can ignore it.

What would make it indefensible: shipping it before the rotation axis exists, so the first grade
this repository publishes is computed over the axis it cannot see.

## Alternatives considered

- **A single score, computed now, from what is declarable today.** Rejected as the epic's opening
  move: it would grade 54 connectors on placement and inbound verification while silently omitting
  rotation — the axis the request was actually about.
- **Deriving posture from vendor documentation by scraping or by model judgement.** Rejected: a
  posture claim must be checkable and stable across regeneration. A derived-by-inference grade cannot
  be reviewed in a diff, which is this repository's whole review model.
- **Inferring rotation from the auth scheme.** `bearer` covers both a 30-day rotating token and a
  permanent one. The inference is exactly the kind of plausible-and-wrong the `Risk` type already
  refuses to make from an HTTP method.
- **A per-operation rating.** The unit is wrong: rotation, placement and verification are connector
  and credential properties. Operation-level risk already exists and is a different question.

## Risks & open questions

- **The largest risk is that this ships and is believed.** A posture published by this repository
  describes *what a connector declares*, and a reader will hear *how safe this integration is*. Every
  artifact must carry that distinction in its own words, the way C-235's three-state credential
  requirement does.
- Whether a posture fact belongs on the connector, the credential, or both. Rotation is a property of
  the credential; inbound verification is a property of the channel; placement is a property of the
  auth method. Forcing them onto one object is how the axes get conflated.
- **A connector that declines to state a posture must be distinguishable from one that states a poor
  one** — the same trap C-235 and C-408 each hit independently. Silence is not a good grade.
- Whether a posture claim should be *checkable* against anything mechanical, or is inherently a
  reviewed assertion. C-186's `repeatable_because` is the precedent: the machine checks that a
  sentence exists and is long enough to be one; only review can check that it is true.

## Acceptance / done

The catalogue answers, per connector and per credential, in terms a consumer can act on: **how long
this credential lives, whether it can be rotated, where it is placed, and whether inbound events are
verified** — each traceable to a declaration the loader enforces, each distinguishing *unstated* from
*stated poorly*. Whether a composed grade ships is decided in the open, with its function published
and recomputable, and it does not ship before the rotation axis exists.
