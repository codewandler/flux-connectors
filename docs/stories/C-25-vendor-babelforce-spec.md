---
id: C-25
title: Vendor the babelforce spec without publishing credentials
pillar: Spec
status: blocked
design: docs/designs/provider-operation-inventory.md
epic: connectors-v1
areas: [providers]
note: **blocked on a human decision** · upstream spec embeds live-shaped credentials
---

# Vendor the babelforce spec without publishing credentials

## Goal
Get a hermetic, committed copy of the babelforce OpenAPI document into `specs/` without publishing
the credential literals its upstream embeds.

## Acceptance
- [ ] The credential question is settled with the babelforce API owners: are the `Testers Inc.`
      values dead fixtures, and have they been rotated?
- [ ] A vendoring policy is chosen and written down. The recommendation from
      [the inventory](../designs/provider-operation-inventory.md) §1.3 is a **declared, reproducible
      scrub** recording **both** hashes — upstream and scrubbed — so drift detection stays honest
      while secrets stay out of a public repo.
- [ ] The scrub is deterministic and re-runnable: the same upstream file always produces the same
      scrubbed output, and the transformation is described precisely enough to audit.
- [ ] `specs/babelforce/` holds the scrubbed document; `connectors.lock` (C-7) records both hashes.
- [ ] A check refuses to vendor any spec containing credential-shaped literals without an explicit
      acknowledgement — so the next provider cannot reintroduce this silently.
- [ ] No credential literal appears anywhere in this repo's history.

## Progress
- **Blocked.** Split out of C-18, whose inventory half is done and merged.
- The upstream document embeds a response example with a 32-hex `accessToken`, a 64-hex stream
  token, an account UUID, and a real `@babelforce.com` address, for a `Testers Inc.` account dated
  2021. Probably dead staging fixtures — but that is an assumption, and it is exactly the credential
  type babelforce is retiring.
- A byte-identical copy exists on `impl/C-18` (commit `54ef636`). **That branch must not be merged or
  pushed as it stands.** Nothing has been pushed anywhere; the repo has no remote.
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
