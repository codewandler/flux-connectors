---
id: C-53
title: Ship the Slack connector
pillar: Spec
status: in-progress
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
- [x] `providers/slack.toml` is hand-authored, following the zendesk precedent, with the header
      comment recording that Slack's own OpenAPI description is the later `[spec]` pointer.
- [x] `base_url = "https://slack.com"`, `vendor = "Slack"`, `[[auth]]` with `scheme = "bearer"` over
      `SLACK_BOT_TOKEN`, named by `default_auth`.
- [x] A curated operation set of roughly four, each with `risk` and `idempotency`. Confirm against
      current vendor docs; the intended set is `slack-chat-post-message` ·
      `slack-conversations-history` · `slack-users-info` · `slack-reactions-add`.
- [x] **Every operation is POST with a JSON body**, including the read operations. Slack's Web API
      accepts `application/json` on POST when authenticating with a bearer token, and using it is what
      keeps channel and user ids out of a query string the emitter cannot encode (C-30). The header
      comment states this as the reason, so a later reader does not "fix" a read operation into a GET.
- [x] **No operation declares a string-ish or `Any`-typed query parameter**, tested.
- [x] `cargo run -p connector-cli -- build` emits `connectors/slack.flux` and
      `connectors/slack.connector.toml`, committed, and a second build is byte-identical.
- [x] The generated module passes the same parse-and-analyze and formatter fixed-point tests every
      existing provider passes.
- [x] `crates/catalog/src/generated.rs` gains its `pub(crate) mod slack;` line and its `PROVIDERS`
      entry in id order — `crates/catalog/tests/embedded_operations.rs` fails until it does.
- [x] `http_hosts` is `slack.com`, never widened; no credential value in any generated artifact.
- [x] `slack-chat-post-message` and `slack-reactions-add` are visible writes to a shared workspace;
      their `risk` reflects that, and neither is marked idempotent.

## Progress
- **Done.** `providers/slack.toml` plus its 11 generated artifacts ship; the catalogue is 4 providers
  and 29 operations. Slack is the first provider whose only recorded defect is the catalogue-wide
  `credential-not-injected`: no `unencodable-query-value`, no `no-credential`, and no
  `unbound-base-url-template`, because `https://slack.com` needs no tenant binding. Closing the
  `$auth` seam therefore makes this connector work without any further provider-side change.
- Failing-first test is `crates/connector-flux/tests/slack_connector.rs` (5 tests). It asserts the
  query surface is empty *twice* — over the IR, and over the emitted text via the absence of the
  emitter's `$sep` separator machinery — because nothing else in the repository would fail if someone
  converted a read to a GET, and the result would look tidier and be broken.

### Schema gaps found

1. **Nothing expresses "the failure is in the body of a 200."** This is the story's predicted
   finding, and it is real. `ErrorEnvelope` carries `message_pointer` and an optional `code_pointer`
   and its own doc comment scopes it to *"where a vendor hides the real error inside a **non-2xx**
   response body"*. There is no success predicate — no `ok_pointer`, no `success_when` — so Slack's
   central quirk cannot be declared. The envelope *is* declared with `message_pointer = "/error"` so
   the location stays machine-readable for C-12, and `connector-flux`'s `description()` then appends
   *"A non-2xx response is returned as data, not a failure…"*, which is true but points a model at
   the wrong signal. The workaround is that each operation's own `description` states the `ok: false`
   contract in prose. Fix: a success-predicate field on `ErrorEnvelope`, plus the already-recorded
   flux seam making `http.request` return a record instead of one flat string.
2. **`message_pointer` is mandatory and documented as a human-readable message**, but Slack publishes
   only a machine code at `/error` and no message field — so the required field has to carry a code.
   A narrower facet of the same gap.
3. **Cursor pagination is unexpressible for a POST+JSON API.** `Pagination::Cursor.cursor_param` is
   defined as *"The query parameter carrying the cursor"*; there is no body-field spelling. So the
   story's exclusion of cursor paging is forced by the quirk model and not only by C-30:
   `slack-conversations-history` is bounded with `limit` instead. Closing it needs the quirk to admit
   a body-carried cursor.
4. **A method-style read cannot declare read metadata.** `connector-flux`'s `check_write_metadata`
   derives write-ness from the HTTP method, so a POST is refused if it declares `risk = "low"` or
   `idempotency = "idempotent"`. Both reads here are genuinely low-risk and repeatable and neither
   can say so; they are recorded `medium` / `non_idempotent`, the weakest values the emitter accepts.
   The direction is fail-closed, but on this provider `risk` no longer separates a read from a write
   — which is why `slack-reactions-add` and the two reads all sit at `medium`. Fix: decide write-ness
   from something other than the verb.

### Vendor-documentation risk, recorded rather than assumed

Confirmed against docs.slack.dev: `application/json` requires the token to travel as a bearer in the
`Authorization` header, which is what makes this request shape legal. But the Web API overview scopes
its JSON guarantee to *"most **write** methods"*, and the reference pages for the two reads
(`conversations.history`, `users.info`) list `application/json` among accepted content types while
giving their HTTP method as `GET`. So POST+JSON is unambiguously documented for the two writes and
rests on an inference for the two reads. This is the one vendor-behaviour assumption in the file and
is what C-15's live run must verify first. If a read rejects POST, the fix is C-30's
percent-encoding, **not** a GET with a raw `channel=` in the query string.

## Notes
- **Slack's `ok: false` envelope is a quirk worth recording**: it returns HTTP 200 with
  `{"ok": false, "error": "channel_not_found"}`, so a connector that only checks status codes reports
  success on failure. `Quirks.error_envelope` has `message_pointer` and `code_pointer` — use
  `/error`. If nothing expresses "the failure is in the body of a 200", report it as a schema gap in
  Progress rather than leaving it silent; this is the most likely finding of this story.
- Deliberately excluded pending C-30: cursor-based pagination (`cursor` is a string query value) and
  `conversations.list`'s `types` filter.
- **Still cannot make a live call** — same `$auth` gap as every other connector.
