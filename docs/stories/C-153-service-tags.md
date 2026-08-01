---
id: C-153
title: "Tag a service with what kind of thing it is, so a catalogue can be filtered"
pillar: Spec
status: done
priority: 1
design: docs/designs/provider-roles.md
epic: provider-roles
areas: [connector-spec, codegen]
note: "a tag is NOT a role — it carries no required members and is never evidence anything is callable. 54 providers and 63 services and no way to ask which are telephony or payments; the axis is the service's own domain, not the vendor's target market"
---

# Tag a service with what kind of thing it is, so a catalogue can be filtered

## Goal

Let a service declare what **kind** of thing it is — `babelforce` is telephony, `stripe` is payments,
`google`'s `drive` is storage while its `gmail` is email — so a catalogue of **54 providers and 63
services** can be filtered by domain rather than scrolled.

## The axis is the service's own domain, not the vendor's target market

Settled 2026-08-01. "Which industry is this business in **or targeting**" is two questions, and only
the first belongs here:

- **What the service *is*** — telephony, payments, support. Derivable from `providers/*.toml` and the
  vendored specs this repository already holds, and checkable against them.
- **Who the vendor *sells to*** — contact centres, ecommerce, developers. A fact about a company's
  go-to-market that appears in no API document, cannot be derived from anything this repository
  compiles, and goes stale without any signal that it has. It is not refused on principle; it is
  simply not this field. File it separately if a consumer ever needs it.

## Measured, 2026-08-01 — the fleet this must cover

`ls providers/*.toml | wc -l` → **54**. `web/public/catalog.json` → **54 providers, 63 services**.
Neither `Provider` nor `Service` nor `catalog.json` carries any category field today: the provider
keys are `api_version, auth, authority, base_url, channels, config_choices, description, hosts, id,
operation_count, operations, runtime, services, vendor`.

A first clustering pass over all 54 descriptions produces roughly 25 domains and covers every
provider, with the user's two named cases falling out directly — **telephony**: `babelforce`,
`twilio`; **payments**: `stripe`. The design record owns the final vocabulary; what the pass settles
is that one is derivable rather than invented, which is what this story's acceptance requires.

Three shipped cases the vocabulary must survive, and they are why the field is on the **service**:

- **`google`** → `gmail` / `calendar` / `drive`, which are three different domains under one vendor.
- **`microsoft_graph`** → `mail` / `calendar` / `files`, the same divergence again.
- **`twilio`** → one `default` service that is genuinely two domains at once (*"programmable
  messaging and voice"*), so a service carries a **set**, not one value.

The `youtube` half of this story's original example is still contingent —
[C-154](C-154-google-youtube-service.md) is `ready` and has not landed — so the cases above replace
it as the worked example rather than sitting alongside it.

The honest cost to decide in the design: several domains are **singletons** across the fleet
(`algolia` is the only search vendor, `okta` the only identity one, `shopify` the only ecommerce
one). A singleton tag is accurate and a weak filter; whether it folds into a broader domain or
stands alone is the judgement the clustering must record, not paper over.

## A tag is not a role, and that distinction is the story

[C-119](C-119-provider-roles-epic.md)'s design says, in as many words, *"an open string set is a tag
system, and a tag system cannot be checked."* That is still true. Tags are the tag system, **filed
deliberately as a second field with weaker guarantees**, not as a loosening of roles:

| | `roles` | `tags` |
|---|---|---|
| answers | "can this service **do** X, checkably?" | "what **kind** of thing is this?" |
| carries | required members | nothing |
| refuses | an unknown name, **and** a claim the members do not satisfy | an unknown name only |

**Why not one field.** Giving `office` a required-member list is meaningless — no operation makes a
service "office". And letting a role carry no members turns every role into an unchecked assertion,
which is precisely what C-119's closed set exists to prevent.

## Acceptance

- [x] A **service** declares `tags = [...]`, beside `roles`, in the IR and the loader. `Service` gains
      the field with `skip_serializing_if` so a service declaring none hashes exactly as before.
- [x] A provider's tags are **derived** as the union of its services', never authored — the same rule
      `roles` and `Level` already follow.
- [x] **A closed vocabulary.** An unknown tag is refused at load, naming the known set. A typo'd tag
      silently means "absent from that filter", which is the same shape of silent-nothing failure a
      typo'd role would be — cheaper to be wrong about, but not free.
- [x] **The vocabulary is derived from what actually ships**, not invented ahead of need. Read all
      **54** shipped providers and propose the set that covers them, rather than a taxonomy nothing
      populates. Record the clustering in the design, including which domains are singletons and
      whether each stands or folds.
- [x] **`telephony` and `payments` are in the vocabulary**, and `babelforce`, `twilio` and `stripe`
      carry them — the cases the request names.
- [x] **A service carries a set, not one value**, proven by `twilio`, whose single `default` service
      is both messaging and voice.
- [x] **Service-level tagging is proven by a provider whose services diverge**: `google`'s `gmail`,
      `calendar` and `drive` do not share a domain, and neither do `microsoft_graph`'s `mail`,
      `calendar` and `files`. A test asserts a provider's derived union is wider than any one of its
      services' — otherwise the field would be provider-level in all but name.
- [~] Tags reach the manifest and `catalog.json`, `catalog` gains a tag query, and the explorer can
      **filter by tag**. **Split out to [C-442](C-442-tags-and-roles-reach-an-artifact.md)** — see
      Progress. Not descoped: a tag no consumer uses is a field, not a feature, and C-442 is where it
      becomes one.
- [x] **Failing-first test:** `an_unknown_tag_is_refused_and_names_the_known_set`.
- [x] A test asserts the tag set is **non-empty and multi-valued** across shipped services, so the
      filter cannot pass vacuously.
- [x] No shipped provider's emitted module changes — a tag is catalogue metadata and reaches no
      `.flux`. Assert it.
- [x] The gate is green; the build stays a fixed point.

## Progress

- **Closed 2026-08-02 as PARTIAL — the declaration landed, the projection did not.** Eleven of
  twelve acceptance items are met; the twelfth is
  [C-442](C-442-tags-and-roles-reach-an-artifact.md).
- **Why it split rather than finished.** The artifact half needs a surface-to-artifact projection
  this repository does not have — and `roles`, which shipped in C-120, does not have it either.
  `connector-surfaces.md:226` assigns that projection to [C-121](C-121-llm-catalogue-role.md), and
  `AGENTS.md` §Intentional gaps says in as many words: *"Do not close this by widening the manifest
  ad hoc; the surface-to-artifact mapping is decided in `connector-surfaces.md`."* Building a tag-only
  path here would have left `roles` dead beside it and pre-empted a decision two other stories own.
  **Measured while deciding:** `catalog.json`'s service objects carry
  `api_version, base_url, description, gid, hosts, name, operation_count` and no `roles` — so the
  belief that roles already reach the catalogue is false.
- **What landed**, verified in this session: `Tag` (27 values), `Service.tags`, `Connector::tags()`,
  four loader refusals, all 54 providers tagged, `service_tags.rs` 7/7 green, `cargo test --workspace`
  green, `cargo fmt --all --check` clean.
- **The cost that was not predicted, and is now recorded in the design.** 47 of 54 providers have
  only the implicit `default` service, so their tag lives in a `[[services]]` entry naming `default`
  — the path C-120 opened and nobody had written. 54 `ir_sha256` values moved and `connectors.lock`
  churned 108 lines. `connector-cli -- build` reports **`1 written`**: the lockfile, and nothing else.
  No address, no filename, no emitted `.flux` byte moved. This story's own "hashes exactly as before"
  clause holds only for a service declaring *no* tag; the design doc now says so.
- **Three tests and one defect fell out of the same spelling.** `services.rs`, `resend_connector.rs`
  and `typeform_connector.rs` each wrote "no addressing surface" as `services.is_empty()`; all three
  now assert `is_default_only()` plus that the entry reaches for no `base_url`/`api_version`/
  `description`. The same spelling in `crates/connector-cli/src/scaffold.rs` was a real defect, not a
  test artefact: `declares_services` would have read a tagged single-surface provider as declaring a
  *named* service and emitted a blocked note naming one that does not exist.

## Notes

- **The misread to design against**: a UI filtering by tag invites "this category means these
  capabilities". It does not. Keep tags and roles visually and semantically distinct wherever both
  render — the explorer already has `SpecChip`, whose tone is *derived from the value*, and a tag
  should not borrow a tone that reads as a safety claim.
- Coordinate with [C-121](C-121-llm-catalogue-role.md): both add a field to `Service` and both touch
  the same loader, so they collide. Whichever lands second consumes the other's derivation helper
  rather than writing a second union. **The same applies to any story replacing `roles` with a
  declared-conformance field** — it rewrites `Service` and `validate_service_roles` wholesale, so it
  must not share a wave with this one. Per `AGENTS.md` §Dispatching, two stories that write the same
  file never go out together; predict the write set from the Acceptance.
- A service may carry several tags. `google`'s `drive` is plausibly both office and storage; do not
  force a single-tag model to keep the filter simple.
- Do not tag *operations*. The request is about surfaces, and an operation already carries risk,
  idempotency and effects — a fourth axis there would be noise. If per-operation categories are wanted
  later, that is a separate decision with its own consumer.
