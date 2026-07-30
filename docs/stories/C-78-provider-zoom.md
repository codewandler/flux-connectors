---
id: C-78
title: Ship the Zoom connector
pillar: Spec
status: ready
priority: 7
design: docs/designs/provider-operation-inventory.md
epic: provider-fleet
areas: [providers, connector-spec]
note: bearer · nested meeting settings
---

# Ship the Zoom connector

## Goal
Ship scheduling: create, read and delete a meeting, so an agent can arrange a call as a typed
operation rather than through a vendor console.

## Acceptance
- [ ] `providers/zoom.toml` is hand-authored, following the `providers/zendesk.toml` precedent: the
      header comment names the upstream document, says why it is not a `[spec]` pointer yet (C-4's
      ingest is unimplemented), and lists the operation set as the selection to reproduce.
- [ ] `base_url = "https://api.zoom.us"`, `vendor = "Zoom"`, and a `[[auth]]` entry with `scheme = "bearer"` over `ZOOM_ACCESS_TOKEN`, named by `default_auth`.
- [ ] A curated set of roughly four over `/v2`: meeting get, meeting create, meeting delete, user
      get — path-addressed by user and meeting id.
- [ ] **No operation declares a string-ish or `Any`-typed query parameter**, tested in the strong form
      (zero query parameters of any type, on the IR and on the emitted URL). C-30 is unimplemented, so
      the emitter still emits such a value unencoded — the defect behind `zendesk-ticket-search`. Any
      operation needing one is excluded and named in Notes.
- [ ] **No optional request-body field is declared** until C-56 lands: an omitted optional body field
      travels as an explicit `null`, which several vendors reject. Declare required fields, and name in
      Notes what was left out for this reason.
- [ ] `cargo run -p connector-cli -- build` emits `connectors/zoom.flux` and
      `connectors/zoom.connector.toml`, both committed, and a second build is byte-identical.
- [ ] The generated module passes the same parse-and-analyze and formatter fixed-point gates as every
      shipped provider, and the provider is picked up by the shared per-provider tests.
- [ ] `http_hosts` derives from the base URL and is never widened to `*`; no credential value appears
      in any generated artifact, the manifest, or the public catalogue. A test asserts it.
- [ ] Every write operation's `risk` reflects what it changes and who sees it; no write is marked
      idempotent unless the vendor documents it as such.
- [ ] **Nested `settings` is declared through `wire` paths or excluded.** A Zoom meeting's options live
      in a nested `settings` object; declare the required subset honestly with C-29's wire paths, or
      restrict the operation to top-level fields and say which options are therefore unreachable.
- [ ] Meeting create and delete both change something people see on their calendars; neither is
      `low` risk and neither is idempotent.
- [ ] The credential is a **server-to-server OAuth access token** taken from the environment. Token
      exchange is effectful acquisition — C-21's business — and must not appear in generated Flux.

## Progress
- Not started. Filed 2026-07-30 in the ten-provider fleet push.

## Notes
- Zoom's access tokens are short-lived, so this connector is declaration-complete but
  operationally dependent on C-21 in a way a static API key is not. Record that plainly.
- Deliberately excluded pending C-30: the meetings *list* with `type`/`page_size`.
- **Still cannot make a live call**, for the same reason as every shipped connector: flux's
  `http.request` takes `{"$secret": …}` as a whole-value replacement, so no scheme prefix is produced
  (`docs/designs/auth-seam.md`). This connector is the catalogue's proof, not a working client.
