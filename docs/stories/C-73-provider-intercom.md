---
id: C-73
title: Ship the Intercom connector
pillar: Spec
status: ready
priority: 7
design: docs/designs/provider-operation-inventory.md
epic: provider-fleet
areas: [providers, connector-spec]
note: bearer · charter-named · version header is optional
---

# Ship the Intercom connector

## Goal
Ship a connector for a vendor `AGENTS.md` already names in the charter: contacts and
conversations, so support automation can read and reply without a hand-written plugin.

## Acceptance
- [ ] `providers/intercom.toml` is hand-authored, following the `providers/zendesk.toml` precedent: the
      header comment names the upstream document, says why it is not a `[spec]` pointer yet (C-4's
      ingest is unimplemented), and lists the operation set as the selection to reproduce.
- [ ] `base_url = "https://api.intercom.io"`, `vendor = "Intercom"`, and a `[[auth]]` entry with `scheme = "bearer"` over `INTERCOM_ACCESS_TOKEN`, named by `default_auth`.
- [ ] A curated set of roughly five, path-addressed: contact get, contact create, conversation
      get, conversation reply, note create.
- [ ] **No operation declares a string-ish or `Any`-typed query parameter**, tested in the strong form
      (zero query parameters of any type, on the IR and on the emitted URL). C-30 is unimplemented, so
      the emitter still emits such a value unencoded — the defect behind `zendesk-ticket-search`. Any
      operation needing one is excluded and named in Notes.
- [ ] **No optional request-body field is declared** until C-56 lands: an omitted optional body field
      travels as an explicit `null`, which several vendors reject. Declare required fields, and name in
      Notes what was left out for this reason.
- [ ] `cargo run -p connector-cli -- build` emits `connectors/intercom.flux` and
      `connectors/intercom.connector.toml`, both committed, and a second build is byte-identical.
- [ ] The generated module passes the same parse-and-analyze and formatter fixed-point gates as every
      shipped provider, and the provider is picked up by the shared per-provider tests.
- [ ] `http_hosts` derives from the base URL and is never widened to `*`; no credential value appears
      in any generated artifact, the manifest, or the public catalogue. A test asserts it.
- [ ] Every write operation's `risk` reflects what it changes and who sees it; no write is marked
      idempotent unless the vendor documents it as such.
- [ ] **The `Intercom-Version` header is optional and the story says what that costs.** Intercom
      defaults to the workspace's pinned version when the header is absent, so the connector is
      well-formed without it — but the version cannot be pinned until
      [C-55](C-55-constant-request-headers.md) lands. Record it as a known limitation, not as a gap
      that blocks the connector.

## Progress
- Not started. Filed 2026-07-30 in the ten-provider fleet push.

## Notes
- Contrast worth recording for whoever picks up C-55: Intercom *defaults* its version, so omission
  is survivable — Notion and Anthropic **require** their version header and were therefore excluded
  from this fleet entirely.
- Deliberately excluded pending C-30: conversation search and every `per_page`/`starting_after` page.
- **Still cannot make a live call**, for the same reason as every shipped connector: flux's
  `http.request` takes `{"$secret": …}` as a whole-value replacement, so no scheme prefix is produced
  (`docs/designs/auth-seam.md`). This connector is the catalogue's proof, not a working client.
