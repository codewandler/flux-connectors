---
id: C-77
title: Ship the Sentry connector
pillar: Spec
status: ready
priority: 7
design: docs/designs/provider-operation-inventory.md
epic: provider-fleet
areas: [providers, connector-spec]
note: bearer · trailing slashes are load-bearing
---

# Ship the Sentry connector

## Goal
Ship error tracking, so an agent can read and triage an issue it was told about instead of
being handed a URL.

## Acceptance
- [ ] `providers/sentry.toml` is hand-authored, following the `providers/zendesk.toml` precedent: the
      header comment names the upstream document, says why it is not a `[spec]` pointer yet (C-4's
      ingest is unimplemented), and lists the operation set as the selection to reproduce.
- [ ] `base_url = "https://sentry.io"`, `vendor = "Sentry"`, and a `[[auth]]` entry with `scheme = "bearer"` over `SENTRY_AUTH_TOKEN`, named by `default_auth`.
- [ ] A curated set of roughly four over `/api/0`: issue get, issue update (resolve/ignore),
      project get, issue events latest — path-addressed by organization, project and issue id.
- [ ] **No operation declares a string-ish or `Any`-typed query parameter**, tested in the strong form
      (zero query parameters of any type, on the IR and on the emitted URL). C-30 is unimplemented, so
      the emitter still emits such a value unencoded — the defect behind `zendesk-ticket-search`. Any
      operation needing one is excluded and named in Notes.
- [ ] **No optional request-body field is declared** until C-56 lands: an omitted optional body field
      travels as an explicit `null`, which several vendors reject. Declare required fields, and name in
      Notes what was left out for this reason.
- [ ] `cargo run -p connector-cli -- build` emits `connectors/sentry.flux` and
      `connectors/sentry.connector.toml`, both committed, and a second build is byte-identical.
- [ ] The generated module passes the same parse-and-analyze and formatter fixed-point gates as every
      shipped provider, and the provider is picked up by the shared per-provider tests.
- [ ] `http_hosts` derives from the base URL and is never widened to `*`; no credential value appears
      in any generated artifact, the manifest, or the public catalogue. A test asserts it.
- [ ] Every write operation's `risk` reflects what it changes and who sees it; no write is marked
      idempotent unless the vendor documents it as such.
- [ ] **The trailing slash is preserved exactly.** Sentry's paths end in `/`, and dropping it
      redirects or 404s depending on the endpoint. A test pins the emitted URL for every operation, since
      this is the kind of detail a later "tidy up" silently breaks.
- [ ] Issue update is a write that changes triage state a team relies on; its `risk` says so and it is
      not idempotent unless the vendor documents the status transition as such.

## Progress
- Not started. Filed 2026-07-30 in the ten-provider fleet push.

## Notes
- Self-hosted Sentry has a different host, which is [C-68](C-68-endpoint-binding.md)'s subject; this
  story targets `sentry.io` and records the limitation rather than inventing a binding.
- Deliberately excluded pending C-30: the issues *list* with its `query` parameter, which is Sentry's
  own search syntax and therefore the most injectable value it exposes.
- **Still cannot make a live call**, for the same reason as every shipped connector: flux's
  `http.request` takes `{"$secret": …}` as a whole-value replacement, so no scheme prefix is produced
  (`docs/designs/auth-seam.md`). This connector is the catalogue's proof, not a working client.
