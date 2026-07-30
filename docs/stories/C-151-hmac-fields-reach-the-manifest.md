---
id: C-151
title: "A verification field that stops at the IR is silently dropped from the manifest and the catalogue"
pillar: Codegen
status: ready
priority: 3
design: docs/designs/inbound-events.md
epic: inbound-events
areas: [connector-cli]
note: "ManifestHmac and HmacEntry enumerate HmacSpec's fields BY HAND, so C-141's timestamp_format never reaches a host. Nothing declares one yet — this must land before the first RFC 3339 vendor binding, or that host reads the wrong spelling"
---

# A verification field that stops at the IR is silently dropped from the manifest and the catalogue

## Goal

Stop the manifest and the public catalogue from silently losing a verification field the IR carries.

## What was measured

[C-141](C-141-hmac-spec-gaps.md) added `HmacSpec::timestamp_format`, because `HmacSpec` said *where* a
timestamp is read from and never *how it is spelled* — Slack and Stripe send unix seconds, Zendesk
sends RFC 3339, and the reference verifier had to sniff.

It reaches the IR, the loader and the published JSON schema. It reaches **neither** consumer:

- `ManifestHmac` — `crates/connector-cli/src/seam.rs:502-514`, built at `:565`
- `HmacEntry` — `crates/connector-cli/src/site.rs:227-245`, built at `:570-580`

Both **enumerate `HmacSpec`'s fields by hand**, so a connector declaring `timestamp_format = "rfc3339"`
loses it in `connectors/<id>.connector.toml` and in `catalog.json`.

That contradicts `AGENTS.md`'s channel-binding contract: *"A channel binding declares … It reaches the
manifest and the catalogue."*

## Why it is filed rather than fixed in place

C-141 could not do it: `seam.rs` and `site.rs` belonged to another story that wave, and adding a
serialized field rewrites committed per-provider artifacts, which a scoped run's "nothing written"
gate forbids. An independent review then confirmed the gap and noted the branch could not file this
story itself with the board fenced — so it is the coordinator's.

**Nothing shipped declares a format**, so nothing is lost today: `providers/slack.toml` is the only
provider with an HMAC binding and it is unix seconds. The drop also fails **closed** — a host
defaulting to `unix_seconds` cannot parse an RFC 3339 value, so it refuses rather than mis-verifying.

**But this must land before the first RFC 3339 vendor binding ships**, or that host reads the wrong
spelling for a signature it is meant to check.

## Acceptance

- [ ] `timestamp_format` reaches `connectors/<id>.connector.toml` and `catalog.json`, under the
      every-key-always-present rule ([catalog-json.md](../designs/catalog-json.md)). Additive — no
      `SCHEMA_VERSION` bump.
- [ ] **The hand-enumeration stops being a place a field can go missing.** Deriving both projections
      from `HmacSpec` rather than restating it is the real fix; if that is not practical, a test must
      fail when `HmacSpec` gains a field neither projection carries. A comment asking the next person
      to remember is not enough — this story exists because that did not work.
- [ ] **Failing-first test:** a shipped binding's declared verification round-trips from
      `providers/*.toml` through the manifest and the catalogue with **every** `HmacSpec` field intact.
      It must fail today, naming `timestamp_format`.
- [ ] The credential is still **named, never valued**, in both projections —
      `no_credential_value_reaches_the_document` stays green with a sentinel set for the signing
      secret.
- [ ] Whole-catalogue artifacts are coordinator-owned: verify with a full build, then hand back the
      red tests rather than committing the regenerated files.

## Notes

- **The general shape is worth naming**: two consumers that restate a type's fields by hand will drift
  from it, and the drift is invisible because both still compile. C-125 hit the same class from a
  different angle — two derivations of one schema — and resolved it with an agreement test rather than
  a comment. Prefer that pattern here.
- Also owed on the flux side: a host must actually *read* the field. C-141's story records that
  `ManifestHmac` is what a host sees, so until this lands a host reading the manifest sees only the
  selector. flux's `verify` block is the counterpart, filed as `C-292` on that board.
- `mod common`'s fixture harness is being fixed under [C-150](C-150-integration-fixture-leak.md); if the
  integration binaries flake while you work here, that is the cause, not your diff.
