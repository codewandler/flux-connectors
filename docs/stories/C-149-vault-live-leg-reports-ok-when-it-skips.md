---
id: C-149
title: "The Vault live leg reports ok when it skips, and three smaller gaps beside it"
pillar: Core
status: ready
priority: 3
areas: [connector-secrets]
note: "found by C-91's review. The test's own module doc says 'there is no third path where it reports success without having talked to anything' — and that is exactly what it does today. A skipped leg that prints ok is the failure mode the whole no-simulated-success rule exists to prevent"
---

# The Vault live leg reports ok when it skips, and three smaller gaps beside it

## Goal

Close the four gaps an independent review of [C-91](C-91-connector-secrets-crate.md) found. The first
is the one that matters; the rest are tidying.

## Acceptance

- [ ] **A skipped live leg is visibly skipped.** `crates/connector-secrets/tests/vault_live.rs:38`
      reports `ok` / `1 passed` when no Vault is offered, and the skip reason is captured and
      invisible. Its own module doc (`:20-22`) claims *"There is no third path where it reports
      success without having talked to anything"* — which is precisely what it does.

      A reader of a green run cannot tell whether the HTTP transport was exercised. Make the skip
      unmistakable in the output, or make the leg absent rather than passing. **Failing-first test or
      a demonstrated run** showing the skip is visible without a Vault.

- [ ] **`a_kv_v1_response_is_named_rather_than_read_as_missing` claims more than it proves.**
      (`crates/connector-secrets/src/vault.rs:697`) It feeds a flat `200 {"data":{…}}` to the KV v2
      URL. A *real* KV v1 mount given that URL reads the literal key `data/<path>` and answers 404,
      which this store maps to `NotFound` — not to the "is this mount KV v1?" message the test's name
      implies. The branch is worth having; rename it to what it actually covers, or extend it to the
      real 404 case.

- [ ] **`StoreError::Layout` is never constructed** (`crates/connector-secrets/src/lib.rs:200`) and
      neither store calls `Layout::parse`, so a custom layout's refusal has no path to a caller.
      Either wire it up or remove the variant — a typed error nothing can raise is a promise that
      does not hold.

- [ ] **`Secret::into_inner` is a second exit from the wrapper**
      (`crates/connector-secrets/src/secret.rs:56`) under a name the doc's "one search for
      `expose_secret`" audit story (`:29`) does not cover. Either fold it into the same audited name
      or amend the doc so the audit instruction is true.

- [ ] The gate is green, including `cargo test -p connector-secrets --features vault`.

## Notes

- **The first item is not cosmetic.** "A leg that needs a live service skips honestly and says so —
  never simulated success" was an explicit instruction in C-91's dispatch, and this is the one place
  it was not met. It is also the exact class of defect this repository has been finding all week: a
  test that is green while asserting nothing (the `$sep` sigil checks), a guard that locked in a
  breakage (the site's base path), a coverage count taken with a runner that stops early. A skipped
  test printing `ok` belongs to that family.
- The review also recorded honestly that `HttpTransport` is **unexercised by any run** — the header
  name, body write and status read are covered by code reading only. `vault server -dev` plus
  `CONNECTOR_SECRETS_VAULT_ADDR`/`_TOKEN` would settle it. That is not a defect, but it is the reason
  item 1 matters: right now nothing tells you which of those two situations you are in.
- The recorded transcript is a scripted in-process `VaultTransport`, not a captured wire dump. It
  proves the store's URL construction, envelope parsing and status mapping — **nothing about Vault**.
  That is a fair scope and is stated; do not let a later change quietly present it as vendor evidence.
