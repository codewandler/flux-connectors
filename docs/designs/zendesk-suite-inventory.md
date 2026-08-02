# Zendesk suite operation and inbound inventory

**Status:** implementation boundary for C-461 through C-465, measured 2026-08-02 · **Epic:**
`zendesk-suite` · **Owner:** [C-460](../stories/C-460-curate-zendesk-suite-inventory.md) ·
**Companion:** [zendesk-suite.md](zendesk-suite.md)

## Decision

Use Zendesk's first-party OpenAPI documents as the source for HTTP shape, but never as the selection
policy. Vendor pinned copies, select the bounded sets below, and apply explicit patches where an
official document is incomplete or where a parameter is unsafe for the current emitter. Official
prose is the source only for the Webhooks API, event schemas, and facts an OAS omits.

This inventory uses three outcomes:

- **Carry** — in the named implementation story's first tranche. A qualification in the row is part
  of the decision, not optional advice.
- **Withhold** — must not become a connector operation because it returns a credential, accepts a
  password/session secret as an ordinary model-visible argument, or crosses another fail-closed
  boundary.
- **Defer** — legitimate API surface outside this tranche or blocked by a named capability. A defer
  is not an invitation to expose the operation with a narrower contract than the vendor requires.

## Sources and reproducible measurements

The first three sources are the bytes C-459 vendors. The fourth is prose because Zendesk publishes
the Webhooks API and event schemas as reference pages, not as a separate downloadable OAS found in
this review.

| label | first-party source | role |
|---|---|---|
| T | [Ticketing OAS](https://developer.zendesk.com/zendesk/oas.yaml) | Support, custom objects, incremental exports |
| H | [Help Center OAS](https://developer.zendesk.com/help_center/oas.yaml) | Guide/Help Center |
| M | [Sunshine Conversations OAS](https://github.com/zendesk/sunshine-conversations-api-spec/blob/master/openapi.yaml) | Messaging |
| W | [Webhooks API](https://developer.zendesk.com/api-reference/webhooks/webhooks-api/webhooks/), [creating and monitoring](https://developer.zendesk.com/documentation/webhooks/creating-and-monitoring-webhooks/), [request anatomy](https://developer.zendesk.com/documentation/webhooks/anatomy-of-a-webhook-request/), [event types](https://developer.zendesk.com/api-reference/webhooks/event-types/webhook-event-types/), [verification](https://developer.zendesk.com/documentation/webhooks/verifying/) | prose-only webhook administration, setup, and ingress |

Measured from the live official bytes in this session:

```text
$ for url in https://developer.zendesk.com/zendesk/oas.yaml \
    https://developer.zendesk.com/help_center/oas.yaml; do
    curl -fsSL "$url" | yq -o=json '.' |
      jq -r '[.openapi,.info.title,.info.version,
              ((.servers // [])|map(.url)|join(",")),(.paths|length),
              ([.paths[]|to_entries[]|
                select(.key|IN("get","post","put","patch","delete"))]|length)]|@tsv'
  done
3.0.3  Support API      2.0.0  https://{subdomain}.{domain}.com  434  625
3.0.2  Help Center API  2.0.0                                      119  181

$ curl -fsSL https://raw.githubusercontent.com/zendesk/sunshine-conversations-api-spec/master/openapi.yaml |
    yq -o=json '.' |
    jq -r '[.openapi,.info.title,.info.version,
            ((.servers // [])|map(.url)|join(",")),(.paths|length),
            ([.paths[]|to_entries[]|
              select(.key|IN("get","post","put","patch","delete"))]|length)]|@tsv'
3.0.2  Sunshine Conversations API  17.12.1  https://{subdomain}.zendesk.com/sc,https://api.smooch.io,https://api.eu-1.smooch.io  42  68

$ curl -fsSL https://developer.zendesk.com/zendesk/oas.yaml | sha256sum
70ba1bca3285ab5c42d0b3d875deadbaf48c3f3b568fc2f04c53886f21799b97  -
$ curl -fsSL https://developer.zendesk.com/help_center/oas.yaml | sha256sum
45e7b9bf50d1c2d569e6f8ec5bda5372483ae8382636935f524c1fb590b7c4e9  -
$ curl -fsSL https://raw.githubusercontent.com/zendesk/sunshine-conversations-api-spec/master/openapi.yaml | sha256sum
f6353fbc041f772b448ab6cdb3f49389f6583a30e5c7f0564b291ea63b98444b  -
```

Those hashes identify what was inspected; C-459 must re-fetch and record both upstream and vendored
hashes rather than copying these into provenance. The documents are mutable URLs.

W has no path in the pinned Ticketing document and no first-party downloadable OAS found in this
review. Re-measured on 2026-08-02: the first command and the reference-page link scan produced no
output, while all three plausible first-party document locations returned 404. The reference pages,
not an inferred or scraped specification URL, are therefore C-465's source.

```text
$ rg -n '^  /api/v2/webhooks' specs/zendesk/ticketing-2026-08-02.openapi.yaml

$ for url in \
    https://developer.zendesk.com/webhooks/oas.yaml \
    https://developer.zendesk.com/webhook/oas.yaml \
    https://developer.zendesk.com/api-reference/webhooks/oas.yaml; do
    curl -L -sS -o /dev/null -w '%{http_code} %{url_effective}\n' "$url"
  done
404 https://developer.zendesk.com/webhooks/oas.yaml
404 https://developer.zendesk.com/webhook/oas.yaml
404 https://developer.zendesk.com/api-reference/webhooks/oas.yaml

$ lynx -dump -listonly \
    https://developer.zendesk.com/api-reference/webhooks/webhooks-api/webhooks/ |
    rg -i '(yaml|oas|openapi|swagger)'
```

The current repository boundary was also re-measured rather than taken from an earlier story:

```text
$ rg -c '^\[\[operations\]\]' providers/zendesk.toml
7
$ rg -c '^\[\[services\]\]' providers/zendesk.toml
1
$ rg -c '^\[\[config\]\]' providers/zendesk.toml
3
$ rg -c '^\[\[events\]\]' providers/zendesk.toml || true
$ rg -c '^\[\[channels\]\]' providers/zendesk.toml || true
```

The one service entry is metadata-only `default`; the last two commands produced no output. C-458
owns preserving that default service's published addresses while named services are added.

## Shared service, authentication, and configuration boundary

| service | base URL and version | authentication | configuration |
|---|---|---|---|
| Support (`default`) | `https://{subdomain}.zendesk.com`, paths under `/api/v2` | existing Basic `{email}/token:{api_token}` | existing `subdomain`, API-token secret, and user/email half |
| `help-center` | same account host, `/api/v2/help_center` | same Support credential | reuse the same subdomain and credential; do not ask twice |
| `messaging` | `https://{subdomain}.zendesk.com/sc`, paths under `/v2` | distinct Basic `{key_id}:{key_secret}` | `subdomain`, `app_id`, non-secret key id, secret key value; do not reuse the Support token |
| `webhooks` | same account host, `/api/v2/webhooks` | same Support credential for administration; separate inbound signing secret | callback URL is supplied by the host through `Subscription.callback_param`, never provider config |

T and H declare root `basicAuth`. M declares Basic and bearer alternatives with operation scopes;
[Zendesk's authentication guide](https://developer.zendesk.com/documentation/conversations/getting-started/api-authentication/)
says Basic is preferred and spells it as `base64(key_id:key_secret)`. Use that one supported,
declarable route in the first tranche. JWT minting is acquisition/rotation work, not an operation.

The media census was produced with:

```text
$ for url in \
    https://developer.zendesk.com/zendesk/oas.yaml \
    https://developer.zendesk.com/help_center/oas.yaml \
    https://raw.githubusercontent.com/zendesk/sunshine-conversations-api-spec/master/openapi.yaml; do
    curl -fsSL "$url" | yq -o=json '.' |
      jq -r '([.paths[]|to_entries[]|
        select(.key|IN("get","post","put","patch","delete"))|
        .value.requestBody.content? // {}|keys[]]|unique|join(","))'
  done
application/json,application/x-www-form-urlencoded,multipart/form-data
application/json
application/json,multipart/form-data
```

Every carried write below is JSON. Multipart uploads are deferred under C-426. No implementation
may silently reinterpret multipart as a string body.

## C-461 — Support foundations

C-6 first reproduces the seven existing operations from T without changing their public ids or
rendered behavior. C-461 adds exactly the rows below; it does not select a resource family wholesale.

| decision | operationId · method/path | contract and implementation evidence |
|---|---|---|
| Defer | `CreateTicket` · `POST /api/v2/tickets` | T omits the documented `Idempotency-Key`, required request-body constraints, and the audit member of the official response. C-6 can eventually add the missing header, but its acceptance has no response-schema override and the current overlay cannot add even the header. Carry only after one mechanism can correct the whole contract; without the key the write is medium/non-idempotent. |
| Carry | `ListRecentTickets` · `GET /api/v2/tickets/recent` | Bounded read (vendor says at most the recently viewed/created set); low/idempotent; JSON response. |
| Carry | `ListAuditsForTicket` · `GET /api/v2/tickets/{ticket_id}/audits` | Low/idempotent. Explicitly omit all seven optional query parameters: `page`, `sort`, `include`, `include_boundary_indicators`, `include_item_cursors`, `filter_events`, and `sort_order`. The response's pagination metadata remains part of its schema even though request pagination arguments are absent. |
| Carry | `ListTicketsFromView` · `GET /api/v2/views/{view_id}/tickets` | Low/idempotent. Omit `sort_by` and `sort_order`; both are optional query strings and C-30 still owns encoded query composition. The source integer-or-string union lowers to uncallable Flux `Any`, so C-466 narrows `view_id` to the documented built-ins `incoming`, `my`, and `my_groups`; account-specific numeric views remain deferred. |
| Carry | `ShowUser` · `GET /api/v2/users/{user_id}` | Numeric path id; low/idempotent; JSON response. Omit optional query `include`. |
| Withhold | `CreateOrUpdateUser` · `POST /api/v2/users/create_or_update` | The required `user` is `anyOf(UserCreateInput, UserMergeInput)`: both expose model-visible `password`, while `UserMergeInput` requires neither stable `email` nor `external_id` and admits `{user:{}}`. The overlay cannot remove a nested secret-shaped field or strengthen this union, so it cannot publish the conditional write honestly. |
| Carry | `ShowOrganization` · `GET /api/v2/organizations/{organization_id}` | Numeric path id; low/idempotent. Omit optional queries `include`, `include_boundary_indicators`, and `include_item_cursors`. |
| Defer | `CreateOrUpdateOrganization` · `POST /api/v2/organizations/create_or_update` | Official prose defines the JSON body and stable `id`/`external_id` matching, while T declares 200/201 JSON responses but no request body. The current overlay corrects only parameters the document already declares; it cannot add this body. Medium/conditional once that contract is representable. |
| Carry | `ListGroups` · `GET /api/v2/groups` | Low/idempotent; JSON response. Omit `exclude_deleted`, `include`, `page`, `per_page`, `sort`, `include_boundary_indicators`, and `include_item_cursors`. |
| Carry | `ListTicketFields` · `GET /api/v2/ticket_fields` | Low/idempotent. Omit `locale`, `creator`, `page`, `sort`, `include_boundary_indicators`, and `include_item_cursors`. |
| Carry | `ListTicketForms` · `GET /api/v2/ticket_forms` | Low/idempotent. Omit `active`, `end_user_visible`, `fallback_to_default`, `form_type`, `associated_to_brand`, `page`, `per_page`, `sort`, `include_boundary_indicators`, `include_item_cursors`, and `locale`; a plain inventory is the bounded requirement. |
| Carry | `ListCustomStatuses` · `GET /api/v2/custom_statuses` | Low/idempotent. Omit `status_categories`, `active`, and `default`; the vendor documents no pagination. |

The patch must explicitly omit each optional parameter under C-422. It must not rely on the current
emitter happening to produce a tolerable URL for one example value. T's ticketing introduction—not
the OAS—documents the ticket-create idempotency key. That prose remains evidence for a future
correction, but it is not enough to carry an operation whose request and response contracts are both
incomplete in T.

The audit omission list above was resolved from T's component references, not counted by eye:

```text
$ curl -fsSL https://developer.zendesk.com/zendesk/oas.yaml | yq -o=json '.' |
    jq -r '. as $root |
      [$root.paths["/api/v2/tickets/{ticket_id}/audits"].get.parameters[] |
       if has("$ref") then .["$ref"] | split("/") | last |
         $root.components.parameters[.] else . end | .name] | join(", ")'
page, sort, include, include_boundary_indicators, include_item_cursors, filter_events, sort_order
```

Defer ticket bulk jobs, imports, spam/merge/redaction, deleted-ticket administration, password and
session endpoints, identity verification, and attachment upload. They are not needed for the first
foundation surface and several introduce asynchronous jobs, multipart, irreversible actions, or
credential/session-shaped data. Existing ticket update/comment/tag operations remain governed by
C-6 and are not duplicated here.

## C-462 — custom data and synchronization

The incremental selection deliberately uses time-based integer cursors. Opaque cursor strings are
not selected while C-30's query encoder remains open.

| decision | operationId · method/path | contract and implementation evidence |
|---|---|---|
| Carry | `IncrementalTicketExportTime` · `GET /api/v2/incremental/tickets` | Required integer `start_time`; low/idempotent. Omit optional string `support_type_scope`; retain only integer-safe parameters. |
| Carry | `IncrementalUserExportTime` · `GET /api/v2/incremental/users` | Required integer `start_time`; low/idempotent; optional integer `per_page` is safe. |
| Carry | `IncrementalOrganizationExport` · `GET /api/v2/incremental/organizations` | Required integer `start_time`; low/idempotent; optional integer `per_page` is safe. |
| Carry | `IncrementalTicketEvents` · `GET /api/v2/incremental/ticket_events` | Required integer `start_time`; low/idempotent. Omit optional strings `support_type_scope` and `include`. |
| Carry | `ListCustomObjects` · `GET /api/v2/custom_objects` | Low/idempotent. Retain optional boolean `include_ui_path`; the pinned document was re-read on 2026-08-02 after the original inventory incorrectly called it a string. |
| Defer | `ListCustomObjectRecords` · `GET /api/v2/custom_objects/{custom_object_key}/records` | T gives `custom_object_key` no closed pattern and the current path emitter does not encode a free-form segment. Request query omissions alone cannot make the path safe; carry only after composition validates or encodes the key. |
| Defer | `ShowCustomObjectRecord` · `GET /api/v2/custom_objects/{custom_object_key}/records/{custom_object_record_id}` | Same unbounded `custom_object_key` path segment. The numeric record id does not cure the preceding ambiguous segment. |
| Defer | `CreateCustomObjectRecord` · `POST /api/v2/custom_objects/{custom_object_key}/records` | JSON body/response, but the unbounded key makes the path unsafe. Medium/non-idempotent once the path contract is representable. |
| Defer | `UpdateCustomObjectRecord` · `PATCH /api/v2/custom_objects/{custom_object_key}/records/{custom_object_record_id}` | The path has the same unsafe key, and T also declares a JSON response but no request body. The current overlay cannot add that body. |
| Defer | `UpsertCustomObjectRecordByExternalIdOrName` · `PATCH /api/v2/custom_objects/{custom_object_key}/records` | JSON body/response and conditional on stable external id or name, but the key path segment has no safe closed contract in T. |
| Defer | `FilteredSearchCustomObjectRecords` · `POST /api/v2/custom_objects/{custom_object_key}/records/search` | Search criteria travel in JSON, but the key path segment remains unsafe. If carried later, omit optional `query`, `sort`, and cursor strings; retain at most integer `page[size]`, and do not declare the POST cacheable without a body-aware cache-key rule. |

The path decision follows the schema T actually gives that name:

```text
$ curl -fsSL https://developer.zendesk.com/zendesk/oas.yaml | yq -o=json '.' |
    jq -c '[.. | objects | select(.name? == "custom_object_key") | .schema] | unique'
[{"type":"string"}]
```

There is no pattern or closed enumeration to turn the raw string into a safe path contract.

Defer cursor-based incremental exports, autocomplete/search GET endpoints with free string query
values, record attachments (multipart/binary), custom-object schema/access-rule/trigger mutation,
and bulk jobs. This keeps C-462 a data/sync surface rather than an administrator for the object model.

## C-463 — Help Center

Use the no-locale read endpoints so callers are not forced to interpolate a free-form locale into
every path. Translation operations still carry the locale where it is semantically required.

| decision | operationId · method/path | contract and implementation evidence |
|---|---|---|
| Carry | `ListCategoriesNoLocale` · `GET /api/v2/help_center/categories` | Low/idempotent; omit optional string sort parameters. |
| Carry | `ListSectionsNoLocale` · `GET /api/v2/help_center/sections` | Low/idempotent; omit optional string sort parameters. |
| Carry | `ListArticlesNoLocale` · `GET /api/v2/help_center/articles` | Low/idempotent; omit `sort_*`, `label_names`, and other string filters. |
| Carry | `ShowArticleNoLocale` · `GET /api/v2/help_center/articles/{article_id}` | Numeric article id; low/idempotent; JSON response. |
| Carry | `ListTranslations` · `GET /api/v2/help_center/articles/{article_id}/translations` | Low/idempotent. Omit optional locale/filter query values. |
| Carry | `ListArticlesIncremental` · `GET /api/v2/help_center/incremental/articles` | Retain integer `start_time`; omit string sort/label filters. Low/idempotent. |
| Carry | `CreateArticleBySection` · `POST /api/v2/help_center/sections/{section_id}/articles` | JSON body/response; high/non-idempotent because it publishes externally visible content. |
| Defer | `UpdateArticleNoLocale` · `PUT /api/v2/help_center/articles/{article_id}` | The pinned H document declares a 200 JSON response but no `requestBody` (re-measured 2026-08-02). The overlay cannot add the prose-defined target-state body; high/conditional once representable. |
| Defer | `CreateTranslation` · `POST /api/v2/help_center/articles/{article_id}/translations` | H declares a 201 JSON response but no request body. The current overlay cannot add the prose-defined body; high/non-idempotent once representable. |
| Defer | `UpdateTranslation` · `PUT /api/v2/help_center/articles/{article_id}/translations/{locale}` | H declares a 200 JSON response but no request body, and the current overlay cannot add it. A future carry must also validate locale as a BCP-47-like path segment and refuse `/`, `?`, and `#`; high/conditional. |

Defer article search/embeddable search until query encoding is structural, all attachments until
multipart lands, comments/votes/subscriptions as a separate community surface, and archive/delete
operations until a concrete destructive workflow asks for them.

## C-464 — Messaging

M is the only source here that declares per-operation auth scopes. The first tranche uses an
app-scoped Basic key because it covers the selected conversation and user operations. Publish the required
scope per operation when C-443's consumer exists; until then preserve it in provider comments and
tests rather than inventing an unused field.

Two approved message operations cross the finite OpenAPI IR's boundary. Both response schemas reach
the cycle `message -> quotedMessage -> quotedMessageMessage -> message`, so ingest skips them rather
than expanding forever. `PostMessage` and `ListMessages` therefore use the supported mixed front-end:
their wire contracts and deliberately bounded response members are transcribed from M, while the
seven cycle-free operations remain exact patches. A provider-scoped negative test pins the ingest
diagnostic and keeps those two patches absent until recursive-schema support exists.

| decision | operationId · method/path | contract and implementation evidence |
|---|---|---|
| Carry | `CreateConversation` · `POST /v2/apps/{appId}/conversations` | JSON; high/non-idempotent; app id is service config. |
| Defer | `ListConversations` · `GET /v2/apps/{appId}/conversations` | Its required `filter` is a `deepObject` query containing `userId` or `userExternalId`. The current query model/emitter cannot reproduce that shape safely. |
| Carry | `GetConversation` · `GET /v2/apps/{appId}/conversations/{conversationId}` | Low/idempotent; JSON response. |
| Carry | `UpdateConversation` · `PATCH /v2/apps/{appId}/conversations/{conversationId}` | Medium/non-idempotent under Flux's cache-safe vocabulary; narrowed to a required absolute `displayName`, whose request replays byte-identically; JSON. |
| Carry | `ListParticipants` · `GET /v2/apps/{appId}/conversations/{conversationId}/participants` | Low/idempotent. Omit optional deep-object cursor pagination in the first tranche. |
| Carry | `PostMessage` · `POST /v2/apps/{appId}/conversations/{conversationId}/messages` | High/non-idempotent and externally visible. JSON body/response. |
| Carry | `ListMessages` · `GET /v2/apps/{appId}/conversations/{conversationId}/messages` | Low/idempotent. Omit optional deep-object cursor pagination. |
| Carry | `CreateUser` · `POST /v2/apps/{appId}/users` | Medium/non-idempotent; JSON body/response. |
| Carry | `GetUser` · `GET /v2/apps/{appId}/users/{userIdOrExternalId}` | Low/idempotent. The path value is free-form; refuse `/`, `?`, and `#` rather than emitting an ambiguous path. |
| Carry | `UpdateUser` · `PATCH /v2/apps/{appId}/users/{userIdOrExternalId}` | Medium/non-idempotent under Flux's cache-safe vocabulary; narrowed to required absolute `toBeRetained`, whose request replays byte-identically; same path constraint; JSON. |
| Withhold | `CreateWebhook` · `POST /v2/apps/{appId}/integrations/{integrationId}/webhooks` | Its 201 `webhookResponse.webhook.secret` is the live signing credential. C-430 requires refusal rather than ordinary response projection. |
| Withhold | `ListWebhooks` · `GET /v2/apps/{appId}/integrations/{integrationId}/webhooks` | Its 200 `webhookListResponse.webhooks[].secret` returns every live signing credential. |
| Withhold | `GetWebhook` · `GET /v2/apps/{appId}/integrations/{integrationId}/webhooks/{webhookId}` | Its 200 `webhookResponse.webhook.secret` returns the live signing credential. |
| Withhold | `UpdateWebhook` · `PATCH /v2/apps/{appId}/integrations/{integrationId}/webhooks/{webhookId}` | Its 200 response returns the live signing credential, regardless of the bounded update body. |
| Defer | `DeleteWebhook` · `DELETE /v2/apps/{appId}/integrations/{integrationId}/webhooks/{webhookId}` | Destructive/conditional and the only response-safe member of a family whose discovery/create/update operations are withheld. Do not publish an orphaned destructive call. |

Withhold `CreateAppKey`, app/integration key reads, and `/oauth/token`: their purpose or response is
a credential, which C-430 forbids returning as an ordinary operation value. Also withhold client
creation if its response mints a client credential. Defer app provisioning, integrations,
switchboards/control transfer, device/client administration, conversion events, personal-data
deletion, and all attachments. They are separate workflows; attachments are multipart.

The four webhook-response findings above were re-measured from M on 2026-08-02. In each named
schema, `secret` is a string described as the webhook secret used to verify incoming-request origin;
this is the same signing credential class the Support webhook inventory withholds explicitly.

## C-465 — Support webhooks and inbound events

### Administration operations

W documents the five ordinary lifecycle requests below exactly. They are an accounting set, not a
surface to publish: the generic Webhook JSON representation names an optional `signing_secret`, even
though the list, show, and create examples omit it. This repository returns an operation's raw
response, so deleting that property from `response_schema` would delete the disclosure and leave the
exposure. Without an authoritative response contract that rules the credential out, those three
operations fail C-430. The response-safe update and delete calls stay out too: they cannot discover
or create the resource and would publish an orphaned write-only lifecycle.

| decision | method/path | contract |
|---|---|---|
| Withhold | `GET /api/v2/webhooks` | Low/idempotent. Its six optional string filters/cursors would be omitted, but each generic Webhook result may carry `signing_secret`. |
| Withhold | `GET /api/v2/webhooks/{webhook_id}` | Low/idempotent. The generic Webhook representation may carry `signing_secret`. |
| Withhold | `POST /api/v2/webhooks` | High/non-idempotent. The restricted request would nest endpoint, method, format, name, status, and subscriptions under `webhook`, omitting `clone_webhook_id`, destination `authentication`, `signing_secret`, and arbitrary custom headers; its 201 generic Webhook response still has the unresolved credential field. |
| Withhold with family | `PUT /api/v2/webhooks/{webhook_id}` | High/conditional. The restricted JSON PUT returns 204, but it is not useful or safe to publish without discovery/create. |
| Withhold with family | `DELETE /api/v2/webhooks/{webhook_id}` | Destructive/non-idempotent. It returns 204, but Zendesk states no repeat guarantee; a later 404 and the final absent state do not license an automatic retry. Publishing only the destructive teardown call would also leave an orphaned lifecycle. |
| Defer | `POST /api/v2/webhooks/test` | It makes Zendesk call an arbitrary destination and accepts a stringified payload plus optional secret-bearing destination authentication. This is not the bounded subscription operation. |
| Defer | invocation and attempt reads | Useful observability, but the list takes free-form filters/cursors and is not needed to connect/disconnect. |
| Withhold | `GET /api/v2/webhooks/{webhook_id}/signing_secret` | Its 200 response is the live signing credential. C-430 requires refusal, not documentation of the hazard. |
| Withhold | `POST /api/v2/webhooks/{webhook_id}/signing_secret` | Reset returns the newly generated live signing credential. It is credential rotation, not an ordinary operation result. |

Destination `authentication` accepts API-key, Basic, and bearer secret values inside the request;
none may become an ordinary model-visible parameter. `custom_headers` is also omitted from the
restricted design because its keys and values are arbitrary. The provider stores neither those
values nor a deployment URL.

There is consequently no `webhooks` service and no `Subscription` declaration in C-465. Even if the
three response contracts were made safe, today's `Subscription` can inject only the callback URL. It
cannot map selected event wire values into `subscriptions`, fill the other required create fields,
or acquire and store the signing secret Zendesk generates only after creation. [C-480](../stories/C-480-provision-webhook-subscriptions-and-secrets.md)
owns that complete lifecycle rather than publishing a declaration that a host cannot finish.

### Verification is expressible; event naming is not

W states the signature as `base64(HMACSHA256(TIMESTAMP + BODY))`, carried in
`X-Zendesk-Webhook-Signature`, with the RFC 3339 timestamp in
`X-Zendesk-Webhook-Signature-Timestamp`. The repository already models that exact row:

```text
$ rg -n 'Zendesk.*base64.*timestamp.*body' crates/connector-spec/src/inbound.rs
138:/// | Zendesk | `X-Zendesk-Webhook-Signature` | sha256 | base64 | `{timestamp}{body}` | tolerance |
```

So Zendesk does not need a new signature algorithm. A future declaration must choose a local
anti-replay tolerance explicitly—the vendor page explains replay resistance but gives no acceptance
window—and label that duration as connector policy, not a vendor fact. Delivery id is body `id`;
discriminator is body `type`; payload schema starts with the official common envelope (`account_id`,
`detail`, `event`, `id`, `subject`, `time`, `type`, `zendesk_event_version`). C-465 records this
representability but emits no verification block because there is no honest channel to attach it to.

The blocker is name fidelity. Official discriminator values include
`zen:event-type:ticket.created`, while the member-address grammar admits lowercase letters, digits,
`-`, `_`, and `.`, not `:`:

```text
$ sed -n '338,360p' crates/connector-address/src/address.rs
pub fn validate_member_name(name: &str) -> Result<(), String> {
    ...
    if !name.chars().all(|c| is_segment_char(c) || c == '.' || c == '_') {
        ...
```

`EventDecl` simultaneously promises to retain the vendor name. Therefore **withhold all inbound
event and channel declarations** until
[C-479](../stories/C-479-preserve-event-wire-discriminators.md) adds a lossless wire discriminator
value distinct from the local address name. It must not silently rename
`zen:event-type:ticket.created` to `ticket.created` and call that fidelity. C-480 then owns the
subscription and secret-provisioning half; both have to land before verified Zendesk ingress exists.

Once that conflict is resolved, the bounded first event set is:

- `zen:event-type:ticket.created`, `.status_changed`, `.comment_added`, and
  `.agent_assignment_changed`;
- `zen:event-type:user.created` and `.deleted`;
- `zen:event-type:organization.created` and `.deleted`;
- `zen:event-type:article.published` and `.unpublished`;
- `zen:event-type:messaging_ticket.message_added`.

The remaining official event domains and event types defer. Ticket events also carry
`event.meta.sequence`; Zendesk says ordering inside a sequence is not guaranteed, so no flow may
interpret `position` as a delivery-order guarantee.

## Explicitly outside this epic

| surface | decision and evidence |
|---|---|
| Talk / Omnichannel | Defer to a separate inventory. [Talk](https://developer.zendesk.com/api-reference/voice/talk-api/introduction/) is part of v2 and the page offers a Postman collection, not a downloadable OAS link observed here; real-time agent availability, queues, callbacks, and call statistics have distinct rate/operational semantics. Ticketing's few voice paths do not justify selecting the domain by accident. |
| AI Agents | Defer to its own charter/inventory. The [official introduction](https://developer.zendesk.com/api-reference/ai-agents/introduction/) requires the Advanced add-on and warns that its APIs do not follow normal Zendesk conventions. Conversation execution and data export need a cost/runtime decision before endpoint selection. |
| Sell | Out of this provider epic. [Sell](https://developer.zendesk.com/api-reference/sales-crm/introduction/) uses `https://api.getbase.com`, OAuth 2.0, and separate Core, Sync, Firehose, and Search APIs. It is a distinct product/auth/host boundary worthy of its own provider decision, not another Zendesk service added for brand similarity. |
| Legacy Chat Conversations | Withhold. The [official page](https://developer.zendesk.com/api-reference/live-chat/chat-conversations-api/conversations-api/) says maintenance mode since 2022, no new integrations after 2025-04-30, and directs Messaging customers to Sunshine Conversations. It is GraphQL over HTTP plus a stateful WebSocket, not the OAS-backed HTTP surface this epic is proving. |

## Implementation gates

Each provider story must turn its table into an exact selector list and run the scoped gate from
`AGENTS.md`. In addition:

- operations whose OAS omits a required request body or whose whole response contract cannot be
  corrected remain deferred; C-6's current patch corrects only parameters the document declares;
- every omitted query/body field is named in `omit`, never dropped heuristically;
- every carried path string gets either a vendor-documented grammar or a refusal test for delimiters;
- every emitted write states `risk` and `idempotency` from the decision above;
- `connector_pack::Rehearsal` must compose every selected operation from declared config;
- credential-returning and multipart families remain named in negative/accounting tests rather than
  disappearing from both the provider and the proof.

C-14 remains the owner of network refresh and upstream drift. This inventory neither adds network IO
to `build` nor turns a mutable URL into a build input.
