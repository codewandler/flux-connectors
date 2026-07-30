---
id: C-53
title: Ship the Slack connector
pillar: Spec
status: ready
priority: 3
design: docs/designs/provider-operation-inventory.md
epic: connectors-v1
areas: [providers, connector-spec]
note: bearer · POST+JSON throughout, which is what avoids the query gap
---

# Ship the Slack connector

## Goal
Add `providers/slack.toml` and its generated artifacts — the messaging connector, and the one that
proves a method-style API (`POST /api/chat.postMessage`) needs nothing new from the emitter.

## Acceptance
- [ ] `providers/slack.toml` is hand-authored, following the zendesk precedent, with the header
      comment recording that Slack's own OpenAPI description is the later `[spec]` pointer.
- [ ] `base_url = "https://slack.com"`, `vendor = "Slack"`, `[[auth]]` with `scheme = "bearer"` over
      `SLACK_BOT_TOKEN`, named by `default_auth`.
- [ ] A curated operation set of roughly four, each with `risk` and `idempotency`. Confirm against
      current vendor docs; the intended set is `slack-chat-post-message` ·
      `slack-conversations-history` · `slack-users-info` · `slack-reactions-add`.
- [ ] **Every operation is POST with a JSON body**, including the read operations. Slack's Web API
      accepts `application/json` on POST when authenticating with a bearer token, and using it is what
      keeps channel and user ids out of a query string the emitter cannot encode (C-30). The header
      comment states this as the reason, so a later reader does not "fix" a read operation into a GET.
- [ ] **No operation declares a string-ish or `Any`-typed query parameter**, tested.
- [ ] `cargo run -p connector-cli -- build` emits `connectors/slack.flux` and
      `connectors/slack.connector.toml`, committed, and a second build is byte-identical.
- [ ] The generated module passes the same parse-and-analyze and formatter fixed-point tests every
      existing provider passes.
- [ ] `crates/catalog/src/generated.rs` gains its `pub(crate) mod slack;` line and its `PROVIDERS`
      entry in id order — `crates/catalog/tests/embedded_operations.rs` fails until it does.
- [ ] `http_hosts` is `slack.com`, never widened; no credential value in any generated artifact.
- [ ] `slack-chat-post-message` and `slack-reactions-add` are visible writes to a shared workspace;
      their `risk` reflects that, and neither is marked idempotent.

## Progress
- Not started. Filed 2026-07-30 under "ship up to 3 connectors, popular and useful".

## Notes
- **Slack's `ok: false` envelope is a quirk worth recording**: it returns HTTP 200 with
  `{"ok": false, "error": "channel_not_found"}`, so a connector that only checks status codes reports
  success on failure. `Quirks.error_envelope` has `message_pointer` and `code_pointer` — use
  `/error`. If nothing expresses "the failure is in the body of a 200", report it as a schema gap in
  Progress rather than leaving it silent; this is the most likely finding of this story.
- Deliberately excluded pending C-30: cursor-based pagination (`cursor` is a string query value) and
  `conversations.list`'s `types` filter.
- **Still cannot make a live call** — same `$auth` gap as every other connector.
