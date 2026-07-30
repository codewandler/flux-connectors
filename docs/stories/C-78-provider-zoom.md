---
id: C-78
title: Ship the Zoom connector
pillar: Spec
status: done
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
- [x] `providers/zoom.toml` is hand-authored, following the `providers/zendesk.toml` precedent: the
      header comment names the upstream document, says why it is not a `[spec]` pointer yet (C-4's
      ingest is unimplemented), and lists the operation set as the selection to reproduce.
      → `providers/zoom.toml:1-21`, which lists the four `operationId`s *and* their method+path.
- [x] `base_url = "https://api.zoom.us"`, `vendor = "Zoom"`, and a `[[auth]]` entry with `scheme = "bearer"` over `ZOOM_ACCESS_TOKEN`, named by `default_auth`.
      → `providers/zoom.toml:158-190`; asserted by
      `zoom_connector.rs::the_zoom_connector_declares_one_expiring_bearer`.
- [x] A curated set of roughly four over `/v2`: meeting get, meeting create, meeting delete, user
      get — path-addressed by user and meeting id.
      → four operations, counted by `shipped_providers.rs::operation_selection_stays_curated`
      (`("zoom", 4)`); the `/v2/` prefix is asserted per operation.
- [x] **No operation declares a string-ish or `Any`-typed query parameter**, tested in the strong form
      (zero query parameters of any type, on the IR and on the emitted URL). C-30 is unimplemented, so
      the emitter still emits such a value unencoded — the defect behind `zendesk-ticket-search`. Any
      operation needing one is excluded and named in Notes.
      → `zoom_connector.rs::no_zoom_operation_declares_a_query_parameter` (IR) and
      `::no_zoom_module_assembles_a_query_string` (every `$url` binding plus `$sep`).
- [x] **No optional request-body field is declared** until C-56 lands: an omitted optional body field
      travels as an explicit `null`, which several vendors reject. Declare required fields, and name in
      Notes what was left out for this reason.
      → `zoom_connector.rs::no_zoom_body_field_is_optional`; the cost is listed in
      `providers/zoom.toml:117-138` and on `zoom-meeting-create` itself.
- [x] `cargo run -p connector-cli -- build` emits `connectors/zoom.flux` and
      `connectors/zoom.connector.toml`, both committed, and a second build is byte-identical.
      → `12 providers, 107 artifacts; 8 written`, then `107 artifacts up to date`.
- [x] The generated module passes the same parse-and-analyze and formatter fixed-point gates as every
      shipped provider, and the provider is picked up by the shared per-provider tests.
      → `zoom_connector.rs::every_zoom_operation_emits_an_analyzable_module`, plus the C-54-derived
      gates that read `providers/` (`shipped_modules.rs`, `catalog_artifacts.rs`,
      `shipped_providers_build.rs`, `site_catalog.rs`, `embedded_operations.rs`).
- [x] `http_hosts` derives from the base URL and is never widened to `*`; no credential value appears
      in any generated artifact, the manifest, or the public catalogue. A test asserts it.
      → `zoom_connector.rs::every_zoom_request_targets_one_host_and_carries_no_credential`, and
      `shipped_providers_build.rs`'s derived host/credential gates.
- [x] Every write operation's `risk` reflects what it changes and who sees it; no write is marked
      idempotent unless the vendor documents it as such.
      → `zoom-meeting-create` is `medium`/`non_idempotent`, `zoom-meeting-delete` is
      `destructive`/`non_idempotent`; each states its reasoning at its declaration.
- [x] **Nested `settings` is declared through `wire` paths or excluded.** A Zoom meeting's options live
      in a nested `settings` object; declare the required subset honestly with C-29's wire paths, or
      restrict the operation to top-level fields and say which options are therefore unreachable.
      → `settings.waiting_room` via `wire` (`providers/zoom.toml`), asserted on the IR and on the
      emitted `$payload` by `zoom_connector.rs::the_meeting_settings_object_is_declared_through_wire_paths`.
      The unreachable options are enumerated at that field.
- [x] Meeting create and delete both change something people see on their calendars; neither is
      `low` risk and neither is idempotent.
      → `zoom_connector.rs::neither_meeting_write_is_low_risk_or_idempotent`, which also pins the
      delete at `Risk::Destructive` — the emitter permits `idempotent` on a `DELETE`, so this test is
      the only thing that forbids it here.
- [x] The credential is a **server-to-server OAuth access token** taken from the environment. Token
      exchange is effectful acquisition — C-21's business — and must not appear in generated Flux.
      → `zoom_connector.rs::no_zoom_module_performs_a_token_exchange` greps every emitted operation for
      `oauth`, `grant_type`, `account_credentials`, `client_secret`, `client_id`, `account_id` and
      `refresh`.

## Progress
- Filed 2026-07-30 in the ten-provider fleet push.
- **Done on `impl/C-78`.** Four operations over `/v2`, one bearer credential, one nested `settings`
  option through a C-29 wire path, no query parameter anywhere and no optional body field.
- The **body root mixes leaves and a branch** — `{topic, type, start_time, duration, settings:
  {waiting_room}}` — which zendesk (everything under `ticket.`) and asana (everything under `data.`)
  do not exercise. babelforce's agent-status update is the only other mixed root, and both of its
  fields are optional. It needed no emitter change; `BodyNode` already handles it.
- `type` is pinned with a JSON Schema `const = 2` rather than declared, the same device
  `providers/zendesk.toml` uses for `ticket.safe_update`: it is Zoom's *scheduled* meeting type, and
  it is the only type this operation can honestly offer, because the recurring types need the
  `recurrence` object C-56 makes undeclarable and the instant type ignores the required `start_time`
  and `duration`. The emitter binds it as `$type = 2` and keeps it out of the op signature.
- `start_time` is UTC-only by `pattern`. Zoom's local-time form needs a paired `timezone`, and under
  C-56 that would be an always-sent field Zoom ignores whenever the start time is already absolute.
- **New schema gap recorded** (`providers/zoom.toml`, third header block): a *response* field can be a
  credential and nothing can say so. Zoom's meeting object carries `start_url`, which embeds the host's
  ZAK token and starts the meeting as the host for anyone holding it. `zoom-meeting-get` and
  `zoom-meeting-create` both return it into a model-visible symbol, and there is no field on
  `Operation` or inside `response_schema` that marks a response location sensitive the way `[[auth]]`
  marks a request credential. The two `response_schema` descriptions flag it in the one place a
  consumer of `web/public/catalog.json` will read; closing it needs a host-applied response-redaction
  declaration, which is C-10/C-21 territory.
- Failing-first proof: `crates/connector-flux/tests/zoom_connector.rs` — all 9 tests fail at the merge
  base with `cannot read …/providers/zoom.toml … — C-78 ships the Zoom connector`.
- The full repository gate is green, plus `npm run build && npm test` in `web/`.

## Notes
- Zoom's access tokens are short-lived, so this connector is declaration-complete but
  operationally dependent on C-21 in a way a static API key is not. Record that plainly.
  **Recorded plainly, in `providers/zoom.toml`'s second header block:** a server-to-server OAuth token
  expires in one hour, and nothing in this repository can renew it. `AuthMethod::oauth2` is the field
  that would say "the host mints this one" and it is left unset — the same as on every other bearer
  provider here, but for a weaker reason, because theirs is minted once in a dashboard and does not
  expire. `OAuth2Spec` could not describe Zoom's grant anyway: `OAuthGrant` has `client_credentials`
  but not `account_credentials`, and the required `account_id` has no field; `OAuth2Spec::endpoint`
  resolves against a declared endpoint's base URL, and Zoom's token host is not the API host, so there
  is nowhere to put it without widening the egress allow-list `http_hosts` derives from. So an operator
  who sets `ZOOM_ACCESS_TOKEN` finds it stale within the hour — a larger gap than the `$auth` seam
  every shipped connector shares.
- Deliberately excluded pending C-30: the meetings *list* with `type`/`page_size`, and its opaque
  `next_page_token` cursor. This is the omission a reader will notice — an agent can create a meeting
  and read one by id, but cannot enumerate a user's schedule. Also excluded for the same reason:
  `occurrence_id` and `show_previous_occurrences` on meeting get, `schedule_for_reminder` and
  `cancel_meeting_reminder` on meeting delete (so **whether Zoom emails anyone about a cancellation is
  Zoom's default, not this connector's choice**), and `login_type`/`encrypted_email` on user get.
- Deliberately excluded pending C-56, all on `zoom-meeting-create`: top-level `agenda`, `password`,
  `default_password`, `timezone`, `schedule_for`, `template_id`, `tracking_fields`, `pre_schedule`, and
  `recurrence` — whose absence is what makes Zoom's recurring meeting types unreachable. Inside
  `settings`, everything except `waiting_room`: `host_video`, `participant_video`, `mute_upon_entry`,
  `join_before_host`, `audio`, `auto_recording`, `meeting_authentication`, `approval_type`,
  `registration_type`, `alternative_hosts`, `contact_email` and `meeting_invitees`. The last is what
  holds the create at `risk = "medium"` rather than `high`: with no invitee list, Zoom notifies nobody.
- Also excluded, though C-30 is not the reason: a meeting **UUID** as the path id. A Zoom UUID is
  base64 that can carry `/`, and Zoom's own documentation requires it to be double URL-encoded in a
  path. Both meeting operations therefore take the numeric id only.
- **Still cannot make a live call**, for the same reason as every shipped connector: flux's
  `http.request` takes `{"$secret": …}` as a whole-value replacement, so no scheme prefix is produced
  (`docs/designs/auth-seam.md`). This connector is the catalogue's proof, not a working client.
