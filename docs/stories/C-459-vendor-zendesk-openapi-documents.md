---
id: C-459
title: "Vendor Zendesk's first-party OpenAPI documents"
pillar: Build
status: done
priority: 1
design: docs/designs/zendesk-suite.md
epic: zendesk-suite
areas: [specs, connector-spec]
note: "Ticketing and Help Center are public Zendesk downloads; Messaging is Zendesk's Sunshine Conversations spec — pin the bytes, never fetch during build"
---

# Vendor Zendesk's first-party OpenAPI documents

## Goal
Put the three first-party documents the epic will compile under `specs/zendesk/`, with reproducible
scrubbing and enough provenance to detect drift without making a build reach the network.

## Acceptance
- [x] Date/version-pinned Ticketing, Help Center, and Messaging documents are committed under
      `specs/zendesk/`; the old excerpt remains only while a named test still uses it.
- [x] A repository script fetches or accepts the official source bytes, removes example values that
      are credential-shaped or identify a person/system, preserves declarations, and reproduces the
      committed documents byte-for-byte.
- [x] Per-document provenance records the public source URL, upstream version, fetch timestamp,
      upstream SHA-256, and vendored SHA-256; tests recompute the latter.
- [x] Tests fail on an undeclared document, a hash mismatch, a personal/credential-shaped example,
      a non-public host introduced by the scrub, or loss of `securitySchemes` declarations.
- [x] `connector-cli build`, `diff`, and `check` remain hermetic and do not invoke the vendor script.

## Progress

- Failing first: `cargo test -p codewandler-connector-spec --test vendored_zendesk_specs` failed all
  five gates because `specs/zendesk.provenance.toml` and its declared documents did not exist.
- Fetched on 2026-08-02 at `08:47:26Z`: Ticketing and Help Center from Zendesk's downloadable
  endpoints, and Messaging from commit `a42f7055d829b67ef5c1d7c0f3e2c48cdddd026d` of Zendesk's
  `sunshine-conversations-api-spec` repository. The upstream SHA-256 values travel in
  `specs/zendesk.provenance.toml`; the script records no unverified secondary measurement.
- Re-running `scripts/vendor-zendesk-specs.sh --source-dir /tmp/zendesk-upstream --fetched-at
  2026-08-02T08:47:26Z` and checking the four pre-run SHA-256 values reported all three documents
  and the provenance file `OK`.
- Focused proof: `cargo test -p codewandler-connector-spec --test vendored_zendesk_specs` — 5 passed;
  `cargo clippy -p codewandler-connector-spec --test vendored_zendesk_specs -- -D warnings` — green.
- Hermetic proof: `cargo test -p connector-cli --test no_network --no-fail-fast` — 3 passed;
  `cargo run -q -p connector-cli -- diff --provider zendesk` —
  `10 artifacts up to date (1 provider checked)`. `rg -n 'vendor-zendesk-specs'` over the three
  compiler crates returned no reference to the opt-in script.

## Notes
- C-14 owns the generic re-fetch/drift command. This story provides its pinned inputs.
