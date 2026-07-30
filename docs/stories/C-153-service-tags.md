---
id: C-153
title: "Tag a service with what kind of thing it is, so a catalogue can be filtered"
pillar: Spec
status: ready
priority: 3
design: docs/designs/provider-roles.md
epic: provider-roles
areas: [connector-spec, codegen]
note: "a tag is NOT a role — it carries no required members and is never evidence anything is callable. Gmail is office, YouTube is social; 19 providers and 22 services and no way to filter them"
---

# Tag a service with what kind of thing it is, so a catalogue can be filtered

## Goal

Let a service declare what **kind** of thing it is — `gmail` is office, `youtube` is social — so a
catalogue of 19 providers can be filtered by category rather than scrolled.

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

- [ ] A **service** declares `tags = [...]`, beside `roles`, in the IR and the loader. `Service` gains
      the field with `skip_serializing_if` so a service declaring none hashes exactly as before.
- [ ] A provider's tags are **derived** as the union of its services', never authored — the same rule
      `roles` and `Level` already follow.
- [ ] **A closed vocabulary.** An unknown tag is refused at load, naming the known set. A typo'd tag
      silently means "absent from that filter", which is the same shape of silent-nothing failure a
      typo'd role would be — cheaper to be wrong about, but not free.
- [ ] **The vocabulary is derived from what actually ships**, not invented ahead of need. The 19
      shipped providers cluster naturally; read them and propose the set that covers them, rather than
      a taxonomy nothing populates. Record the clustering in the design.
- [ ] `gmail` is tagged office and `youtube` social — the two the request names. YouTube is a **new
      service** and is [C-154](C-154-google-youtube-service.md); if it has not landed, tag `gmail`,
      `calendar` and `drive` and say so.
- [ ] Tags reach the manifest and `catalog.json` under the every-key-always-present rule, and
      `catalog` gains a way to ask which services carry a tag.
- [ ] The explorer can **filter by tag**. That is the whole point — a tag no consumer uses is a
      field, not a feature.
- [ ] **Failing-first test:** `an_unknown_tag_is_refused_and_names_the_known_set`.
- [ ] A test asserts the tag set is **non-empty and multi-valued** across shipped services, so the
      filter cannot pass vacuously.
- [ ] No shipped provider's emitted module changes — a tag is catalogue metadata and reaches no
      `.flux`. Assert it.
- [ ] The gate is green; the build stays a fixed point.

## Notes

- **The misread to design against**: a UI filtering by tag invites "this category means these
  capabilities". It does not. Keep tags and roles visually and semantically distinct wherever both
  render — the explorer already has `SpecChip`, whose tone is *derived from the value*, and a tag
  should not borrow a tone that reads as a safety claim.
- Coordinate with [C-121](C-121-llm-catalogue-role.md): both add a field to `Service` and both touch
  the same loader, so they collide. Whichever lands second consumes the other's derivation helper
  rather than writing a second union.
- A service may carry several tags. `google`'s `drive` is plausibly both office and storage; do not
  force a single-tag model to keep the filter simple.
- Do not tag *operations*. The request is about surfaces, and an operation already carries risk,
  idempotency and effects — a fourth axis there would be noise. If per-operation categories are wanted
  later, that is a separate decision with its own consumer.
