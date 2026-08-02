---
id: C-185
title: "A request body cannot contain an array, so an envelope-shaped vendor cannot be addressed"
pillar: Codegen
status: blocked
priority: 2
areas: [connector-flux, connector-spec]
note: "found by C-168, then NARROWED by C-179: a flat single-level array DOES work (front's tag_ids emits as List<String>). What is blocked is an array a wire path must DECOMPOSE across nested segments, which is what SendGrid's personalizations[].to[] needs"
---

# A request body cannot contain an array, so an envelope-shaped vendor cannot be addressed

## Goal

Let an operation declare a request body containing an array, so a vendor whose write surface is an
envelope can be addressed at all.

## What was measured

[C-168](C-168-provider-sendgrid.md) established this rather than assuming it. `BodyNode`
(`crates/connector-flux/src/op.rs`) composes a nested **object** from a dotted `wire` path.

**The original framing of this story was too broad, and C-179 corrected it by reading `body_tree`
rather than trusting the summary.** A **flat, single-level array is already expressible** — Front's
`tag_ids` emits as `List<String>` in its shipped module. What is blocked is an array that a `wire` path
must **decompose across nested segments**. That is a narrower claim and it is the accurate one.

SendGrid's `POST /v3/mail/send` requires:

```json
{ "personalizations": [ { "to": [ { "email": "…" } ] } ], "content": [ { "type": "…", "value": "…" } ] }
```

Arrays of objects containing arrays of objects. SendGrid does not accept the bare-object form. So the
operation was **excluded**, and this catalogue now ships an email provider that cannot send email.

The one mechanically-legal workaround — a single array-typed body-root parameter — was rejected on
`providers/notion.toml`'s precedent: it decomposes nothing, and it dresses an unassisted guess at the
one shape the vendor is strictest about as a typed field.

## Why this is bigger than SendGrid

An envelope is the normal shape for a bulk or batch write. Already in or near the fleet:

- **SendGrid** — `personalizations[]`, `content[]` (excluded by C-168).
- **Postmark** ([C-180](C-180-provider-postmark.md)) — batch send is an array at the body root.
- **Algolia** ([C-164](C-164-provider-algolia.md)) — `requests[]` for batched index operations.
- **Cloudflare** ([C-169](C-169-provider-cloudflare.md)) — cache purge takes `files[]`, a *flat* array
  of strings, so per C-179's correction this one is **not** blocked and was wrongly listed here.
- **Miro** ([C-183](C-183-provider-miro.md)) and **Webflow** ([C-182](C-182-provider-webflow.md)) — bulk item writes.

Each will hit this independently and each will have to decide again whether to exclude the operation.
That is the argument for fixing the mechanism rather than recording the gap five more times.

## Acceptance

- [ ] A body field can declare an array, including an array **of objects**, and including an array
      nested inside an object. Decide how deep the spelling goes and **record what it refuses** — an
      unbounded recursive body model is how a connector ends up shipping an untyped blob, which is the
      outcome C-107 and C-168 both refused.
- [ ] **A fixed-length envelope and a caller-supplied list are different problems. Say which you
      solved.** SendGrid's `personalizations` is usually a one-element array wrapping real fields;
      Cloudflare's `files[]` is a genuine list of caller values. Solving only the first is a legitimate
      scope and would unblock SendGrid; pretending it solved the second would not.
- [ ] **Failing-first test:** a provider declaring an array body field does not load or does not emit
      today.
- [ ] Every existing operation's emitted module is **byte-identical** — 27 providers, so no object body
      may change shape.
- [ ] `sendgrid-mail-send` ships, or this story records why it still cannot. It is the concrete case
      that motivated the work and the natural proof.
- [ ] Generated Flux still parses, analyzes and is a fixed point of flux's formatter, and the build
      stays a fixed point.

## Notes

- **Check what `flux_lang::ast` can express before designing the TOML surface.** An array literal
  containing interpolated values may or may not be constructible the way `fmt` builds a string today;
  that constraint should shape the spelling rather than be discovered after it.
- Coordinate with [C-144](C-144-request-body-encoding.md): a `form` body **refuses** nesting outright,
  so arrays must stay refused there. Whatever lands must not accidentally make `form` accept one.
- **Do not confuse this with [C-56](C-56-optional-body-fields.md).** C-179 hit an adjacent wall and
  cited the right story: Front's optional `to`/`cc`/`bcc` are flat arrays this pipeline *can* build, but
  an optional body field cannot be omitted without sending an explicit `null`. That is C-56's problem,
  not this one, and conflating them will send an implementor down the wrong path.
- Optional fields inside an array element are the sharp edge — `when` guards work for a flat body, and
  an omitted field inside an array element cannot leave a hole. Say what happens.
- This runs solo: it changes body lowering, which every provider reads.
