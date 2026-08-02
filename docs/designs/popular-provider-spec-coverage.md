# Five popular providers — spec-backed coverage expansion

## Goal and boundary

Increase useful, curated coverage for five existing providers by making their first-party OpenAPI
documents the upstream contract:

| provider | pre-wave baseline operations | first-party source family | initial expansion target |
|---|---:|---|---|
| GitHub | 5 | `github/rest-api-description` | issue, pull-file, workflow-run and commit collection reads |
| Stripe | 8 | `stripe/openapi` | country-spec, event, exchange-rate and billing-meter collections |
| Microsoft Graph | 8 | `microsoftgraph/msgraph-metadata` v1.0 | message and Outlook calendar-metadata reads |
| OpenAI | 4 | `openai/openai-openapi` | stored responses, response inputs, files and batches reads |
| Twilio | 5 | `twilio/twilio-oai` API v2010 | recording, usage and conference reads |

The baseline counts above were re-measured on 2026-08-02 from `web/public/catalog.json`. All five provider ids
already exist, so this work expands providers; it does not add providers or trigger the separate
"release after each new provider" rule.

Slack was considered but not selected: its first-party machine description is Swagger 2.0, while
`connector-spec` deliberately accepts OpenAPI 3 and refuses Swagger. HubSpot was measured and then
removed from the wave: its first-party repository and documents carry no redistribution license, so
public availability was not treated as permission to commit the bytes. Shopify and Salesforce were
not selected because no comparably stable, complete first-party OpenAPI artifact was established for
their current connector surfaces. These are source-contract decisions, not popularity judgements.

## Source policy

Each provider vendors an exact upstream document at a reviewed commit or immutable release. The
vendor script:

- fetches only public first-party URLs and supports deterministic `--source-dir` replay;
- records upstream and vendored SHA-256, source commit/tag and retrieval date;
- removes example values that look like credentials, personal email addresses or telephone numbers
  while preserving declarations and security schemes;
- refuses an unexpected OpenAPI version, missing selected operation id, duplicate operation id or
  outbound `paths` source inventory change (callback and root `webhooks` Operation Objects are not
  selectable outbound connector calls);
- never runs during `build`, `diff` or `check`.

The full pulled contract is kept where practical. If an upstream monolith is too large to review,
the repository keeps the full upstream hash plus a deterministic, allowlist-shaped extraction
script and vendors the extracted OpenAPI document. The extraction must retain referenced schemas,
security declarations, servers and every selected operation verbatim; it cannot be a hand-authored
excerpt.

## Selection policy

OpenAPI is evidence, not the published catalogue. Every new operation is opted in by exact operation
id and reviewed for:

- stable provider/service address and host;
- required path/body parameters that the ingest can represent without guessing;
- query parameters curated rather than swept wholesale;
- response schema or an explicit documented absence;
- credential placement and scopes;
- low/high/destructive risk, idempotency and semantic effects;
- a request rehearsal proving declared configuration produces an absolute, brace-free URL without
  moving body or headers.

Existing operation ids, addresses and Flux renderings are pinned before changing a provider. Moving
an existing hand-authored operation onto the spec is allowed only when its emitted contract remains
byte-identical or the change is deliberately documented as breaking.

### One value may own Basic auth and an imported request path

Twilio's Account SID is both the non-secret username half of Basic authentication and the required
`AccountSid` segment in every API v2010 path. Asking for it twice creates two independently mutable
answers; leaving the imported path parameter in the tool lets a caller redirect an authenticated
request. C-475 therefore extends the existing multi-destination field rather than either compromise.

A username-headed request pin emits a qualified `username.<credential>` placeholder, which the pack
resolves through the username configuration address rather than the endpoint address used by legacy
pins. An imported path parameter may be omitted only by an explicit operation patch and only when an
exact path pin in that operation's service proves what fills the surviving template variable. The
loader continues to refuse every unclaimed path omission and every pin a caller can also supply.

## Initial bounded coverage

The inventory story selects exact upstream operation ids, but the implementation target is not a
single proof operation. Each provider adds at least four useful operations spanning a collection read
and, where the provider's existing purpose requires it, a reviewed mutation:

- GitHub: issues collection, pull-request files, workflow runs and commits collection.
- Stripe: country specs, events, exchange rates and billing meters; customer, payment-intent,
  invoice and subscription responses remain explicit recursive-schema deferrals.
- Microsoft Graph: messages plus master categories, supported time zones and supported languages;
  cyclic event/drive schemas and `sendMail` remain explicit deferrals.
- OpenAI: stored response get/input reads plus files and batches collections; omit string cursor and
  expansion queries until structured query encoding exists.
- Twilio: recording list/get, usage records and conferences; message/call creation remains visible as
  deferred until structured form encoding exists, with `send_external` and high risk still required.

Each provider story owns only its provider file, vendored source/provenance/script, provider-specific
tests and per-provider generated artifacts. The coordinator alone regenerates whole-catalogue files,
raises response coverage fences, closes the stories and changes changelogs.

### Public operation provenance

Vendoring a document proves where the connector came from to a repository reviewer, but a public
catalogue consumer must also be able to distinguish an operation selected from that document from an
inline operation beside it. Service-level inference is insufficient: Zendesk Support and Messaging
deliberately mix both front ends.

Every public catalogue operation therefore carries a `spec_source` key. It is `null` for an inline
operation and an object for a selected operation, containing the vendor `operation_id`, public
`source_url`, `upstream_version`, and SHA-256 of the committed document. The value is derived while
the patch is applied, never authored as an independent claim. Repository-local paths, fetch times,
story ids, and internal planning stay out of the public document.

## Delivery and release order

1. Inventory exact source commits, operation ids, parameters, response models and known ingest gaps.
2. Implement the five providers in parallel after the inventory freezes their disjoint write sets.
3. Confirm each provider's scoped fixed point, then regenerate the whole catalogue once over the
   integrated wave and run the full Rust/web gates. A full build is coordinator-owned and sees every
   provider; repeating it between already-shared disjoint provider edits adds no independent proof.
4. Publish the per-operation source distinction through C-481 before claiming the public catalogue
   is spec-aware.
5. This wave adds no provider, so it gets one ordinary release after integration. Any future story
   that creates a new `providers/<id>.toml` gets its own release immediately after its integration,
   per the operator instruction.
