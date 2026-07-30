---
id: C-155
title: "An operation cannot say it costs money, and all 110 of them claim only `network`"
pillar: Spec
status: ready
priority: 2
areas: [connector-spec, connector-flux, bridge, web]
note: "measured: every one of 110 emitted operations declares `effects [\"network\"]` — including Stripe's refund, which is risk `destructive`. flux has a semantic tier (Money/Delete/SendExternal) and built OpSignature::semantic_effects so 'a downstream visual editor' could see it"
---

# An operation cannot say it costs money, and all 110 of them claim only `network`

## Goal

Let an operation declare what it **means** — this one moves money, this one irreversibly deletes, this
one sends something to a third party — and render that in the explorer so it is visible before a call.

## What was measured

```
$ grep -ho 'effects \[.*\]' connectors/*.flux | sort | uniq -c
    110 effects ["network"]
```

Every emitted operation, without exception. Including:

```
op stripe-charge-refund-create …
  risk "destructive"
  effects ["network"]
```

A refund that moves real money declares the same effect as reading a balance. `risk` carries the
signal; `effects` does not.

## flux already has the tier, and built it for this consumer

Two vocabularies exist there, and this repo only reaches the first:

- **`flux_spec::Effect`** — host *resources*: `Read`, `Write`, `Network`, `Process`, `Browser`,
  `Filesystem`, `LocalSystem`. This is what `effects [...]` emits today.
- **`flux_lang::FlowEffect`** — *semantic*: `Money`, `Delete`, `SendExternal`. From
  `crates/flux-lang/src/opspec.rs:97-103`:

  > `Self::lower`'s `ToolSpec` can only carry the host-resource effects `FlowEffect::lower` projects
  > (**`Money` vanishes entirely**, `Delete`/`SendExternal` collapse into `Write`/`Network`), because a
  > `ToolSpec` has no room for a semantic-effect field. This method … additionally copies the ORIGINAL,
  > undegraded effects onto `OpSignature::semantic_effects` — so a consumer reading the signature (the
  > SDK catalog, **a downstream visual editor**, `annotate_effects`) can see `Money`/`Delete`/
  > `SendExternal` even though no host `Effect` distinguishes them.

So flux erases `Money` when it lowers to a `ToolSpec`, and deliberately preserves it on the signature
**for exactly the consumer this story is about**. `Tool::semantic_effects()` exists for it and
`connector-pack` returns the trait's empty default.

## Acceptance

- [ ] An operation can declare **semantic effects** from a **closed** set aligned to
      `flux_lang::FlowEffect` — do not invent a parallel vocabulary. An unknown value is refused at load.
- [ ] They are **distinct from `effects`**, not merged into it. The host tier answers "what resource
      does this touch"; the semantic tier answers "what does this mean". Collapsing them is what loses
      `Money`, since no host effect distinguishes it.
- [ ] **Declared where the truth is, and checked against risk.** A `destructive` write that moves money
      should not be able to omit `money`. Decide whether that is a refusal or a lint, and record why —
      a refusal is stronger but may not hold for every vendor.
- [ ] `connector-pack`'s `Tool::semantic_effects()` returns them, so flux's own signature carries them
      instead of the empty default.
- [ ] They reach the manifest and `catalog.json` under the every-key-always-present rule.
- [ ] **The explorer renders them, and a money effect is unmistakable.** `SpecChip` already derives its
      tone from the value — extend that vocabulary rather than adding a second chip component.
- [ ] Stripe's three writes declare `money`; its refund declares `money` **and** `delete` if that is
      what a refund is. Get that right rather than plausible — it is the worked example.
- [ ] **Failing-first test:** `every_write_that_moves_money_declares_it`, over the shipped catalogue.
- [ ] No shipped provider's *host* effects change. Assert it.
- [ ] The gate is green; the build stays a fixed point.

## Notes

- **This is not cosmetic.** `Tool::semantic_effects()` feeds flux's approval and policy layer — its own
  doc says "policy decides allow / deny / require-approval" on `FlowEffect`. An operation that cannot
  say it costs money cannot be gated on costing money.
- Read `crates/connector-flux/tests/stripe_connector.rs` first: C-106 already graded Stripe by what it
  does to money and recorded the reasoning. The declaration this story adds is the machine-readable
  half of a decision already made in prose.
- **The explorer half matters and is the visible one.** A chip tone is a claim: `SpecChip` deliberately
  keeps an unrecognised value **neutral** rather than guessing, because a wrong colour on a safety field
  reads as an assurance nobody made. A `money` chip must be alarming; a `send_external` chip probably
  cautionary. Do not let a *tag* ([C-153](C-153-service-tags.md)) borrow a semantic-effect tone.
- Related and separate: `Risk` has no axis for what a *read discloses* — `stripe-customer-get` returns a
  named individual's email, phone and billing address and grades `low`, the same as reading a balance.
  `shopify.toml` records the same gap. That is a third axis, not this one.
