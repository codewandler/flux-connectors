---
id: C-25
title: Vendor the babelforce spec without publishing credentials
pillar: Spec
status: done
design: docs/designs/provider-operation-inventory.md
epic: connectors-v1
areas: [providers]
note: "DONE through C-415 — five scrubbed documents, a deterministic allowlist-shaped vendoring script, per-document provenance, and tests that refuse credential-shaped or identifying examples"
---

# Vendor the babelforce spec without publishing credentials

## Goal
Get a hermetic, committed copy of the babelforce OpenAPI document into `specs/` without publishing
the credential literals its upstream embeds.

## Acceptance
- [x] The credential question is settled for this public repository: live-shaped values are removed
      regardless of whether they were rotated. Confirmation with the API owners remains worthwhile,
      but C-415 records why it is not a publication gate.
- [x] A vendoring policy is chosen and written down. The recommendation from
      [the inventory](../designs/provider-operation-inventory.md) §1.3 is a **declared, reproducible
      scrub** recording **both** hashes — upstream and scrubbed — so drift detection stays honest
      while secrets stay out of a public repo.
- [x] The scrub is deterministic and re-runnable: the same upstream file always produces the same
      scrubbed output, and the transformation is described precisely enough to audit.
- [x] `specs/babelforce/` holds the five scrubbed documents; both identities are recorded in
      `specs/babelforce.provenance.toml`. `connectors.lock` covers the scrubbed documents the provider
      currently ingests; the withheld auth document remains covered by provenance and its tests.
- [x] A check refuses to vendor any spec containing credential-shaped literals without an explicit
      acknowledgement — so the next provider cannot reintroduce this silently.
- [x] No credential literal appears in reachable history. The abandoned `impl/C-18` object was never
      merged or pushed; C-454 audits and prunes the unreachable object before release.

## Progress
- **Done through [C-415](C-415-vendor-babelforce-specs.md).** Split out of C-18, then superseded by
  the five-document vendoring story that implemented the scrub, provenance, policy and tests.
- The earlier blocked state is kept below as history of why the unsanitized bytes were never merged.
- The upstream document embeds a response example with a 32-hex `accessToken`, a 64-hex stream
  token, an account UUID, and a real `@babelforce.com` address, for a `Testers Inc.` account dated
  2021. Probably dead staging fixtures — but that is an assumption, and it is exactly the credential
  type babelforce is retiring.
- A byte-identical copy existed only in abandoned commit `54ef636`. It is not an ancestor of `main`,
  no branch or tag contains it, and C-454 removes it from the local object database before release.
- Upstream sha256 is recorded in the inventory doc, so C-25 does not need to re-derive it.

## Notes
- The tension is real and worth stating: hermetic vendoring wants a byte-identical copy with a
  recorded hash, and that is exactly what conflicts with scrubbing. Recording both hashes resolves it
  — the upstream hash still detects drift, the scrubbed hash still pins what we build from.
- This is not a babelforce-specific problem. Vendor specs routinely carry example payloads with
  plausible-looking tokens, so the check in Acceptance is the durable fix and the scrub is the
  immediate one.
- Related: `servers[0]` in the same document is **staging** (`latest.dev.babelforce.com`), not
  production. Positional `servers[0]` ingest would silently target dev — see C-4.
