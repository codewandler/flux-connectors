---
id: C-149
title: "The Vault live leg reports ok when it skips, and three smaller gaps beside it"
pillar: Core
status: in-progress
priority: 3
areas: [connector-secrets]
note: "found by C-91's review. The test's own module doc says 'there is no third path where it reports success without having talked to anything' — and that is exactly what it does today. A skipped leg that prints ok is the failure mode the whole no-simulated-success rule exists to prevent"
---

# The Vault live leg reports ok when it skips, and three smaller gaps beside it

## Goal

Close the four gaps an independent review of [C-91](C-91-connector-secrets-crate.md) found. The first
is the one that matters; the rest are tidying.

## Acceptance

- [x] **A skipped live leg is visibly skipped.** `crates/connector-secrets/tests/vault_live.rs:38`
      reports `ok` / `1 passed` when no Vault is offered, and the skip reason is captured and
      invisible. Its own module doc (`:20-22`) claims *"There is no third path where it reports
      success without having talked to anything"* — which is precisely what it does.

      A reader of a green run cannot tell whether the HTTP transport was exercised. Make the skip
      unmistakable in the output, or make the leg absent rather than passing. **Failing-first test or
      a demonstrated run** showing the skip is visible without a Vault.

- [x] **`a_kv_v1_response_is_named_rather_than_read_as_missing` claims more than it proves.**
      (`crates/connector-secrets/src/vault.rs:697`) It feeds a flat `200 {"data":{…}}` to the KV v2
      URL. A *real* KV v1 mount given that URL reads the literal key `data/<path>` and answers 404,
      which this store maps to `NotFound` — not to the "is this mount KV v1?" message the test's name
      implies. The branch is worth having; rename it to what it actually covers, or extend it to the
      real 404 case.

- [x] **`StoreError::Layout` is never constructed** (`crates/connector-secrets/src/lib.rs:200`) and
      neither store calls `Layout::parse`, so a custom layout's refusal has no path to a caller.
      Either wire it up or remove the variant — a typed error nothing can raise is a promise that
      does not hold.

- [x] **`Secret::into_inner` is a second exit from the wrapper**
      (`crates/connector-secrets/src/secret.rs:56`) under a name the doc's "one search for
      `expose_secret`" audit story (`:29`) does not cover. Either fold it into the same audited name
      or amend the doc so the audit instruction is true.

- [x] The gate is green, including `cargo test -p connector-secrets --features vault`.

## Progress

Done on `impl/C-149`. The four gaps, and where each one is now closed:

1. **The skip is visible and the leg is not a pass.** The decision moved from *runtime* to *build
   time*, because `#[ignore]` is the only skip libtest reports in its default output and an attribute
   is a compile-time thing. New `crates/connector-secrets/build.rs` reads the two variables and sets
   `cfg(live_vault)`; the leg is `#[cfg_attr(not(live_vault), ignore = "…")]`. A run without a Vault
   now prints `ignored, no live Vault was offered…` on its own line and reports **`0 passed;
   1 ignored`**, and the build script emits a `cargo::warning` naming the transport as UNEXERCISED.
   With the variables set the leg runs by itself — no `--ignored` flag — and if they vanish between
   build and run it *panics* rather than skipping, so the third path is gone rather than relabelled.

   Held by `without_a_vault_the_live_leg_is_skipped_and_never_reported_as_a_pass`, which re-execs the
   test binary filtered to the live leg and asserts over libtest's own report — the claim is about
   what a reader sees, so nothing short of the real output can prove it. It failed against the old
   leg quoting `ok. 1 passed`.

2. `a_kv_v1_response_is_named_rather_than_read_as_missing` →
   `a_two_hundred_without_data_data_is_named_rather_than_read_as_missing`, which is what it feeds, and
   the real v1 case was added beside it as
   `a_real_kv_v1_mount_reads_as_not_found_because_the_data_prefix_is_a_literal_key`. The old name was
   not merely loose: the flat body it feeds is reachable from a v1 mount *only* in the sub-case where
   a literal `data/<path>` key exists there. The ordinary v1 outcome is a 404 → `NotFound`, and a
   migration meets that one.

3. `StoreError::Layout` is now constructible, by `MemoryStore::reference` and
   `VaultStore::reference` — the inverse of `path`, and the only place a `Layout` can refuse, since
   `render` is infallible. Asserted over both layouts in
   `a_layout_refusal_reaches_the_caller_as_a_layout_error`.

4. `Secret::into_inner` → `Secret::expose_secret_owned`, folded into the audited name. The doc's "one
   search" promise is now asserted rather than stated:
   `every_exit_from_the_wrapper_is_named_expose_secret` reads `secret.rs`'s own source and refuses any
   `pub fn` returning `&str`/`String` whose name a `grep expose_secret` would miss. It failed against
   `into_inner` by name.

Also demonstrated, because it had never once been run: the live leg **passes against a real socket**,
with `X-Vault-Token` on all six requests, `Content-Type: application/json` on the POST only, `data/`
for read/write and `metadata/` for delete. Run against a throwaway KV v2 stub, so it is evidence about
`HttpTransport`, **not** about Vault — the transcript's scope is unchanged and still says so.

Untouched deliberately: `Secret` still has no `Serialize`/`Display`/`Deref`/`AsRef`/`Hash` and the
`compile_fail` doctest still pins the first; the `vault` feature is still off by default; no
dependency and no manifest changed.

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
