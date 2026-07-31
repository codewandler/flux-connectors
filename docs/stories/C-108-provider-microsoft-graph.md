---
id: C-108
title: Ship the Microsoft Graph connector
pillar: Spec
status: done
design:
epic: provider-fleet-2
areas: [providers]
note: "the second multi-service provider after Google — and the first where the services share a host, so it tests whether a service is a real level or just Google's URL problem"
---

# Ship the Microsoft Graph connector

## Goal
Mail, calendar and files behind one vendor — and a second, differently-shaped test of the service
level.

## Acceptance
- [x] Services for the surfaces shipped — `mail`, `calendar`, `files` — each with its own curated
      operations, under one authority. → `providers/microsoft_graph.toml` `[[services]]` (three
      entries: `mail`, `calendar`, `files`), one vendor/connector (no `authority` field is declared —
      see the next item and the file's header comment for why that is a deliberate, separate
      decision from "one vendor").
- [x] **The services share `graph.microsoft.com`.** Google's three justified themselves partly by
      differing hosts (`gmail.googleapis.com` versus `www.googleapis.com`); these do not. So this
      connector answers a real question: is a service a genuine addressing level, or was it Google's
      host problem wearing a hat? Whichever the answer, the file records it — and if the honest
      answer is "one service", that is a finding about C-49 and not a failure here.
      → Answered and recorded in `providers/microsoft_graph.toml`'s header comment ("C-108'S
      QUESTION"): all three services share both `base_url` (`https://graph.microsoft.com`) *and*
      `api_version` (`v1.0`) — a strictly harder case than Google's, which only shared neither.
      **The finding is that the level still earns its place**, for a different reason than Google's:
      not routing or versioning (neither varies), but the installable unit — `--service mail`
      installs exactly the three mail operations into `microsoft_graph-mail.flux`, never the other
      five. `crates/connector-flux/tests/microsoft_graph_connector.rs::graph_services_share_a_host_and_version_but_still_partition_cleanly`
      asserts the shared host/version directly rather than assuming it.
- [x] Auth: OAuth2 bearer. Coordinate with [C-88](C-88-prove-oauth2.md) rather than duplicating it —
      if C-88 has landed, this connector uses the proven shape; if not, this is a candidate to be the
      provider that proves it.
      → C-88 is still `status: ready` as of this story and has already earmarked Slack or HubSpot as
      its provider (its own Acceptance names them explicitly), not this connector. So this file takes
      the already-minted-bearer-token shape `providers/google.toml`/`providers/zoom.toml` ship — one
      `[[auth]] scheme = "bearer"` credential over an environment variable, no `[auth.oauth2]` block —
      and records that choice and the coordination reasoning in its header comment rather than
      duplicating C-88's unfinished work.
- [x] A `[[config]]` surface, a `verify` operation, and a per-provider contract test.
      → One `[[config]]` field (`access_token`, necessarily scoped to one service — `mail` — because
      `ConfigField::service` must name a declared service and there is no "shared across services"
      spelling; recorded as a finding in the TOML's Configuration section). `verify =
      "microsoft_graph-calendar-calendar-get"`, a zero-argument low-risk read
      (`GET /v1.0/me/calendar`). Contract test:
      `crates/connector-flux/tests/microsoft_graph_connector.rs`, 12 tests.

## Progress
- **Implemented.** Eight operations across three services — three mail (`message-get`,
  `message-reply`, `folder-list`), three calendar (`event-get`, `event-create`, `calendar-get`), two
  files (`item-get`, `item-update`). Zero query parameters anywhere (every Graph OData system query
  option — `$filter`, `$search`, `$select`, `$expand`, `$orderby`, `$top`, `$skip` — was checked
  against the query-encoding trap this story's dispatch specifically warned about, and excluded; see
  the TOML's own "SCHEMA GAP: NO QUERY PARAMETER" section for the per-parameter reasoning). No array
  body field anywhere, so C-185 is not exercised. Every write is `POST` or `PATCH`, so C-186 forces
  `non_idempotent` on all three regardless of vendor semantics — which happens to agree with what
  Graph documents (no idempotency key on any of the three).
- **A real, separate defect surfaced and was fixed: the connector id cannot contain a hyphen.**
  `crates/connector-cli/src/catalog.rs::module_ident` requires a provider id to be a valid Rust
  identifier (`^[a-z_][a-z0-9_]*$`) because a full build declares `mod <id>;` in
  `crates/catalog/src/generated.rs`. The story's own id, "microsoft-graph", violates that — a
  `--provider`-scoped build does not catch it (it never touches `generated.rs`), but
  `connector-cli::core_catalog`'s `a_full_build_publishes_every_core_id_and_schema_once` does, because
  it simulates a full build's plan. Caught during this story's own gate run, not left for the
  coordinator's integration build to discover. Renamed the provider id to `microsoft_graph`
  (underscore) throughout: the TOML, its file name, every op id's prefix, the credential name, and the
  contract test. Service names (`mail`/`calendar`/`files`) and op-id suffixes keep hyphens — only the
  provider id itself is constrained.
- **No credential-bearing response field, checked and none found.** This story's dispatch asked for
  the Zoom `start_url`/Postmark `ApiTokens` treatment on any response field that carries a bearer
  secret. Every link Graph returns here (`webLink`, `webUrl`) requires the viewer's own separate
  sign-in and embeds nothing privileged, unlike Zoom's ZAK-bearing `start_url`. Teams'
  `onlineMeeting.joinUrl` would need that treatment, but no operation here sets `isOnlineMeeting`, so
  none can return one — recorded in the TOML's header rather than left implicit.
- **No PII anywhere in the file.** Every response schema names which fields carry a person's name,
  address or id in its `description`, without ever giving one a literal example value — verified
  against real Microsoft Learn reference pages that do show example names/addresses, none of which
  were copied in.
- **Not shipped, and why:** mail send (`POST /me/sendMail`) — its `toRecipients` is the
  array-of-objects C-185 refuses; `microsoft_graph-mail-message-reply` ships instead, whose recipients
  Graph resolves automatically. File content download (`GET /me/drive/items/{id}/content`) — a binary
  response, a different shape this pipeline has not explored, and not needed by the curated set. Full
  list and reasoning in the TOML's "EXCLUDED" section.

## Notes
- Microsoft versions by path prefix (`/v1.0`, `/beta`), which interacts with how `base_url` and
  `api_version` divide — Shopify already hit the "API version lives in the path" shape and its file
  records the reasoning. Read it before deciding.
  → Confirmed and followed: `api_version` on a `[[services]]` entry is inert metadata that only feeds
  `Connector::gid_of` (and this connector declares no `authority`, so no `gid` renders at all) —
  `connector-flux/src/op.rs` builds the request URL from the service's `base_url` and `operation.path`
  alone, never from `api_version`. So every `path` below spells `/v1.0/` literally, the same shape
  `providers/shopify.toml` uses and, notably, the same shape `providers/google.toml` uses even on
  services whose `api_version` genuinely varies (`/gmail/v1/...`) — declaring the field does not
  change how the path is written.

### Coordinator note at integration

**It caught a defect in the story's own premise, in its own gate.** The story implied a provider id of
`microsoft-graph`. A provider id must be a valid Rust identifier — `catalog.rs::module_ident` requires
`^[a-z_][a-z0-9_]*$` because a full build declares `mod <id>;` — so a hyphen fails. It found this through
`connector-cli::core_catalog`'s full-build simulation *before* handing over, rather than leaving me to hit
it at integration, and renamed the id to `microsoft_graph` while keeping hyphens in service names and
operation-id suffixes where they are legal.

That is the same family as C-171's finding (`box` is a Rust keyword and `module_ident` escapes it to
`r#box`): **the provider id is the one author-chosen string that has to survive becoming Rust.** Two
connectors in one run hit it from different directions and both traced it rather than working around it.

Its answer on the service question is worth keeping too: all three services share the host **and** the API
version, which is stricter than Google's case. The conclusion is not that the service level is unnecessary
but that it earns its place as **the installable unit**, not as a routing or versioning mechanism — and a
test asserts exactly that.
