---
id: C-69
title: Ship the Google Workspace connector
pillar: Spec
status: ready
priority: 7
design: docs/designs/provider-operation-inventory.md
epic: provider-fleet
areas: [providers, connector-spec]
note: the multi-service showcase: gmail · calendar · drive under one provider
---

# Ship the Google Workspace connector

## Goal
Prove the service level on the vendor that most needs it: one provider, three services —
`gmail`, `calendar`, `drive` — each with its own API version and its own curated operations.

## Acceptance
- [ ] `providers/google.toml` is hand-authored, following the `providers/zendesk.toml` precedent: the
      header comment names the upstream document, says why it is not a `[spec]` pointer yet (C-4's
      ingest is unimplemented), and lists the operation set as the selection to reproduce.
- [ ] `base_url = "https://www.googleapis.com"`, `vendor = "Google Workspace"`, and a `[[auth]]` entry with `scheme = "bearer"` over `GOOGLE_ACCESS_TOKEN`, named by `default_auth`.
- [ ] Three declared services, each with a curated handful addressed by path parameters. Confirm
      shapes against current vendor docs; the intended sets are `gmail` (message get, message send,
      labels list), `calendar` (event get, event insert, calendar get), `drive` (file get, file
      metadata update).
- [ ] **No operation declares a string-ish or `Any`-typed query parameter**, tested in the strong form
      (zero query parameters of any type, on the IR and on the emitted URL). C-30 is unimplemented, so
      the emitter still emits such a value unencoded — the defect behind `zendesk-ticket-search`. Any
      operation needing one is excluded and named in Notes.
- [ ] **No optional request-body field is declared** until C-56 lands: an omitted optional body field
      travels as an explicit `null`, which several vendors reject. Declare required fields, and name in
      Notes what was left out for this reason.
- [ ] `cargo run -p connector-cli -- build` emits `connectors/google.flux` and
      `connectors/google.connector.toml`, both committed, and a second build is byte-identical.
- [ ] The generated module passes the same parse-and-analyze and formatter fixed-point gates as every
      shipped provider, and the provider is picked up by the shared per-provider tests.
- [ ] `http_hosts` derives from the base URL and is never widened to `*`; no credential value appears
      in any generated artifact, the manifest, or the public catalogue. A test asserts it.
- [ ] Every write operation's `risk` reflects what it changes and who sees it; no write is marked
      idempotent unless the vendor documents it as such.
- [ ] **Each service declares its own `api_version`** — `gmail:v1`, `calendar:v3`, `drive:v3` — which is
      the concrete case that forced `api_version` onto the service in C-49.
- [ ] Per-service hosts are correct and narrow: the manifest's `http_hosts` names
      `www.googleapis.com` (or the per-service host the vendor documents), never `*.googleapis.com`.
- [ ] The story states plainly that a Google **OAuth2 access token is short-lived**: this connector
      declares a bearer credential sourced from the environment, and refresh is effectful acquisition,
      which is C-21's business and must not appear in generated Flux.

## Progress
- Not started. Filed 2026-07-30 in the ten-provider fleet push.

## Notes
- **Blocked on [C-49](C-49-provider-services.md)** — without the service level this would have to be
  three separate providers or one flat operation list, and neither is what Google is.
- Per-operation OAuth **scopes** are exactly [C-67](C-67-required-scopes.md)'s subject; declare them if
  C-67 has landed, otherwise name the omission.
- Deliberately excluded pending C-30: every `list` endpoint with a query filter (`q`, `pageToken`).
- **Still cannot make a live call**, for the same reason as every shipped connector: flux's
  `http.request` takes `{"$secret": …}` as a whole-value replacement, so no scheme prefix is produced
  (`docs/designs/auth-seam.md`). This connector is the catalogue's proof, not a working client.
