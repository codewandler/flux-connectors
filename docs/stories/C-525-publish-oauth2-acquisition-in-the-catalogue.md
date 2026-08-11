---
id: C-525
title: "The published catalogue carries a credential's OAuth2 acquisition"
pillar: Bridge
status: done
priority: 0
design: docs/designs/unified-auth.md
epic: connector-config
areas: [catalog, connector-cli, connector-spec, tests]
note: "OAuth2Spec is modelled in the IR and reaches the explorer's catalog.json and the emitted manifest, but NOT crates/catalog — the one artifact Exchange and autodev link. Declaring an OAuth2 connector before this ships a marking no host can read"
---

# The published catalogue carries a credential's OAuth2 acquisition

## Goal

Make an `[auth.oauth2]` declaration readable by the hosts that consume this repository's published
Rust catalogue, so that a connector declaring an OAuth2 grant is a fact a host can act on rather
than a fact only a website displays.

Measured before filing, in this session:

- **Zero of 55 providers declare an `OAuth2Spec`**, and zero `[[config]]` fields bind
  `oauth.client_id` or `oauth.client_secret`. The IR models the whole surface
  (`crates/connector-spec/src/auth.rs:174`), and the loader already refuses an `oauth.client_id`
  binding whose credential declares no `[auth.oauth2]` block
  (`crates/connector-spec/src/provider.rs:2870`). Nothing exercises any of it.
- **The emitted manifest already carries it.** `crates/connector-cli/src/seam.rs:502` serializes
  `Vec<&AuthMethod>` wholesale and `AuthMethod::oauth2` is `skip_serializing_if = "Option::is_none"`,
  so `connectors/<id>.connector.toml` gains the block with no emitter change.
- **`web/public/catalog.json` already carries it** — `crates/connector-cli/src/site.rs:447` has an
  `OAuth2Entry` and `site.rs:966` fills it.
- **`crates/catalog` does not, and cannot.** `catalog::Credential` is exactly `name`, `leaf`,
  `acquire`, `place` (`crates/catalog/src/lib.rs:411`), and `Acquisition`
  (`lib.rs:338`) has three variants — `Static`, `Minted`, `BasicJoin`. There is no
  representation for an authorize endpoint, a token endpoint, a scope or a grant.

That last gap is the one that matters, because `crates/catalog` is what the hosts link:
flux-exchange depends on `codewandler-connector-catalog` 0.18 and autodev on 0.20, and neither
depends on `connector-spec`. AGENTS.md's own rule decides the ordering — *"A marking flux does not
read is worse than none: it reads as safety while changing nothing."* Declaring `[auth.oauth2]` on
`github.toml` before this lands would publish exactly such a marking.

## Acceptance

- [x] `Acquisition` gains an `OAuth2` variant carrying a `&'static OAuth2`, rather than `Credential`
      gaining a field. This is the axis the fact belongs on — `Acquisition` is documented as "how
      stored material becomes the value that is placed" — and it follows `Minted`'s stated
      precedent: a variant costs nothing until something uses it, whereas a field on `Credential`
      rewrites every generated table for a fact no connector declares yet.
- [x] `catalog::OAuth2`, `catalog::OAuthGrant` and `catalog::OAuthRedirect` mirror the IR's
      `OAuth2Spec`/`OAuthGrant`/`OAuthRedirect` field for field, in `&'static` form. The crate keeps
      **zero runtime dependencies**: no `String`, no `serde`, no allocation.
- [x] The types carry no credential value and no field one could occupy. `client_id` is public by
      specification and is the only literal; the client *secret* remains a `[[config]]` binding
      resolved from the store, never a catalogue field.
- [x] `crates/connector-cli/src/catalog.rs`'s `acquisition()` emits the variant, and the emission is
      const-promotable in a `static` initializer.
- [x] **`OAuth2` takes precedence over `Minted`, and declaring both is refused at the loader.** The
      two are contradictory dispositions of the same credential — one says the host runs a grant, the
      other says one of this connector's own operations mints it — and silently preferring either is
      how a host comes to run the wrong acquisition. The refusal names both declarations, following
      `validate_one_credential_disposition`'s precedent.
- [x] Failing-first tests cover: the variant reaching a generated table, round-tripping the full
      field set, the loader's refusal of the contradictory pair, and the catalogue's continued
      zero-dependency status.
- [x] The full connector fixed point is unchanged for every shipped connector — no provider declares
      `[auth.oauth2]` in this story, so `build` and `diff` must still report 1102 artifacts up to
      date and `connectors.lock` must not move.

## Progress

- 2026-08-11: Filed and implemented. `Acquisition::OAuth2` lands with its three supporting types,
  the emitter fills it, and the loader refuses the contradictory pair. No provider declares the
  block yet — that is [C-526](C-526-declare-oauth2-on-the-forge-connectors.md), deliberately
  separate so this story's diff leaves every generated artifact byte-identical.

## Notes

- **This is a breaking change to a published crate**, and deliberately so. `Acquisition` is not
  `#[non_exhaustive]` — the crate documents that choice for `Placement` and inherits it here — so an
  exhaustive `match` in a consumer stops compiling. That is the same break `Minted` made, it is
  what a pre-1.0 minor bump is for, and it is preferable to `#[non_exhaustive]`, which would
  permanently cost every consumer the ability to construct one in a test.
- The host still performs the grant. Nothing here emits an authorize or token call as an operation:
  AGENTS.md's *"an authentication endpoint is never a connector operation"* is untouched, and this
  story adds no operation to any connector.
