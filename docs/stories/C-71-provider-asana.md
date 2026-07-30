---
id: C-71
title: Ship the Asana connector
pillar: Spec
status: ready
priority: 7
design: docs/designs/provider-operation-inventory.md
epic: provider-fleet
areas: [providers, connector-spec]
note: bearer · every body and response wrapped in `data`
---

# Ship the Asana connector

## Goal
Ship a clean, well-documented task API — and with it the first connector whose every request
body and response is wrapped in a `data` envelope, which is a real test of C-29's wire paths.

## Acceptance
- [ ] `providers/asana.toml` is hand-authored, following the `providers/zendesk.toml` precedent: the
      header comment names the upstream document, says why it is not a `[spec]` pointer yet (C-4's
      ingest is unimplemented), and lists the operation set as the selection to reproduce.
- [ ] `base_url = "https://app.asana.com"`, `vendor = "Asana"`, and a `[[auth]]` entry with `scheme = "bearer"` over `ASANA_ACCESS_TOKEN`, named by `default_auth`.
- [ ] A curated set of roughly five, path-addressed over `/api/1.0`: task get, task create,
      task update, story (comment) add, project get.
- [ ] **No operation declares a string-ish or `Any`-typed query parameter**, tested in the strong form
      (zero query parameters of any type, on the IR and on the emitted URL). C-30 is unimplemented, so
      the emitter still emits such a value unencoded — the defect behind `zendesk-ticket-search`. Any
      operation needing one is excluded and named in Notes.
- [ ] **No optional request-body field is declared** until C-56 lands: an omitted optional body field
      travels as an explicit `null`, which several vendors reject. Declare required fields, and name in
      Notes what was left out for this reason.
- [ ] `cargo run -p connector-cli -- build` emits `connectors/asana.flux` and
      `connectors/asana.connector.toml`, both committed, and a second build is byte-identical.
- [ ] The generated module passes the same parse-and-analyze and formatter fixed-point gates as every
      shipped provider, and the provider is picked up by the shared per-provider tests.
- [ ] `http_hosts` derives from the base URL and is never widened to `*`; no credential value appears
      in any generated artifact, the manifest, or the public catalogue. A test asserts it.
- [ ] Every write operation's `risk` reflects what it changes and who sees it; no write is marked
      idempotent unless the vendor documents it as such.
- [ ] **The `data` envelope is declared through `wire` paths, not flattened.** Asana wraps every
      request body in `{{"data": {{…}}}}` and every response in `{{"data": …}}`. C-29 added wire paths for
      exactly this; a connector that ignores the envelope produces requests Asana rejects.
- [ ] The response envelope is recorded so a consumer knows the payload is at `/data`.

## Progress
- Not started. Filed 2026-07-30 in the ten-provider fleet push.

## Notes
- Asana is the best available proof that C-29's nested-body support works on a real vendor rather
  than a fixture — babelforce's bodies motivated it, but the envelope here is universal across every
  operation.
- Deliberately excluded pending C-30: `opt_fields`, `limit`/`offset` paging, and workspace search.
- **Still cannot make a live call**, for the same reason as every shipped connector: flux's
  `http.request` takes `{"$secret": …}` as a whole-value replacement, so no scheme prefix is produced
  (`docs/designs/auth-seam.md`). This connector is the catalogue's proof, not a working client.
