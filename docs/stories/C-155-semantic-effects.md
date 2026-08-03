---
id: C-155
title: "An operation cannot say it costs money, and all 829 of them claim only `network`"
pillar: Spec
status: done
areas: [connector-spec, connector-flux, bridge, web]
note: "re-measured 2026-08-03: all 829 emitted operations declare only the host effect `network`; this story adds Flux's distinct semantic tier without changing that host effect"
---

# An operation cannot say it costs money, and all 829 of them claim only `network`

## Goal

Let an operation declare what it **means** — this one moves money, this one irreversibly deletes, this
one sends something to a third party — and render that in the explorer so it is visible before a call.

## What was measured

Re-measured on 2026-08-03 after C-30:

```
$ rg -o 'effects \[[^]]*\]' connectors -g '*.flux' | sed 's/^[^:]*://' | sort | uniq -c
    829 effects ["network"]
$ rg -c '^op ' connectors -g '*.flux' | awk -F: '{sum += $2} END {print "operations=" sum}'
operations=829
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

- [x] An operation can declare **semantic effects** from a **closed** set aligned to
      `flux_lang::FlowEffect` — do not invent a parallel vocabulary. An unknown value is refused at load.
- [x] They are **distinct from `effects`**, not merged into it. The host tier answers "what resource
      does this touch"; the semantic tier answers "what does this mean". Collapsing them is what loses
      `Money`, since no host effect distinguishes it.
- [x] **Declared where the truth is, and checked against risk.** Unknown or incoherent declarations
      are refused at load; known money-moving operations are held by a hard whole-catalogue gate.
      The compiler does not infer business meaning from an HTTP verb or a risk grade.
- [x] `connector-pack`'s `Tool::semantic_effects()` returns them, so flux's own signature carries them
      instead of the empty default.
- [x] They reach the manifest and `catalog.json` under the every-key-always-present rule.
- [x] **The explorer renders them, and a money effect is unmistakable.** `SpecChip` already derives its
      tone from the value — extend that vocabulary rather than adding a second chip component.
- [x] Stripe capture and refund declare `money`; capture rises to `destructive` to satisfy Flux's
      money-risk floor. Cancellation declares none because it releases an authorization without
      moving money, and refund does not declare `delete` because it deletes no entity.
- [x] **Failing-first test:** `every_write_that_moves_money_declares_it`, over the shipped catalogue.
- [x] No shipped provider's *host* effects change. Assert it.
- [x] The gate is green; the build stays a fixed point.

## Completion evidence

Recorded on 2026-08-03:

```text
$ cargo test --workspace --no-fail-fast -q
exit 0
$ cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile
$ cargo fmt --all --check
exit 0
$ cargo run -p connector-cli -- diff
1102 artifacts up to date (55 providers checked)
$ cd web && npm run build && npm test
44 passed; 0 failed
```

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
