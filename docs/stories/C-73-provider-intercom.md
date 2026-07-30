---
id: C-73
title: Ship the Intercom connector
pillar: Spec
status: in-progress
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
- [x] `providers/intercom.toml` is hand-authored, following the `providers/zendesk.toml` precedent: the
      header comment names the upstream document, says why it is not a `[spec]` pointer yet (C-4's
      ingest is unimplemented), and lists the operation set as the selection to reproduce.
- [x] `base_url = "https://api.intercom.io"`, `vendor = "Intercom"`, and a `[[auth]]` entry with `scheme = "bearer"` over `INTERCOM_ACCESS_TOKEN`, named by `default_auth`.
- [x] A curated set of roughly five, path-addressed: contact get, contact create, conversation
      get, conversation reply, note create.
- [x] **No operation declares a string-ish or `Any`-typed query parameter**, tested in the strong form
      (zero query parameters of any type, on the IR and on the emitted URL). C-30 is unimplemented, so
      the emitter still emits such a value unencoded — the defect behind `zendesk-ticket-search`. Any
      operation needing one is excluded and named in Notes.
- [x] **No optional request-body field is declared** until C-56 lands: an omitted optional body field
      travels as an explicit `null`, which several vendors reject. Declare required fields, and name in
      Notes what was left out for this reason.
- [x] `cargo run -p connector-cli -- build` emits `connectors/intercom.flux` and
      `connectors/intercom.connector.toml`, both committed, and a second build is byte-identical.
- [x] The generated module passes the same parse-and-analyze and formatter fixed-point gates as every
      shipped provider, and the provider is picked up by the shared per-provider tests.
- [x] `http_hosts` derives from the base URL and is never widened to `*`; no credential value appears
      in any generated artifact, the manifest, or the public catalogue. A test asserts it.
- [x] Every write operation's `risk` reflects what it changes and who sees it; no write is marked
      idempotent unless the vendor documents it as such.
- [x] **The `Intercom-Version` header is optional and the story says what that costs.** Intercom
      defaults to the workspace's pinned version when the header is absent, so the connector is
      well-formed without it — but the version cannot be pinned until
      [C-55](C-55-constant-request-headers.md) lands. Record it as a known limitation, not as a gap
      that blocks the connector.

## Progress
- Filed 2026-07-30 in the ten-provider fleet push.
- **Done.** `providers/intercom.toml` ships five operations —
  `GET /contacts/{id}`, `POST /contacts`, `GET /conversations/{id}`,
  `POST /conversations/{id}/reply`, `POST /contacts/{id}/notes` — over
  `https://api.intercom.io` with one bearer credential, `intercom.access_token` from
  `INTERCOM_ACCESS_TOKEN`. `build` emits 9 new artifacts; `diff` reports
  `67 artifacts up to date (7 providers checked)`.
- The provider is picked up automatically by every per-provider gate, because C-54 derives the shipped
  set from `providers/`. Two edits were still explicit and are the only shared files touched:
  `crates/catalog/src/generated.rs` (the hand-maintained module index) and the curated count
  `("intercom", 5)` in `connector-spec`'s `operation_selection_stays_curated`, which is an inventory
  claim rather than a copy of the provider set.
- `crates/connector-flux/tests/intercom_connector.rs` is the connector's own gate: zero query
  parameters on the IR *and* on every `$url` binding of the emitted text, zero optional body fields,
  zero header parameters, honest write metadata, and the C-11 parse/analyze/canonical-form gate.
  `crates/connector-cli/tests/shipped_providers_build.rs::intercom_publishes_one_host_and_no_credential_in_its_module`
  covers the two pipeline-derived claims: the host is exactly `api.intercom.io` with no wildcard, and
  the generated Flux names no credential at all.
- **The version header is the story's subtlety and it is recorded, not smuggled.** `Intercom-Version`
  is not declared anywhere. A `params.header` entry with a JSON Schema `const` would emit as a
  required, caller-*overridable* argument with the `const` dropped — C-52's finding, because
  `op.rs` filters `constant(...)` on the `body` chain only — so declaring it that way would be a
  disguise rather than a pin. `no_intercom_operation_declares_a_header_parameter` fails if a later
  author tries. The cost, stated in the TOML header: the connector's contract is whatever version the
  operator's workspace is pinned to, and an Intercom-side default change moves it silently. C-55 is
  what closes this, and that test is the one to change deliberately when it lands.
- Left out for the C-56 null-body reason, so a resuming agent knows what to fill in: contact create's
  `external_id`, `phone`, `name`, `avatar`, `signed_up_at`, `last_seen_at`, `owner_id`,
  `unsubscribed_from_emails`, `custom_attributes`; conversation reply's `attachment_urls` and
  `created_at`. The most consequential is `external_id` — without it the operation creates
  **email-identified contacts only**, which is not what a system-of-record integration wants.
  `intercom-contact-note-create` goes the other way and declares `admin_id` **required** although
  Intercom documents it as optional, so every note has an author; that is stricter than the vendor and
  is a second thing C-56 should revisit.
- Gate run in this worktree with its own target directory: `cargo fmt --all`,
  `cargo build --workspace`, `cargo test --workspace` (the `FAILED|error: test failed|panicked at`
  diagnostic prints nothing), `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all --check`, plus `npm ci && npm run build && npm test` in `web/` (10/10) because
  `web/public/catalog.json` was regenerated. Not touched: `CHANGELOG.md`, the board,
  `docs/roadmap.md`, `Cargo.lock`, `README.md`, any dependency list.

## Notes
- Contrast worth recording for whoever picks up C-55: Intercom *defaults* its version, so omission
  is survivable — Notion and Anthropic **require** their version header and were therefore excluded
  from this fleet entirely.
- Deliberately excluded pending C-30: conversation search and every `per_page`/`starting_after` page.
  `starting_after` is an opaque cursor, which is the unencodable shape exactly; `?display_as=plaintext`
  on the conversation read goes with them, so a caller who needs plaintext rendering cannot ask for it
  here. Cursor pagination is also left off as a quirk, since `Pagination::Cursor.cursor_param` is
  defined as a *query* parameter and declaring it would declare the thing this connector must not have.
- Two schema gaps recorded in the TOML rather than worked around. First, `ErrorEnvelope` holds one
  `message_pointer`, but Intercom answers with a **list** — `{"type": "error.list", "errors": [...]}` —
  so the pointers name `/errors/0` and a second error of a multi-error validation failure has nowhere
  to live; `request_id`, which Intercom's support process asks for, has no field either. Second, the
  regional hosts `api.eu.intercom.io` and `api.au.intercom.io` are not selectable: `base_url` is a
  plain string and nothing declares "this part is operator-chosen", the same missing binding zendesk
  records for `{subdomain}` → `ZENDESK_URL` (C-17). An EU workspace needs C-17 or a second connector,
  never a widened host list. No `[quirks.rate_limit]` is declared, because Intercom's limit varies by
  plan and version and a published bound that is wrong is a wrong contract.
- **Still cannot make a live call**, for the same reason as every shipped connector: flux's
  `http.request` takes `{"$secret": …}` as a whole-value replacement, so no scheme prefix is produced
  (`docs/designs/auth-seam.md`). This connector is the catalogue's proof, not a working client.
