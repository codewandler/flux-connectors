---
id: C-69
title: Ship the Google Workspace connector
pillar: Spec
status: in-progress
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
- [x] `providers/google.toml` is hand-authored, following the `providers/zendesk.toml` precedent: the
      header comment names the upstream document, says why it is not a `[spec]` pointer yet (C-4's
      ingest is unimplemented), and lists the operation set as the selection to reproduce.
- [x] `base_url = "https://www.googleapis.com"`, `vendor = "Google Workspace"`, and a `[[auth]]` entry with `scheme = "bearer"` over `GOOGLE_ACCESS_TOKEN`, named by `default_auth`.
- [x] Three declared services, each with a curated handful addressed by path parameters. Confirm
      shapes against current vendor docs; the intended sets are `gmail` (message get, message send,
      labels list), `calendar` (event get, event insert, calendar get), `drive` (file get, file
      metadata update).
- [x] **No operation declares a string-ish or `Any`-typed query parameter**, tested in the strong form
      (zero query parameters of any type, on the IR and on the emitted URL). C-30 is unimplemented, so
      the emitter still emits such a value unencoded — the defect behind `zendesk-ticket-search`. Any
      operation needing one is excluded and named in Notes.
- [x] **No optional request-body field is declared** until C-56 lands: an omitted optional body field
      travels as an explicit `null`, which several vendors reject. Declare required fields, and name in
      Notes what was left out for this reason.
- [x] `cargo run -p connector-cli -- build` emits `connectors/google.flux` and
      `connectors/google.connector.toml`, both committed, and a second build is byte-identical.
      **Read through C-49: the emitted unit is the service**, so the pairs are
      `connectors/google-gmail.{flux,connector.toml}`, `google-calendar.*` and `google-drive.*`, and
      there is deliberately no `google.flux` — an installable unit no service owns. All six are
      committed and `diff` reports `115 artifacts up to date (12 providers checked)`.
- [x] The generated module passes the same parse-and-analyze and formatter fixed-point gates as every
      shipped provider, and the provider is picked up by the shared per-provider tests.
- [x] `http_hosts` derives from the base URL and is never widened to `*`; no credential value appears
      in any generated artifact, the manifest, or the public catalogue. A test asserts it.
- [x] Every write operation's `risk` reflects what it changes and who sees it; no write is marked
      idempotent unless the vendor documents it as such.
- [x] **Each service declares its own `api_version`** — `gmail:v1`, `calendar:v3`, `drive:v3` — which is
      the concrete case that forced `api_version` onto the service in C-49.
- [x] Per-service hosts are correct and narrow: the manifest's `http_hosts` names
      `www.googleapis.com` (or the per-service host the vendor documents), never `*.googleapis.com`.
- [x] The story states plainly that a Google **OAuth2 access token is short-lived**: this connector
      declares a bearer credential sourced from the environment, and refresh is effectful acquisition,
      which is C-21's business and must not appear in generated Flux. Stated in Notes below and in the
      header comment of `providers/google.toml`.

## Progress
- Filed 2026-07-30 in the ten-provider fleet push.
- **Done.** `providers/google.toml` ships 8 operations across three declared services — `gmail:v1`
  (message get, message send, labels list), `calendar:v3` (event get, event insert, calendar get),
  `drive:v3` (file get, file metadata update) — and the build emits one module + manifest per service.
  `crates/connector-flux/tests/google_connector.rs` holds the connector's own claims; the shared
  per-provider gates cover it because they derive their set from `providers/` (C-54).
- **Two emitter-level defects that only a multi-service provider could expose, both fixed here:**
  - `connector-flux`'s op body bound `connector.base_url` rather than
    `base_url_of(operation.service)`, so a Gmail op would have requested `www.googleapis.com` while
    `google-gmail.connector.toml` — the manifest that installs with it, and the value C-10's
    `http_hosts` derives from — named `gmail.googleapis.com`. Fixed in
    `crates/connector-flux/src/op.rs`, with a unit test; byte-identical for every `default`-only
    provider, whose service base URL *is* the connector's.
  - `catalog.json` published `providers[].hosts` as the connector base URL's host alone, which for
    google omitted a host three of its eight operations reach. It is now the union of the services'
    hosts, in declaration order; each *service*'s `hosts` stays its own, because that one is an
    egress claim rather than a description. No existing provider's published data moved (the
    `catalog.json` diff is purely additive).
- **`every_shipped_provider_is_single_service` is gone**, as this story requires: it pinned that no
  shipped provider declared services. Its replacement,
  `every_shipped_service_is_spellable_and_a_single_service_provider_declares_none`, keeps the teeth —
  every declared name satisfies the address grammar and owns operations, no operation falls into the
  reserved `default`, single-surface providers still encode nothing at all, and it fails if *no*
  provider is multi-service.
- Four shared gates became service-aware rather than provider-shaped:
  `every_shipped_provider_compiles`, `every_shipped_operation_reaches_its_module`,
  `every_rendering_is_the_text_the_shipped_module_carries` and
  `every_shipped_provider_emits_the_pair_its_shape_calls_for` (renamed from
  `…emits_its_unsuffixed_pair`). Each now asserts the *stronger* claim — an operation must reach its
  own service's module, not merely one of the provider's.
- **No `authority` is declared**, so no address renders and `gid` is `null`, as for every other
  shipped provider. Choosing the reverse-DNS spelling of Google's authority is a permanent decision
  under C-37's stability contract and it is not this story's to make; the per-service `api_version`
  values are what an address would be built from. See `providers/google.toml`'s header.

## Notes
- **Blocked on [C-49](C-49-provider-services.md)** — without the service level this would have to be
  three separate providers or one flat operation list, and neither is what Google is. C-49 has landed,
  and this is its first real consumer.
- **A Google OAuth2 access token expires in about an hour.** This connector declares a bearer
  credential sourced from `GOOGLE_ACCESS_TOKEN` and nothing else. Refresh is effectful acquisition — a
  POST to `oauth2.googleapis.com/token` with a refresh token, a client id and a client secret — and it
  belongs to the host ([C-21](C-21-effectful-acquisition.md), `docs/designs/auth-seam.md`). It must never
  appear in generated Flux: a refresh performed inside a module would put the refresh token *and* the
  client secret into model-visible symbols. The operational consequence is that a stale token fails
  closed with a 401, which is the better of the two available failures.
- Per-operation OAuth **scopes** are exactly [C-67](C-67-required-scopes.md)'s subject. **C-67 has not
  landed, so there is no field for them**: each operation's `description` names the scope it needs
  (`gmail.send`, `calendar.events`, `drive.metadata.readonly`, …) as prose, which a model reads and no
  machine checks. Google enforces scopes server-side, so a token minted without one fails closed with
  a 403 rather than acting.
- Deliberately excluded pending C-30, and this is the widest cut in the fleet because Google's query
  surface is where all its power is: every search (`q` in Gmail's and in Drive's own query languages),
  all `pageToken` paging and therefore every collection endpoint bar `labels.list` — the one listing
  Google gives no parameters at all — Calendar's `timeMin`/`timeMax`, and every response-shaping
  parameter (`fields=`, Gmail's `format=`, Drive's `alt=media`). The last one has a consequence worth
  stating: **`google-drive-file-get` returns metadata, never file content.**
- Excluded pending C-56 (no optional body field): `threadId` on the mail send; `description`,
  `location`, `attendees`, `recurrence`, `reminders` and both `timeZone` halves on the event insert;
  `description`, `mimeType`, `starred` and `trashed` on the file update. Losing `attendees` is what
  keeps the event insert at `medium` risk — it can notify nobody — and *not* declaring `trashed` is
  what keeps the file update out of the `destructive` tier.
- **Still cannot make a live call**, for the same reason as every shipped connector: flux's
  `http.request` takes `{"$secret": …}` as a whole-value replacement, so no scheme prefix is produced
  (`docs/designs/auth-seam.md`). This connector is the catalogue's proof, not a working client.
- **Schema gaps this connector ran into**, none of them blocking:
  - `RateLimit` cannot express Google's metering, which is *weighted quota units per project per 100
    seconds* — a `messages.get` costs 5 and a `messages.send` 100 — so the limit depends on the mix of
    operations a flow runs rather than on a per-endpoint request rate. No `rate_limit` is declared,
    because any single `requests`/`per_seconds` pair would be wrong for every mix.
  - Nothing at connector level can say "every service of this provider shares one error envelope", so
    `[operations.quirks.error_envelope]` is restated on all eight operations. Same shape of gap Asana
    records for its `data` envelope.
  - `ErrorEnvelope` addresses one location, while Google's body carries both a canonical status
    (`/error/status`, declared as `code_pointer`) and an `errors[]` array whose `reason` is the
    machine-readable cause (`rateLimitExceeded`, `insufficientPermissions`). The array has nowhere to
    live.
