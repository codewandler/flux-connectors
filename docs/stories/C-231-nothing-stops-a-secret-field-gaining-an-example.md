---
id: C-231
title: "Nothing catalogue-wide stops a secret configuration field gaining an `example`, and that is the exact shape that has blocked a release here before"
pillar: Build
status: ready
priority: 1
design:
epic:
areas: [build, connector-spec]
note: "measured in review 2026-07-31: adding `example = \"NRAK-ABCDEFG\"` to newrelic's secret api_key turned NO test red. C-219 wrote no_secret_config_field_carries_an_example for itself; C-220 has no analogue and no catalogue-wide rule exists. Every provider story's Acceptance demands this and enforcement is per-connector goodwill"
---

# Nothing catalogue-wide stops a secret field gaining an `example`

## Goal

Make "no realistic-looking example on a secret field" a checked property of the catalogue rather than
a sentence each provider story repeats and each implementor enforces or does not.

## What was measured

During the 2026-07-31 provider wave's review, mutation **N4** added

```toml
example = "NRAK-ABCDEFG"
```

to `providers/newrelic.toml`'s secret `api_key` field. **No test went red.** A grep across
`crates/connector-spec/tests` and `crates/connector-cli/tests` found no catalogue-wide rule either.

The shipped file is correct — `api_key` carries no `example`, with the reason recorded beside it —
but that correctness rests on the implementor having chosen to honour it. C-219, working the same
wave, wrote itself a guard (`no_secret_config_field_carries_an_example`). C-220 did not. Both passed
review, because both shipped files satisfy the bullet; only one of them would keep satisfying it.

## Why this one is priority 1

**This exact shape has blocked a release in this repository.** A token-shaped placeholder tripped
GitHub push protection and stopped a release, which is why the sentence appears in the Acceptance of
every provider story. The wave that just landed added eight connectors; the next will add more, and
the protection against a repeat is currently:

1. the story text saying not to, and
2. whether the implementor happens to write a guard for their own connector.

Neither is a mechanism. The failure is also **asymmetric in cost**: an `example` that merely looks
like a token blocks a push and costs an hour; one that *is* a token is a disclosed credential.

## Acceptance

- [ ] **Failing-first test:** a catalogue-wide check that fails when any `[[config]]` field with
      `secret = true` declares an `example`. It must fail against a tree where N4's mutation is
      applied and pass against the catalogue as it stands. Name it.
- [ ] It reads the **catalogue**, not a list. `providers/` is enumerated the way
      `response_schema_coverage.rs` and `shipped_modules.rs` already do it — a hand-kept list of
      providers-to-check is the same defect one level up, and
      [C-81](C-81-declared-counts-are-checked.md) is the standing example of what hand-maintained
      lists do.
- [ ] Decide whether this belongs as a **loader refusal** in `connector-spec` rather than a test, and
      record the reason. A loader refusal is strictly stronger — it fails at `provider::load` for
      anyone, including a downstream consumer authoring their own provider file — and the loader
      already refuses `secret` disagreeing with `binds`, so the shape is established. The argument
      against is that an `example` is documentation rather than a safety property; make it or reject
      it explicitly.
- [ ] C-219's `no_secret_config_field_carries_an_example` is **removed or reduced** once the
      catalogue-wide rule exists. Leaving both is two spellings of one rule — the defect
      [C-214](C-214-a-pinned-value-reaches-the-wire-unvalidated.md) is an instance of and
      [C-230](C-230-per-provider-tests-hold-catalogue-wide-literals.md) is about.
- [ ] The guard is placed where it does **not** reintroduce C-230's defect: a catalogue-wide
      assertion belongs in a catalogue-wide test, not inside one provider's contract test.

## Notes

- Related but distinct from [C-230](C-230-per-provider-tests-hold-catalogue-wide-literals.md). That
  story is about catalogue-wide assertions living in the *wrong place*; this one is about a
  catalogue-wide assertion that does not exist at all. Implementing this one badly would create an
  instance of that one — hence the last Acceptance bullet.
- Scope discipline: this is about `example` on a **secret** field. Whether a *non-secret* field's
  example is realistic is a documentation question, not a safety one, and is not this story.
- Worth checking while in here, though it is not the deliverable: whether anything stops a
  credential-shaped literal appearing in a `help` string, a `description`, or a provider header
  comment. The push-protection scanner does not care which TOML key the string sits under.
