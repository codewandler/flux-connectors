# Five popular providers — frozen OpenAPI inventory

**Measured 2026-08-02 for C-468.** This is the dispatch contract for C-469 through C-473. It
freezes the upstream bytes, the exact operation selections, the current public names, and the
provider-owned write sets before those stories run in parallel. OpenAPI is evidence; none of the
operation totals below is permission to expose a document wholesale.

## 1. How the measurements were made

The current catalogue baseline and the hand-authored provider files agree:

```text
$ for p in github stripe microsoft_graph openai twilio; do
>   printf '%s\t' "$p"; rg -c '^\[\[operations\]\]' "providers/$p.toml"
> done
github          5
stripe          8
microsoft_graph 8
openai          4
twilio          5

$ jq -r '.providers[] |
>   select(.id == "github" or .id == "stripe" or .id == "microsoft_graph" or
>          .id == "openai" or .id == "twilio") |
>   [.id,.operation_count] | @tsv' web/public/catalog.json | sort
github          5
microsoft_graph 8
openai          4
stripe          8
twilio          5
```

The source commits were read, not copied from a note:

```text
$ git ls-remote https://github.com/github/rest-api-description.git HEAD
5e28810649ba41b5483753ba74f976f83856a504  HEAD
$ git ls-remote https://github.com/stripe/openapi.git HEAD
8da624f9b4f65178eb2e2c2b6fc80162a6c0dceb  HEAD
$ git ls-remote https://github.com/microsoftgraph/msgraph-metadata.git HEAD
60b50e2e5b23612aac74ecdf65d35d566c5a4031  HEAD
$ git ls-remote https://github.com/openai/openai-openapi.git HEAD
117ce5680e4269f6656a4fd70d28f9755630d938  HEAD
$ git ls-remote https://github.com/twilio/twilio-oai.git HEAD
97418cf0e4d6cf35b02333dd624090a8c62fa25d  HEAD
```

Every raw URL below includes that immutable commit. The implementation scripts may also expose a
friendly moving URL for checking upstream drift, but the vendored bytes and provenance are pinned
to these identities.

## 2. Upstream source ledger

| provider | exact first-party input | OpenAPI / upstream version | bytes | paths / operations | SHA-256 |
|---|---|---:|---:|---:|---|
| GitHub | [`api.github.com.2022-11-28.json`](https://raw.githubusercontent.com/github/rest-api-description/5e28810649ba41b5483753ba74f976f83856a504/descriptions/api.github.com/api.github.com.2022-11-28.json) | 3.0.3 / 1.1.4 | 12,815,453 | 805 / 1,216 | `281dc9e4ab24860c4010cea1bc90232175a6c92aa73dc569e1ccea6a5018cae9` |
| Stripe | [`latest/openapi.spec3.json`](https://raw.githubusercontent.com/stripe/openapi/8da624f9b4f65178eb2e2c2b6fc80162a6c0dceb/latest/openapi.spec3.json) | 3.0.0 / 2026-07-29.dahlia | 4,483,103 | 439 / 621 | `6f3623aece40493eec2f5e3e631219f8c6bffa4f477e3807a4bf785ad377f237` |
| Microsoft Graph | [`openapi/v1.0/openapi.yaml`](https://raw.githubusercontent.com/microsoftgraph/msgraph-metadata/60b50e2e5b23612aac74ecdf65d35d566c5a4031/openapi/v1.0/openapi.yaml) | 3.0.4 / v1.0 | 38,050,122 | 10,790 / 16,702 | `2749e51f363a471cdaa4835493c2c57198aa834262666da39c03a2e7f9f9d831` |
| OpenAI | [`openapi.json`](https://raw.githubusercontent.com/openai/openai-openapi/117ce5680e4269f6656a4fd70d28f9755630d938/openapi.json) | 3.1.0 / 2.3.0 | 3,244,309 | 182 / 288 | `ef36175ba6181b9d725a16b1eedcaa75a8a1268124896b10ccc5d9ddf0d543d3` |
| Twilio | [`twilio_api_v2010.json`](https://raw.githubusercontent.com/twilio/twilio-oai/97418cf0e4d6cf35b02333dd624090a8c62fa25d/spec/json/twilio_api_v2010.json) | 3.0.1 / 1.0.0 | 1,869,905 | 121 / 197 | `a6753266b8b05a201e8658734e332ee51d07a0913f2d419335d87bdb287643a2` |

The measurements above came from the fetched bytes in one temporary directory:

```text
$ wc -c github.json stripe.json microsoft-graph.yaml openai.json twilio.json
12815453 github.json
 4483103 stripe.json
38050122 microsoft-graph.yaml
 3244309 openai.json
 1869905 twilio.json

$ jq -r '[.openapi,.info.version,(.paths|length),
>   ([.paths[]|to_entries[]|select(.key|IN("get","put","post","delete","options","head","patch","trace"))]|length)]|@tsv' \
>   github.json stripe.json openai.json twilio.json
3.0.3  1.1.4              805  1216
3.0.0  2026-07-29.dahlia  439  621
3.1.0  2.3.0              182  288
3.0.1  1.0.0              121  197

$ yq '[.openapi,.info.version,(.paths|length),
>   ([.paths[]|to_entries|.[]|select(.key == "get" or .key == "put" or
>     .key == "post" or .key == "delete" or .key == "options" or
>     .key == "head" or .key == "patch" or .key == "trace")]|length)]' microsoft-graph.yaml
- 3.0.4
- v1.0
- 10790
- 16702
```

### 2.1 License posture

- GitHub's document declares MIT and the first-party repository carries an [MIT
  license](https://github.com/github/rest-api-description/blob/5e28810649ba41b5483753ba74f976f83856a504/LICENSE.md).
- Stripe's document has no `info.license`; its first-party repository carries an [MIT
  license](https://github.com/stripe/openapi/blob/8da624f9b4f65178eb2e2c2b6fc80162a6c0dceb/LICENSE).
- Microsoft Graph's document has no `info.license`; its first-party metadata repository carries an
  [MIT license](https://github.com/microsoftgraph/msgraph-metadata/blob/60b50e2e5b23612aac74ecdf65d35d566c5a4031/LICENSE).
- Twilio's document declares Apache-2.0 while the repository carries an [MIT
  license](https://github.com/twilio/twilio-oai/blob/97418cf0e4d6cf35b02333dd624090a8c62fa25d/LICENSE).
  C-473 must retain both notices rather than deciding that one silently supersedes the other.
- OpenAI's document and its first-party repository carry an [MIT
  license](https://github.com/openai/openai-openapi/blob/117ce5680e4269f6656a4fd70d28f9755630d938/LICENSE).

This was checked through GitHub's license endpoint, not inferred from repository ownership:

```text
$ for repo in github/rest-api-description stripe/openapi microsoftgraph/msgraph-metadata \
>   openai/openai-openapi twilio/twilio-oai; do
>   curl -fsSL "https://api.github.com/repos/$repo/license" |
>     jq -r --arg repo "$repo" '[$repo,.license.spdx_id,.html_url] | @tsv'
> done
github/rest-api-description                    MIT  .../LICENSE.md
stripe/openapi                                 MIT  .../LICENSE
microsoftgraph/msgraph-metadata                MIT  .../LICENSE
openai/openai-openapi                         MIT  .../LICENSE
twilio/twilio-oai                              MIT  .../LICENSE
```

The vendor descriptions are authoritative source material: GitHub documents that its REST API is
fully described by this repository and offers bundled 3.0/3.1 variants in its [REST
documentation](https://docs.github.com/en/rest/about-the-rest-api/about-the-openapi-description-for-the-rest-api);
Stripe calls `latest/` the recommended GA public specification in its [repository
README](https://github.com/stripe/openapi/blob/8da624f9b4f65178eb2e2c2b6fc80162a6c0dceb/README.md);
Microsoft says `openapi/` holds Graph OpenAPI descriptions in its [metadata
README](https://github.com/microsoftgraph/msgraph-metadata/tree/60b50e2e5b23612aac74ecdf65d35d566c5a4031);
OpenAI publishes its machine-readable API contract in its official [OpenAPI
repository](https://github.com/openai/openai-openapi/tree/117ce5680e4269f6656a4fd70d28f9755630d938);
and Twilio says its GA documents are kept current and used to validate requests in its [OAI
README](https://github.com/twilio/twilio-oai/tree/97418cf0e4d6cf35b02333dd624090a8c62fa25d).

HubSpot was measured before this freeze but excluded: its first-party spec repository and documents
declare no redistribution license. Public availability was not treated as permission to copy bytes
into this public repository when the licensed OpenAI source provides a safe fifth provider.

## 3. Exact expansion selections

The tables below name exact `operationId` values. Each becomes one explicit
`[[patch.operations]]`; a prefix or document-wide selector is forbidden. `Keep query` is the whole
caller-visible query surface. Every other optional query parameter in the source is listed in that
operation's `omit.query` entry. Required path/body members cannot be omitted.

### 3.1 GitHub — C-469

All four are reads: `risk = "low"`, `idempotency = "idempotent"`, no request body, service
`default`. Keep only integer pagination, whose decimal rendering cannot inject a query pair.

| exact `operationId` → public id | method and path | required parameters | keep query | 200 JSON response |
|---|---|---|---|---|
| `issues/list-for-repo` → `github-issue-list` | `GET /repos/{owner}/{repo}/issues` | path `owner: string`, `repo: string` | `per_page: integer`, `page: integer` | array of `#/components/schemas/issue` |
| `pulls/list-files` → `github-pull-files-list` | `GET /repos/{owner}/{repo}/pulls/{pull_number}/files` | path `owner: string`, `repo: string`, `pull_number: integer` | `per_page`, `page` integers | array of `#/components/schemas/diff-entry` |
| `actions/list-workflow-runs-for-repo` → `github-workflow-run-list` | `GET /repos/{owner}/{repo}/actions/runs` | path `owner: string`, `repo: string` | `per_page`, `page` integers | object containing `workflow_runs: array<workflow-run>` |
| `repos/list-commits` → `github-commit-list` | `GET /repos/{owner}/{repo}/commits` | path `owner: string`, `repo: string` | `per_page`, `page` integers | array of `#/components/schemas/commit` |

The source declares 15, 5, 12 and 10 parameters respectively; the unlisted ones are optional
string/array filters such as `labels`, `branch`, `created`, `sha`, `path`, and date strings. They are
omitted because query values are interpolated without percent-encoding. A temporary scaffold over
the pinned document reported no ingest loss:

```text
$ cargo run -q -p connector-cli -- scaffold github --root "$probe" \
>   --select ':/repos/{owner}/{repo}/actions/runs:GET' | tail -12
#       0  operation(s) the document declares that this pipeline could not read at all
#       0  narrower problem(s) in a document that did not cost the operation
```

Concrete deferrals:

- `repos/compare-commits` is not the advertised fourth operation: its `{basehead}` accepts refs,
  and refs may contain `/`; path parameters are interpolated without segment encoding, so a caller
  can change the path shape.
- `search/issues-and-pull-requests` is withheld because its required free-form `q` is the exact
  string-query injection shape tracked by C-30.
- `actions/create-workflow-dispatch` is withheld from this read slice because it triggers a remote
  workflow, has no JSON response body (204), and needs separate high-risk review of externally
  visible execution.

### 3.2 Stripe — C-470

These four make account and billing reference state visible without moving money. All are
`low`/`idempotent`, service
`default`, and have no required path or body members. Keep only `limit: integer`; omit cursor,
filter, array and object queries.

| exact `operationId` → public id | method and path | required parameters | keep query | 200 JSON response |
|---|---|---|---|---|
| `GetCountrySpecs` → `stripe-country-spec-list` | `GET /v1/country_specs` | none | `limit` | object with `data: array<country_spec>` |
| `GetEvents` → `stripe-event-list` | `GET /v1/events` | none | `limit` | object with `data: array<event>` |
| `GetExchangeRates` → `stripe-exchange-rate-list` | `GET /v1/exchange_rates` | none | `limit` | object with `data: array<exchange_rate>` |
| `GetBillingMeters` → `stripe-billing-meter-list` | `GET /v1/billing/meters` | none | `limit` | object with `data: array<billing.meter>` |

The GA document exposes a normalization prerequisite
requirement: each selected GET declares an optional `application/x-www-form-urlencoded` request body
whose schema is exactly an empty object. Ingest represents that as a free-form form body; lowering
then correctly refuses `UnencodableFormBody`. C-470 therefore vendors the full upstream hash and a
deterministically normalized document that removes only an optional GET form body whose schema is
an exact empty object with `additionalProperties: false`; tests prove no required or non-empty body
is affected.

```text
$ jq '.paths["/v1/events"].get.requestBody' stripe.json
{
  "content": {"application/x-www-form-urlencoded": {
    "schema": {"type":"object","properties":{},"additionalProperties":false},
    "encoding": {}
  }},
  "required": false
}

$ cargo run -q -p connector-cli -- scaffold stripe --root "$probe" \
>   --select ':/v1/events:GET' | tail -12
#       0  operation(s) the selected document declares that this pipeline could not read at all
#       0  narrower problem(s) in a document that did not cost the operation
```

The inventory's first four selectors were withdrawn after implementation re-opened the pinned bytes:
`GetCustomers`, `GetPaymentIntents`, `GetInvoices` and `GetSubscriptions` all fail current ingest on
the same response backedge, `file -> file_link -> file`. The replacement selectors above retain
their official 200 response envelopes without truncating that recursive model. Concrete deferrals:

- Customer, payment-intent, invoice and subscription collections are refused on the measured
  `file -> file_link -> file` cycle; product reads are separately refused on
  `product -> price -> product`. Do not hand-copy a response around either refusal.
- Stripe search endpoints are withheld because their required free-form `query` values are not
  percent-encoded.
- Customer/payment/subscription creation and other unselected POSTs are withheld from this story:
  they can create billable or money-moving state, their form bodies need the structured encoder,
  and retry safety depends on the `Idempotency-Key` header rather than HTTP method.

### 3.3 Microsoft Graph — C-471

The full 38 MB YAML is source evidence, not an ingest input: parsing and eagerly resolving all
16,702 operations is outside a provider story's review/memory boundary. C-471 must vendor a
deterministic reference-closed extraction per target service while recording the full upstream hash:
one `mail` document for `me.ListMessages` and one `calendar` document for the other three rows. A
measured trial closure over all four retained 36 referenced components and current scaffold accepted
all four with zero diagnostics.

The source server is `https://graph.microsoft.com/v1.0`, while the existing provider deliberately
keeps `base_url = "https://graph.microsoft.com"` and its paths begin `/v1.0`. To preserve every
existing Flux byte, extraction materializes the server path prefix onto the four source path keys.
The operation objects remain unchanged, and a test proves
`source_server + source_path == provider_base_url + published_path` for every row.

All four are `low`/`idempotent`, have no required path/body members, and keep only the integer OData
queries `$top` and `$skip`. The wire names remain `$top`/`$skip`; generated Flux symbols normalize
them to `_top`/`_skip`. String `$search`, `$filter`, `$orderby`, `$select`, `$expand`, boolean
`$count`, and `includeHiddenMessages` are omitted.

| exact `operationId` → public id / service | method and path | required parameters | keep query | 2XX JSON response |
|---|---|---|---|---|
| `me.ListMessages` → `microsoft_graph-mail-message-list` / `mail` | source `GET /me/messages`; publish `GET /v1.0/me/messages` | none | `$top`, `$skip` | `microsoft.graph.messageCollectionResponse` |
| `me.outlook.ListMasterCategories` → `microsoft_graph-calendar-category-list` / `calendar` | source `GET /me/outlook/masterCategories`; publish with `/v1.0` prefix | none | `$top`, `$skip` | `microsoft.graph.outlookCategoryCollectionResponse` |
| `me.outlook.supportedTimeZones-5c4f` → `microsoft_graph-calendar-time-zone-list` / `calendar` | source `GET /me/outlook/supportedTimeZones()`; publish with `/v1.0` prefix | none | `$top`, `$skip` | collection of `microsoft.graph.timeZoneInformation` |
| `me.outlook.supportedLanguages` → `microsoft_graph-calendar-language-list` / `calendar` | source `GET /me/outlook/supportedLanguages()`; publish with `/v1.0` prefix | none | `$top`, `$skip` | collection of `microsoft.graph.localeInfo` |

```text
$ cargo run -q -p connector-cli -- scaffold microsoft_graph --root "$probe" \
>   --select '::GET' | tail -16
# 4 operation(s) of `specs/microsoft_graph/v1.json`.
#       0  operation(s) the document declares that this pipeline could not read at all
#       0  narrower problem(s) in a document that did not cost the operation
```

This deliberately replaces the design's first guess after measuring it. A reference-closed trial of
`me.ListEvents`, `me.GetDrive`, and `me.ListDrives` was refused:

```text
# default: GET /me/drive: `$ref` cycle: microsoft.graph.userActivity ->
#   microsoft.graph.activityHistoryItem -> microsoft.graph.userActivity
# default: GET /me/drives: the same cycle
# default: GET /me/events: `$ref` cycle: microsoft.graph.event ->
#   microsoft.graph.calendar -> microsoft.graph.event
```

Those three remain visible deferrals, not hand-authored substitutes. `me.sendMail` is also deferred:
it sends externally supplied recipients/content, returns 202 without a JSON response model, and
requires explicit `send_external`/high-risk approval. Drive coverage therefore remains unchanged in
C-471; the service address and its eight existing operations are still pinned below.

### 3.4 OpenAI — C-472

All four operations are read-only `low`/`idempotent` operations in the existing `default` service.
They have no request body. Keep only integer `limit` queries, whose decimal rendering cannot inject
a second query pair; omit string cursors, ordering, expansion controls and streaming controls.

The source server is `https://api.openai.com/v1`, while the existing provider keeps
`base_url = "https://api.openai.com"` and its paths begin `/v1`. The deterministic extraction
prefixes each selected source path with `/v1`, retains the operation objects and reference-closed
schemas, and proves `source_server + source_path == provider_base_url + published_path` for every
row.

| exact `operationId` → public id | method and path | required parameters | keep query | 200 JSON response |
|---|---|---|---|---|
| `getResponse` → `openai-response-get` | source `GET /responses/{response_id}`; publish `GET /v1/responses/{response_id}` | path `response_id: string` | none | `#/components/schemas/Response` |
| `listInputItems` → `openai-response-input-item-list` | source `GET /responses/{response_id}/input_items`; publish with `/v1` prefix | path `response_id: string` | `limit: integer` | `#/components/schemas/ResponseItemList` |
| `listFiles` → `openai-file-list` | source `GET /files`; publish `GET /v1/files` | none | `limit: integer` | `#/components/schemas/ListFilesResponse` |
| `listBatches` → `openai-batch-list` | source `GET /batches`; publish `GET /v1/batches` | none | `limit: integer` | `#/components/schemas/ListBatchesResponse` |

A scaffold over the pinned document accepted every exact target. The single narrower diagnostic in
the selected path families belongs to the unselected `GET /files/{file_id}/content` string response,
not to any row above:

```text
$ cargo run -q -p connector-cli -- scaffold openai --root "$probe" \
>   --select ':/responses/{response_id}:GET' \
>   --select ':/files:GET' --select ':/batches:GET' | tail -16
#       0  operation(s) the document declares that this pipeline could not read at all
#       1  narrower problem(s) in a document that did not cost the operation
# default: GET /files/{file_id}/content: the lowest 2xx response declares `{"type":"string"}`,
#   which admits every document and so states nothing the absence of a schema does not. It was
#   dropped rather than published, because a schema that constrains nothing counts as coverage
#   while carrying none
```

Concrete deferrals:

- `createResponse` can generate billable content, invoke tools and stream externally visible output;
  its broad optional composite body also exceeds the current honest input projection. It needs a
  focused high-risk generation story rather than arriving with these visibility reads.
- `createFile` is multipart/form-data, which the current compiler does not lower safely.
- `GET /files/{file_id}/content` returns file bytes rather than the catalogue's JSON-oriented
  response contract; current scaffold reports its string response schema as unconstraining.
- `deleteFile`, `cancelBatch`, and organization administration endpoints mutate or expose
  credential-bearing administrative state. They need destructive/high-risk and scope review.
- `getResponse` omits `stream`, `starting_after`, `include` and `include_obfuscation`: streaming
  changes the response to SSE, `starting_after` only paginates an SSE stream, and
  expansion/obfuscation controls widen output beyond the bounded stored-response read.
  `listInputItems` omits `order`, string cursor `after` and `include`; the other collection reads
  omit their string cursors and filters.

### 3.5 Twilio — C-473

All four replacements are read-only `low`/`idempotent` operations. C-473 adds
`also_binds = ["path.AccountSid"]` to the existing `account_sid` configuration field, so the
capitalized path member from the source document is operator-pinned rather than model-supplied. It
does not bind the lowercase `account_sid` used by the five existing inline operations, because doing
so would change the Flux-byte fence below. Keep only integer pagination and explicitly safe boolean
toggles; omit string dates, category/status filters, SIDs and opaque `PageToken`.

The source server is `https://api.twilio.com` and its paths include `/2010-04-01`; the current
provider base already includes that version prefix. The deterministic extracted path keys strip
exactly `/2010-04-01` and prove `source_server + source_path == provider_base_url + published_path`.

| exact `operationId` → public id | method and path | required parameters | keep query | 200 JSON response |
|---|---|---|---|---|
| `ListRecording` → `twilio-recording-list` | source `GET /2010-04-01/Accounts/{AccountSid}/Recordings.json`; publish without the version prefix | configured path `AccountSid` | `PageSize`, `Page`, `IncludeSoftDeleted` | object with `recordings: array<api.v2010.account.recording>` |
| `FetchRecording` → `twilio-recording-get` | source `GET /2010-04-01/Accounts/{AccountSid}/Recordings/{Sid}.json`; publish without the version prefix | configured `AccountSid`; caller path `Sid: string` | `IncludeSoftDeleted` | `api.v2010.account.recording` |
| `ListUsageRecord` → `twilio-usage-record-list` | source `GET /2010-04-01/Accounts/{AccountSid}/Usage/Records.json`; publish without the version prefix | configured `AccountSid` | `PageSize`, `Page`, `IncludeSubaccounts` | object with `usage_records: array<api.v2010.account.usage.usage_record>` |
| `ListConference` → `twilio-conference-list` | source `GET /2010-04-01/Accounts/{AccountSid}/Conferences.json`; publish without the version prefix | configured `AccountSid` | `PageSize`, `Page` | object with `conferences: array<api.v2010.account.conference>` |

Current ingest reported no dropped operation for the document:

```text
$ cargo run -q -p connector-cli -- scaffold twilio --root "$probe" \
>   --select ':/2010-04-01/Accounts/{AccountSid}/IncomingPhoneNumbers:GET' | tail -12
#       0  operation(s) the document declares that this pipeline could not read at all
#       0  narrower problem(s) in a document that did not cost the operation
```

Concrete deferrals:

- `CreateMessage` and `CreateCall` are not downgraded to ordinary POSTs. Their official bodies are
  `application/x-www-form-urlencoded`; current Flux lowering assembles form pairs with `fmt`, so
  caller-supplied message text, destinations and URLs containing `&`, `=` or `+` can change the
  request. They remain deferred until the structured form encoder reaches the pinned flux-lang.
  When they do ship they are `high`, non-idempotent, and `send_external`.
- Date filters such as `DateCreated<`, `DateCreated>`, `StartDate`, and `EndDate`, categorical
  strings, and opaque `PageToken` are omitted for the same missing percent-encoding reason.
- Recording media/content download is not selected: it is a non-JSON response, while the catalogue
  response contract and current host return JSON-oriented schemas/text.

## 4. Existing public-name and Flux-byte fence

C-469 through C-473 copy these rows into failing-first provider-specific tests before changing a
provider. A migration from inline TOML to a spec pointer is correct only if every existing file
keeps the hash recorded here. The command was run against the current worktree:

```text
$ for p in github stripe microsoft_graph openai twilio; do
>   sha256sum "crates/catalog/ops/$p"/*.flux
> done
```

| provider | existing public operation id | emitted Flux SHA-256 |
|---|---|---|
| GitHub | `github-issue-comment-add` | `b9e8d697628f0ac1b39ae6057d51d2e1b9e1f3a3e475ef90fc6a0d32897f8156` |
| GitHub | `github-issue-create` | `d196b03b41d4ebf5b7a833f9f2b00e93b94cba0ed35c001bd298222269015a00` |
| GitHub | `github-issue-get` | `47b2a8dc8932ca5af3109e8e70bc739d77e2614d43ec7417666530fccea4eb69` |
| GitHub | `github-pull-get` | `b6ed8367268345da6400ad8b6696ff155c82c608ee129108bffbca50bd216402` |
| GitHub | `github-repo-get` | `be3c3669255f3caa7e1131a7fee9c0c42407e24cdcacc2b8f78b64d8fbc21183` |
| Stripe | `stripe-balance-get` | `8aa021012daa4a8b769971349c0fc32243ee9d79fd49262347323863e547d4c9` |
| Stripe | `stripe-charge-get` | `437bc81655909aae320492ec4f86b2904ea38fcd3a7bfd36d22f513186116d86` |
| Stripe | `stripe-charge-refund-create` | `cff7fc446cbca6d6847012af000ebf8f820c106a3212d40bc9af2c9ab0064cb1` |
| Stripe | `stripe-customer-get` | `4b6b2cc2595c26400bc1eb21c54b9e1a8dacbcdc6328c90dae3f1f9817ad4b76` |
| Stripe | `stripe-payment-intent-cancel` | `48b5090c48f8316befe576b62375f44e1ce702e20087d6bfb2f9c44ae2a0f290` |
| Stripe | `stripe-payment-intent-capture` | `44a66f1474291b5d30a2eb654917f9b208533801e81016b3dd1b32da8358f295` |
| Stripe | `stripe-payment-intent-get` | `c395f7eb33782f26e7dc7a58f1b2357fedbb8ed4befd51f91fbbc4a248e2c60a` |
| Stripe | `stripe-refund-get` | `520942e8c3df9c6fdccc250d4d4e24c148cc60794bde8550bb4cbe3c8a21124e` |
| Microsoft Graph | `microsoft_graph-calendar-calendar-get` | `412213d36d8e0f5dee58e59c0aaa4c41d9c9db0e15f7030271444080f0f65285` |
| Microsoft Graph | `microsoft_graph-calendar-event-create` | `8eb5af5bd1c98df7023fd6cd65cfde4a8500e4260c97dc868c77fdd74116f56c` |
| Microsoft Graph | `microsoft_graph-calendar-event-get` | `4c9b9e030a7ca1c1e221a420a378f66989da4ed0796e9c59640a65371ba7838f` |
| Microsoft Graph | `microsoft_graph-files-item-get` | `60bf6766f88e44762c82ded7867a482338a3bd6507de3ad04428b1f7e6b526d8` |
| Microsoft Graph | `microsoft_graph-files-item-update` | `0373a662f297d0ef379f020df88471ba02a4d7d0f667724f53270b4c8c78f245` |
| Microsoft Graph | `microsoft_graph-mail-folder-list` | `e42ea239a7c0d085512e0c027aa80f81aacd452a5f286854f40ba8d3d90216d7` |
| Microsoft Graph | `microsoft_graph-mail-message-get` | `b010a7d1c9eddf59b961ec28467475681965b8b5c41e2f561e9035c05cc7dc3e` |
| Microsoft Graph | `microsoft_graph-mail-message-reply` | `a604176356b9e320748d299e92261e81ae36f7e24f7380da0be0b1ecc2a0ac51` |
| OpenAI | `openai-chat-completion` | `065939ba98cffd414a33aa81f37a116accc38de345deaf2cfd4cdce00bd38cc2` |
| OpenAI | `openai-embeddings-create` | `61664039f947dee160ee6a3023c47268f110b530ffc5ab105de23166de897b08` |
| OpenAI | `openai-model-get` | `c6ea0961053a40761d3df145d9af7bdf89cd42878cae019a0b65a64f5498fa89` |
| OpenAI | `openai-models-list` | `909d028a27dc466a4d1f2b121188e733979333ff6db6f7a11b996636f843e2a1` |
| Twilio | `twilio-account-get` | `eb5644b85ca45449a7f8eb58fc7a51805993abe07606746744567b0e7ba04858` |
| Twilio | `twilio-call-get` | `8f135e7b4787eaa1430d10ae6a0cfa35795580a54917b067ed424bd55d8069b1` |
| Twilio | `twilio-call-list` | `c6481d383dedefb4e7a0c6865d83fa6bb4ac095872aedda45401945eecfc6b6e` |
| Twilio | `twilio-message-get` | `45debc8e5fac98fc7166d5218db7554ba69406c9c9badb8ffec2ec165973b4fd` |
| Twilio | `twilio-message-list` | `5b2cb0c168a9df7e83514b61db1ef20b4635cfde2312cdeba71f403c6a9f3b9f` |

## 5. Parallel write-set contract

Each implementor owns the rows in its column and nothing else:

| story | provider-owned source and tests | provider-owned generated artifacts |
|---|---|---|
| C-469 | `providers/github.toml`, `specs/github/**`, `specs/github.provenance.toml`, `scripts/vendor-github-spec.sh`, GitHub-specific tests | `connectors/github.{flux,connector.toml}`, `crates/catalog/src/generated/github.rs`, `crates/catalog/ops/github/**` |
| C-470 | corresponding `stripe` paths and Stripe-specific tests | `connectors/stripe.*`, generated `stripe.rs`, `ops/stripe/**` |
| C-471 | corresponding `microsoft_graph` paths, extraction/vendor script and Graph-specific tests | all `connectors/microsoft_graph-{mail,calendar,files}.*`, generated `microsoft_graph.rs`, `ops/microsoft_graph/**` |
| C-472 | corresponding `openai` paths and OpenAI-specific tests | `connectors/openai.*`, generated `openai.rs`, `ops/openai/**` |
| C-473 | corresponding `twilio` paths and Twilio-specific tests | `connectors/twilio.*`, generated `twilio.rs`, `ops/twilio/**` |

The measured artifact stems are distinct:

```text
$ find connectors -maxdepth 1 -type f \
>   \( -name 'github*' -o -name 'stripe*' -o -name 'microsoft_graph*' -o
>      -name 'openai*' -o -name 'twilio*' \) -printf '%f\n' | sort
github.connector.toml
github.flux
microsoft_graph-calendar.connector.toml
microsoft_graph-calendar.flux
microsoft_graph-files.connector.toml
microsoft_graph-files.flux
microsoft_graph-mail.connector.toml
microsoft_graph-mail.flux
openai.connector.toml
openai.flux
stripe.connector.toml
stripe.flux
twilio.connector.toml
twilio.flux
```

No provider implementor edits `connectors.lock`, `crates/catalog/src/generated.rs`,
`web/public/catalog.json`, README images, `CHANGELOG.md`, `WHATS-NEW.md`, the response-coverage
constants, or the generated board. C-474 owns those whole-catalogue/coordinator files. The five
provider write sets therefore have no shared path; a test whose filename is provider-specific also
must not walk `providers/`, per `per_provider_test_scope.rs`.

## 6. Dispatch gates

Before implementation, C-470 needs the exact empty optional GET-body normalization test. C-471 needs
the reference-closed extractor and must pin both the 38,050,122-byte upstream hash and its extracted
hash. C-472 needs the deterministic `/v1` path-prefix extraction and must fence the four existing
Flux hashes above. These are not reasons to widen the stories: they are the measured work required
to make their selected operations honest.

Each story then proves its provider with the scoped gate from `AGENTS.md`; C-474 alone performs full
catalogue regeneration and release integration.
