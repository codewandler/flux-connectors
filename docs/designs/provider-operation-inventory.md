# Design: provider operation inventory — zendesk, freshdesk, babelforce

> **Citation notice.** The `../flux/plugins/zendesk/src/main.rs:*` references below **cannot be
> re-checked**: that file was uncommitted working-tree material in the flux checkout and is absent at
> flux `v0.38.0`. The Zendesk operation set itself is unaffected — it describes Zendesk's real public
> API and is independently checkable against Zendesk's documentation. Treat the line numbers as a
> record of what was read, not as live references. See
> [zendesk-plugin-citation.md](zendesk-plugin-citation.md).

**Status:** accepted (research) · **Pillar:** Spec · **Stories:** [C-18](../stories/C-18-vendor-specs-and-inventory.md) → feeds [C-17](../stories/C-17-provider-configs.md)

> This document is **raw material, not a schema.** It records what each of the three launch providers
> actually exposes, which operations we selected, and exactly how each one authenticates. C-17 writes
> `providers/{zendesk,freshdesk,babelforce}.toml` *from this document*; the TOML schema itself is
> C-3's job and deliberately does not exist yet.
>
> **No credential value appears anywhere in this file.** Environment variable *names* only — that is
> a hard invariant of this repo ([AGENTS.md](../../AGENTS.md)).

## Contents

1. [The vendored spec cache](#1-the-vendored-spec-cache)
2. [How auth is recorded](#2-how-auth-is-recorded)
3. [Zendesk](#3-zendesk)
4. [Freshdesk](#4-freshdesk)
5. [Babelforce](#5-babelforce)
6. [Findings that change other stories](#6-findings-that-change-other-stories)
7. [Sources](#7-sources)

---

## 1. The vendored spec cache

Builds are hermetic and offline: generation reads bytes from `specs/`, never the network
([AGENTS.md](../../AGENTS.md), *"Generated artifacts are committed and reviewed"*).

### 1.1 babelforce — vendored

| | |
|---|---|
| **Vendored file** | `specs/babelforce/manager-0.7.0.openapi.json` |
| **Upstream path** | `/home/timo/babelforce/projects/babelforce-api/babelforce-api/openapi/manager.openapi.json` |
| **Upstream repo** | `babelforce-api` (private) — the file is generated from the manager service's route definitions |
| **Format** | OpenAPI **3.0.3** |
| **`info.version`** | **0.7.0** |
| **sha256** | `6a79679409787c4ab1716936bca987226aacdc28eeff19039c0ea5ea34285421` |
| **Size / shape** | 449 291 bytes · 17 281 lines · **98 paths** · **163 operations** |
| **Copied** | byte-identical; `diff` against upstream is empty. The upstream file was **not modified**. |

Servers declared in the document (`specs/babelforce/manager-0.7.0.openapi.json:12`):

| URL | Role | Use for the connector? |
|---|---|---|
| `https://services.babelforce.com` | Main environment | **yes** — the default base URL |
| `https://latest.dev.babelforce.com` | Staging | no — reachable via the `BABELFORCE_URL` override |
| `http://localhost:7777` | Local development | no |

Note the *first* declared server is staging, not production. A naive "take `servers[0]`" ingest
(C-4) would point the connector at the dev environment. **Base URL selection must be explicit in the
provider TOML, not inherited positionally.**

**Refreshing this file** is C-14's `flux-connectors fetch` + drift-check job. Until then: re-copy from
the upstream path, re-record `info.version` and the sha256 in this table, and review the operation
diff against §5.2 before regenerating.

> ⚠ **Before this file is published, read §1.3.** It embeds credential-shaped example values and the
> vendoring is not cleared for a public repository yet.

### 1.2 zendesk and freshdesk — no vendored spec, hand-derived

Neither provider has a spec in `specs/`. Their operation sets in §3 and §4 are **hand-derived** from
on-disk sources (a working Rust plugin and a working integration collection), and **no network fetch
was performed for this story.** That is a deliberate, temporary state:

| Provider | Where a real spec would come from later | Confidence |
|---|---|---|
| zendesk | Zendesk publishes OpenAPI descriptions alongside its API reference on `developer.zendesk.com`. When C-14 lands, that document should be fetched into `specs/zendesk/` and the set below becomes an **overlay selection** (C-6) over it rather than a hand-written list. | likely available — **verify when C-14 lands** |
| freshdesk | Freshdesk's developer documentation (`developers.freshdesk.com/api/`, cited by the source collection) is hand-written HTML with no official machine-readable description known to us. Absent an official spec, freshdesk stays hand-authored; a community spec would need vetting before being vendored. | **no official spec known** |

Until a spec is vendored, drift for these two providers is undetectable by machine. That is a real
gap, not an oversight — see §6.7.

### 1.3 ⚠ UNRESOLVED — the vendored spec embeds credential-shaped example values

`manager.openapi.json` carries a **response `example` block for a real-looking test account**
(`specs/babelforce/manager-0.7.0.openapi.json:16345-16370`). It contains, in plain text:

| Line | Field | Shape |
|---|---|---|
| `:16361` | `customer.apis.babelforce.accessId` | UUID |
| `:16362` | `customer.apis.babelforce.accessToken` | 32-char hex |
| `:16365` | `customer.apis.stream.token` | 64-char hex |
| `:16351` | `customer.email` | a `@babelforce.com` address of a named person |

Across the document there are **4 token-shaped hex literals** and **5 `accessToken` occurrences**.
The account is labelled `Testers Inc.` and the example is dated 2021, so these are most likely
long-dead fixtures for a **staging** account — but that is an assumption, not a verified fact, and
the pair is exactly the credential type in §5.1.3.

**Why this is escalated rather than handled here:**

- `flux-connectors` is **public** — `repository = "https://github.com/codewandler/flux-connectors"`,
  dual MIT/Apache (`Cargo.toml:12-13`). Vendoring commits these literals to a public history, where
  removing them later requires a history rewrite, not a delete.
- The upstream repository is private, so these values have not been published before.
- The two deliverable requirements are in genuine tension: the story requires a **byte-identical**
  copy with a recorded sha256 (that identity is what makes drift-check and provenance meaningful),
  while scrubbing the example would break both.

**This is not a decision for the implementing agent.** Required before merge, in order:

1. **Confirm with the babelforce API owners** whether the `Testers Inc.` credentials are live. If
   they are, **rotate first** — the exposure already exists in the private repo.
2. **Then choose the vendoring policy**, explicitly:
   - *(a)* commit byte-identical and accept the disclosure (only defensible once rotation is
     confirmed);
   - *(b)* vendor a **scrubbed** copy, record **both** hashes — upstream-original for drift-check
     and scrubbed-local for the lockfile — and make the scrub a declared, reproducible transform so
     C-14 can re-apply it on every fetch. **Recommended**: it keeps drift-check honest and keeps
     secrets out of a public repo.
   - *(c)* keep `specs/` out of the published tree. Rejected — it breaks the hermetic-build
     invariant ([AGENTS.md](../../AGENTS.md)).
3. **Generalise it**: C-14's fetch path should scan every fetched spec for credential-shaped
   literals and refuse to vendor silently. This will not be the last vendor spec with a live-looking
   example in it. Worth its own story.

Until (1) and (2) are answered, treat `specs/babelforce/manager-0.7.0.openapi.json` as **staged, not
cleared for publication**. Everything else in this document is independent of the outcome — the
operation inventory below does not depend on which vendoring policy is chosen.

---

## 2. How auth is recorded

Vocabulary is `flux_plugin_protocol::AuthScheme`
(`../flux/crates/flux-plugin-protocol/src/lib.rs:344`), reused rather than reinvented, per
[auth-seam.md §2](auth-seam.md):

| Scheme | Wire form | Composed from |
|---|---|---|
| `bearer` | `Authorization: Bearer <secret>` | `env` |
| `basic` | `Authorization: Basic base64(<user>:<secret>)` | `user_env` (non-secret config) + `env` (gated secret) |
| `header{name}` | `<name>: <secret>` | `env` |
| `query{name}` | `?<name>=<secret>` | `env` |

**Requirement-set shape** — what an operation needs in order to be authorized:

- **single** — exactly one purpose. (A `basic` purpose is still *single*: `user_env` + `env` are two
  inputs to **one** credential, not two credentials.)
- **AND** — several distinct purposes must all be present on the same request.
- **OR** — alternative sets, any one of which authorizes; codegen picks the first satisfiable set in
  declared order (C-10).
- **none** — the operation needs no credential.

All three launch providers land on **single** for every selected operation. The AND and OR shapes are
still required by the schema — see §6.1 and §5.1.3.

---

## 3. Zendesk

### 3.1 Auth model

| Field | Value |
|---|---|
| Scheme | **`basic`** |
| Purpose | `zendesk.api_token` |
| Secret env (`env`) | **`ZENDESK_API_TOKEN`** — the only secret |
| User env (`user_env`, non-secret) | **`ZENDESK_USER`** |
| User-half format | Zendesk's **`<email>/token`** form, e.g. `agent@example.com/token`. The literal `/token` suffix is what tells Zendesk the password is an API token rather than a password. |
| Endpoint env | **`ZENDESK_URL`** — e.g. `https://company.zendesk.com` |
| `http_hosts` | `*.zendesk.com` |
| Requirement-set shape | **single**, identically for all 7 operations |

Citations: `../flux/plugins/zendesk/src/main.rs:5-6` (the env-var contract, verbatim: *"`ZENDESK_USER`
is the non-secret username half and should use Zendesk's `<email>/token` form; `ZENDESK_API_TOKEN` is
the sole secret"*), `:14-15` (purpose + endpoint names), `:128-146` (`Caps.secrets`,
`AuthMethod::basic`, `EndpointSpec`), `:664-666` (the test that pins them).

`Basic` here fits `AuthScheme::Basic` exactly: the non-secret identity is in the user half, the
secret in the password half. Contrast Freshdesk (§4.1), which does not.

### 3.2 Operations — 7 selected of 7 available

The plugin *is* the requirement: a connector replaces it only if it covers the same surface. All
seven are taken.

| # | Operation | Method | Path | Description |
|---|---|---|---|---|
| 1 | `zendesk.test` | GET | `/api/v2/users/me.json` | Verify credentials by fetching the authenticated user. |
| 2 | `zendesk.ticket.search` | GET | `/api/v2/search.json` | Search tickets with Zendesk search syntax; pages bounded to 100 results. |
| 3 | `zendesk.ticket.show` | GET | `/api/v2/tickets/{ticket_id}.json` | Show one ticket. |
| 4 | `zendesk.ticket.comment.list` | GET | `/api/v2/tickets/{ticket_id}/comments.json` | List a ticket's comments; pages bounded to 100. |
| 5 | `zendesk.ticket.update` | PUT | `/api/v2/tickets/{ticket_id}.json` | Safe-update selected ticket fields against the caller's `updated_stamp`. |
| 6 | `zendesk.ticket.comment.add` | PUT | `/api/v2/tickets/{ticket_id}.json` | Add a comment; **internal unless `public` is explicitly true**. |
| 7 | `zendesk.ticket.tag.add` | PUT | `/api/v2/tickets/{ticket_id}.json` | Add tags **without replacing** existing tags. |

#### Parameters

**1. `zendesk.test`** — none. (`main.rs:19`, `:152-158`, `:255-262`)

**2. `zendesk.ticket.search`** (`main.rs:23-31`, `:264-292`)

| Name | In | Type | Required | Notes |
|---|---|---|---|---|
| `query` | query | string | **yes** | Zendesk search expression. |
| `page` | query | integer | no | default `1`, min `1` |
| `per_page` | query | integer | no | default `100`, range `1..=100` |

**3. `zendesk.ticket.show`** (`main.rs:35-38`, `:294-303`)

| Name | In | Type | Required | Notes |
|---|---|---|---|---|
| `ticket_id` | path | integer (u64) | **yes** | min `1` |

**4. `zendesk.ticket.comment.list`** (`main.rs:42-51`, `:305-325`)

| Name | In | Type | Required | Notes |
|---|---|---|---|---|
| `ticket_id` | path | integer (u64) | **yes** | min `1` |
| `page` | query | integer | no | default `1`, min `1` |
| `per_page` | query | integer | no | default `100`, range `1..=100` |

**5. `zendesk.ticket.update`** (`main.rs:55-69`, `:327-337`, `:366-377`)

| Name | In | Type | Required | Notes |
|---|---|---|---|---|
| `ticket_id` | path | integer (u64) | **yes** | min `1` |
| `updated_stamp` | body `ticket.updated_stamp` | string | **yes** | optimistic-concurrency stamp |
| `status` | body `ticket.status` | string | no | |
| `priority` | body `ticket.priority` | string | no | |
| `assignee_id` | body `ticket.assignee_id` | integer (u64) | no | |
| `group_id` | body `ticket.group_id` | integer (u64) | no | |
| `type` | body `ticket.type` | string | no | Rust field `ticket_type`, wire name `type`. |
| — | body `ticket.safe_update` | boolean | **constant `true`** | Not caller-supplied; always emitted. |

Precondition (plugin preflight, `main.rs:201-209`): **at least one** of
`status`/`priority`/`assignee_id`/`group_id`/`type` must be non-null.

**6. `zendesk.ticket.comment.add`** (`main.rs:73-80`, `:339-352`)

| Name | In | Type | Required | Notes |
|---|---|---|---|---|
| `ticket_id` | path | integer (u64) | **yes** | min `1` |
| `updated_stamp` | body `ticket.updated_stamp` | string | **yes** | |
| `body` | body `ticket.comment.body` | string | **yes** | must not be blank (`main.rs:210-218`) |
| `public` | body `ticket.comment.public` | boolean | no | **default `false`** — internal note unless explicit |
| — | body `ticket.safe_update` | boolean | constant `true` | |

**7. `zendesk.ticket.tag.add`** (`main.rs:84-89`, `:354-364`)

| Name | In | Type | Required | Notes |
|---|---|---|---|---|
| `ticket_id` | path | integer (u64) | **yes** | min `1` |
| `updated_stamp` | body `ticket.updated_stamp` | string | **yes** | |
| `tags` | body `ticket.additional_tags` | array[string] | **yes** | non-empty, no blank entries (`main.rs:219-232`) |
| — | body `ticket.safe_update` | boolean | constant `true` | |

### 3.3 Quirks C-12 must carry

These are behavior, not schema. They are why "just generate the CRUD" is not enough.

1. **Three operations, one endpoint.** Ops 5, 6 and 7 are all `PUT /api/v2/tickets/{id}.json`,
   distinguished *only* by request body. A path-keyed operation model cannot represent them; the IR
   (C-2) needs body shape as part of operation identity.
2. **`safe_update` + `updated_stamp`.** Every write carries `safe_update: true` and the caller's
   stamp; Zendesk rejects the update if the ticket moved. This is optimistic concurrency baked into
   the operation, and dropping it turns every write into a last-write-wins race.
3. **`additional_tags`, not `tags`.** Sending `tags` *replaces* the ticket's tags. The whole point of
   op 7 is the additive field. Getting this wrong silently destroys data.
4. **Comments default to internal.** `public` defaults to `false` (`main.rs:79`). A generated op that
   inherits a "sensible" default of `true` would leak internal notes to end users.
5. **Strict percent-encoding of the search query** (`main.rs:426-441`): spaces become `%20` never
   `+`, colons `%3A`. Zendesk search expressions are colon-heavy (`type:ticket status:new`), and
   form-encoding them breaks the query silently rather than erroring.
6. **`per_page` is capped at 100** by the input schema, before the request is sent
   (`main.rs:29`, proven by `main.rs:518-529`).

---

## 4. Freshdesk

Source: `action-proxy`'s `freshdesk` collection. Per the dispatch and
[connector-pipeline.md:13](connector-pipeline.md), action-proxy is mined **for endpoint facts only** —
it hand-maintained 649 lines of YAML per provider and needed a bespoke template function for
Zendesk's Basic auth. It is the problem statement, not the template.

### 4.1 Auth model — and the inversion it exposes

| Field | Value |
|---|---|
| Scheme | **`basic`** |
| Purpose | `freshdesk.api_key` |
| Secret env (`env`) | **`FRESHDESK_API_KEY`** |
| Password half | the **literal string `X`** — a constant, not a credential |
| Endpoint env | **`FRESHDESK_DOMAIN`** — the account domain, e.g. `my-company.freshdesk.com` |
| Base URL | `https://{FRESHDESK_DOMAIN}/api/v2` |
| `http_hosts` | `*.freshdesk.com` |
| Requirement-set shape | **single**, identically for all 9 operations |

Citations: `freshdesk.yml:40` (base URL template `https://{{context.api_host}}/api/v2`),
`freshdesk.yml:48-50` (`auth: {user: :context.api_key, pass: X}`), `template.yml:15-20`
(`type: basic`, `username: :context.api_key`, `password: X`), `template.yml:1-13` / `freshdesk.yml:29-36` (`api_host` and
`api_key` are the two required context properties).

> **⚠ This does not fit `AuthScheme::Basic` as currently defined.**
>
> `AuthMethod::basic` composes `base64(<user_env>:<env>)`, and its doc comment states that `user_env`
> values *"are config (not a gated secret), so they resolve directly from declared env like an
> endpoint"* (`../flux/crates/flux-plugin-protocol/src/lib.rs:433-434`).
>
> Freshdesk needs `base64(<secret>:X)` — the **secret occupies the username position** and the
> password is a literal constant. Expressing this with today's types would mean putting
> `FRESHDESK_API_KEY` in `user_env`, i.e. **declaring the API key as non-secret config**, which
> removes it from the secret-gating path and from the redactor's `add_secret` registration
> ([auth-seam.md §4](auth-seam.md)).
>
> That is a security regression, not a cosmetic mismatch, and it must not be worked around in a
> provider TOML. Recorded as an open question for C-16 / C-5 — see §6.2.

### 4.2 Operations — 9 selected of 16 available

The source collection defines 16 actions: 12 that issue HTTP directly, plus 4 `forward:` aliases that
re-dispatch to another action with remapped parameters. Selection is **ticket-centric** with the
contact operations that make ticket work possible (you cannot file a ticket for a caller you cannot
resolve), plus one verification op.

| # | Operation | Method | Path | Source | Description |
|---|---|---|---|---|---|
| 1 | `freshdesk.test` | GET | `/contacts` | `yml:522-528` | Verify credentials with a bounded contact read. |
| 2 | `freshdesk.ticket.list` | GET | `/tickets` | `yml:55-82` | List and filter tickets. |
| 3 | `freshdesk.ticket.get` | GET | `/tickets/{id}` | `yml:84-97` | View one ticket. |
| 4 | `freshdesk.ticket.create` | POST | `/tickets` | `yml:99-211` | Create a ticket. |
| 5 | `freshdesk.ticket.update` | PUT | `/tickets/{id}` | `yml:213-305` | Update a ticket's fields. |
| 6 | `freshdesk.ticket.note.add` | POST | `/tickets/{id}/notes` | `yml:307-348` | Add a note to a ticket; **private by default**. |
| 7 | `freshdesk.contact.list` | GET | `/contacts` | `yml:412-439` | List and filter contacts (caller lookup). |
| 8 | `freshdesk.contact.get` | GET | `/contacts/{id}` | `yml:441-454` | Get one contact. |
| 9 | `freshdesk.contact.create` | POST | `/contacts` | `yml:456-468` | Create a contact. |

**Excluded (7):**

| Excluded | Why |
|---|---|
| `tickets.outbound_email` (`yml:350-408`) | Requires a mandatory `email_config_id` that is per-account outbound-mailbox configuration, and it *sends mail* as a side effect of ticket creation. High blast radius, narrow use. Revisit when there is a caller who wants it. |
| `agent.get`, `agent.list` (`yml:472-517`) | Agent-directory sync, not ticket work. `agent.list` also carries the collection's only inline expression (§6.3), which we deliberately do not port. |
| `babelforce.integration.users.get` / `.users.list` / `.enduser.create` / `.enduser.lookup` (`yml:530-641`) | Not Freshdesk operations at all — they are action-proxy's own capability contract (`babelforce.enduser`, `babelforce.users`, `yml:12-15`), implemented by forwarding to the underlying action and reshaping the response. That reshaping belongs in a Flux flow, not in a connector. |

#### Parameters

**1. `freshdesk.test`** — GET `/contacts`

| Name | In | Type | Required | Notes |
|---|---|---|---|---|
| `per_page` | query | integer | no | default `1`. **Ours, not the source's** — the source op (`yml:522-528`) is an unbounded `GET /contacts`. Bounding a liveness check to one record is a deliberate addition. |

**2. `freshdesk.ticket.list`** (`yml:55-82`) — note every parameter is **renamed** on the wire.

| Param name | In | Wire name | Type | Required | Description |
|---|---|---|---|---|---|
| `req_id` | query | `requester_id` | string | no | Filter by requester. |
| `req_email` | query | `email` | string | no | Filter by requester email. |
| `company_id` | query | `company_id` | string | no | Filter by company ID. |
| `updated` | query | `updated_since` | date | **see note** | Filter by updated-since date. |

*Note on `updated`:* the source marks it `required: true` (`yml:69-74`). That is an action-proxy UI
constraint — Freshdesk's `GET /tickets` does not require `updated_since`. **Recorded as optional**,
with the deviation flagged in §6.4. C-17 should not inherit the false requirement.

**3. `freshdesk.ticket.get`** (`yml:84-97`)

| Name | In | Type | Required | Notes |
|---|---|---|---|---|
| `id` | path | string | **yes** | |

**4. `freshdesk.ticket.create`** (`yml:99-211`) — body is the whole parameter map (`body: :params`).

| Name | In | Type | Required | Default | Description |
|---|---|---|---|---|---|
| `phone` | body | string | yes¹ | `{consumer.number}`² | Requester phone number. |
| `status` | body | integer | yes | `2` | Ticket status; enum `[2,3,4,5]`. |
| `priority` | body | integer | yes | `2` | Ticket priority; enum `[1,2,3,4]`. |
| `description` | body | string | yes | `My Description`² | Ticket description (HTML/long text). |
| `subject` | body | string | yes | `Call from {consumer.number}`² | Ticket subject. |
| `requester_id` | body | integer | no | — | Requester ID. |
| `responder_id` | body | integer | no | — | Responding agent ID. |
| `name` | body | string | no | `New caller {call.from}`² | Requester name. |
| `source` | body | integer | no | `3` | Channel; enum `[1,2,3,7,8,9,10]`. |
| `type` | body | string | no | — | Query type. |
| `email_config_id` | body | integer | no | — | Email config used for this ticket. |
| `group_id` | body | integer | no | — | Group the ticket is assigned to. |
| `product_id` | body | integer | no | — | Associated product. |
| `tags` | body | array[string] | no | — | Ticket tags. |
| `cc_emails` | body | array[string:email] | no | — | CC recipients. |
| `custom_fields` | body | object(string→string\|integer) | no | — | Custom ticket field data. |

¹ **The real constraint is an OR, expressed only in prose.** `yml:104-106`: *"you should either fill
phone AND name field, or the requester_id"*. So the requester is identified by
(`phone` AND `name`) **OR** `requester_id` — neither of which the `required:` flags capture. See §6.4.
² Defaults are **action-proxy call-context interpolations** (`{consumer.number}`, `{call.from}`) —
babelforce telephony variables, meaningless outside action-proxy. They must **not** be copied into a
provider TOML as literal defaults. Drop them; a Flux flow supplies real values.

**5. `freshdesk.ticket.update`** (`yml:213-305`) — body is an explicit field map (`yml:290-305`).

| Name | In | Type | Required | Description |
|---|---|---|---|---|
| `id` | path | integer | **yes**³ | ID of the ticket to update. |
| `phone` | body | string | no | Requester phone. |
| `status` | body | integer | no | enum `[2,3,4,5]` |
| `priority` | body | integer | no | enum `[1,2,3,4]` |
| `description` | body | string | no | Ticket description. |
| `subject` | body | string | no | Ticket subject. |
| `requester_id` | body | integer | no | Requester ID. |
| `responder_id` | body | integer | no | Responding agent ID. |
| `email` | body | string (email) | no | Requester email. |
| `name` | body | string | no | Requester name. |
| `type` | body | string | no | Query type. |
| `email_config_id` | body | integer | no | Email config used for this ticket. |
| `group_id` | body | integer | no | Assigned group. |
| `product_id` | body | integer | no | Associated product. |
| `tags` | body | array[string] | no | Ticket tags. |
| `custom_fields` | body | object(string→string\|integer) | no | Custom field data. |

³ The source does **not** mark `id` required (`yml:218-223`); it supplies a default of
`{integration.freshdesk.ticket.id}` instead. A path parameter with no value produces the URL
`/tickets/` — a silent mis-request. **Recorded as required**, deviation flagged in §6.4.

Also note the asymmetry: `email` is updatable but not settable at create — `ticket.create` has no
`email` parameter although Freshdesk accepts one. An artifact of the collection, not of the API.

**6. `freshdesk.ticket.note.add`** (`yml:307-348`)

| Name | In | Type | Required | Default | Description |
|---|---|---|---|---|---|
| `id` | path | integer | **yes**³ | — | ID of the ticket. (Source: not marked required, `yml:312`.) |
| `body` | body | string | **yes** | — | Note body. (Source description reads *"Requester ID"* — copy-paste error, `yml:318`.) |
| `private` | body | boolean | no | **`true`** | Whether the note is hidden from end users. |
| `incoming` | body | boolean | no | `true` | Set true for an external system. |
| `notify_emails` | body | array[string:email] | no | — | Addresses to notify. |

`private` defaulting to `true` is the same safety posture as Zendesk's `public: false` (§3.3.4) and
must survive into the generated op.

**7. `freshdesk.contact.list`** (`yml:412-439`)

| Name | In | Type | Required | Description |
|---|---|---|---|---|
| `phone` | query | string | no | Filter by phone. |
| `email` | query | string | no | Filter by email. |
| `mobile` | query | string | no | Filter by mobile. |
| `company_id` | query | string | no | Filter by company ID. |
| `state` | query | string | no | enum `[blocked, deleted, unverified, verified]` |

**8. `freshdesk.contact.get`** (`yml:441-454`)

| Name | In | Type | Required |
|---|---|---|---|
| `id` | path | string | **yes** |

Method is not stated on the action; it inherits `get` from `defaults.action.http.method`
(`yml:41`).

**9. `freshdesk.contact.create`** (`yml:456-470`) — body is the parameter map.

| Name | In | Type | Required |
|---|---|---|---|
| `name` | body | string | **yes** |
| `email` | body | string | no |
| `phone` | body | string | no |

### 4.3 Common request shape

From `defaults.action.http` (`yml:37-50`): `content-type: application/json` on every request, JSON
response parsing, and a **10 s timeout** (`yml:39`). The timeout is a per-provider default worth
carrying into the provider TOML rather than relying on flux's global HTTP default.

---

## 5. Babelforce

Source: the vendored spec, §1.1. **163 operations must become a usable handful** — 163 generated
`op` declarations would become 163 LLM tools, most of them destructive admin CRUD.

### 5.1 Auth model

#### 5.1.1 What we ship

| Field | Value |
|---|---|
| Scheme | **`bearer`** — `Authorization: Bearer <token>` |
| Purpose | `babelforce.access_token` |
| Secret env (`env`) | **`BABELFORCE_ACCESS_TOKEN`** |
| Token provenance | **SSO-issued.** The token is minted outside flux and supplied through the environment; flux injects it and registers it with the redactor. flux runs no grant. |
| Endpoint env | **`BABELFORCE_URL`**, default `https://services.babelforce.com` |
| `http_hosts` | `*.babelforce.com` |
| Requirement-set shape | **single**, identically for all selected operations |

The requirement-set claim is mechanically verified, not assumed: the document declares `security` once
at the root (`specs/babelforce/manager-0.7.0.openapi.json:17271`) and **zero of the 163 operations
override it** with an operation-level `security` key.

#### 5.1.2 JWT is planned — the manifest schema must already accept it

Babelforce intends to move to **JWT**. On the wire that changes nothing: a JWT still travels as
`Authorization: Bearer <token>`, so `AuthScheme::Bearer` remains correct. What changes is the token's
**provenance** — how it is minted, validated and refreshed.

**Requirement for C-10:** the connector manifest's auth entry must keep *wire scheme* and *token
provenance* as separate fields, so that adding JWT later is an additive change and never a `scheme`
change. `AuthMethod` already has exactly this shape — `scheme: AuthScheme` alongside an optional
`oauth2: Option<OAuth2Spec>` provenance block
(`../flux/crates/flux-plugin-protocol/src/lib.rs:422-443`). C-10 should design
`<provider>.connector.toml` so a sibling provenance block (`[auth.jwt]`, carrying at minimum issuer,
audience and a JWKS or key reference) can be added without touching `scheme = "bearer"` and without
re-generating any existing connector.

Concretely, **do not** model babelforce's auth as "an opaque string in an env var" in a way that makes
provenance unrepresentable. Today's config *is* a static env token; the schema must not assume it
always will be.

#### 5.1.3 EXCLUDED: the `accessId` / `accessToken` header pair — **deprecated, do not re-add**

The spec declares a second security option
(`specs/babelforce/manager-0.7.0.openapi.json:17259-17268`):

```
accessId:    apiKey, in: header, name: X-Auth-Access-Id
accessToken: apiKey, in: header, name: X-Auth-Access-Token
```

and the root `security` array (`:17271-17281`) offers it as an **alternative** to `oauth2` — the two
headers being an **AND-set** within that alternative.

**This pair is excluded from the connector. It is deprecated and is being scrubbed from the API by
the babelforce maintainers.**

> 🚫 **To a future reader:** its absence is not a bug and not an oversight in ingest. Do not "fix" the
> connector by re-adding `X-Auth-Access-Id` / `X-Auth-Access-Token`, and do not treat a regenerated
> spec that still lists them as evidence they are supported. They are on their way out of the API.
> Removing them here is the point.

Reasons, beyond the maintainers' decision:

- It is a **static, long-lived credential pair** with no expiry and no rotation story — the opposite
  direction from the SSO/JWT move.
- Both halves are secrets sent as raw headers, doubling the surface that must be gated and redacted
  for zero benefit over a single bearer.
- The spec carries **no `deprecated` marker anywhere** (0 occurrences in 17 281 lines), so the
  deprecation is *only* recorded here. An automated ingest reading the spec alone would happily
  select it. That is precisely why this section exists.

**Consequence for C-4/C-5:** ingest will keep seeing an OR of two sets, one of which is an AND-set.
The exclusion is an **overlay decision** (C-6), applied on top of faithful ingest — the ingest layer
must not be taught that this pair does not exist, or drift-check (C-14) will stop reporting on it.
After the overlay, C-10's "first satisfiable set in declared order" rule selects `oauth2` → bearer,
which is the correct outcome.

**Consequence for C-10:** see §6.1 — this was C-10's motivating AND example.

#### 5.1.4 A discrepancy worth stating

The spec models the bearer as **OAuth2 with a `password` grant** at `/oauth/token`
(`:17246-17258`). The operational reality is an **SSO-issued** token. These are not the same thing,
and we follow the operational reality:

- **Do not** implement the password grant. It would require flux to hold a babelforce *user password*
  in order to mint tokens — strictly worse than holding the token.
- Model it as `scheme = "bearer"` resolved from `BABELFORCE_ACCESS_TOKEN`, with provenance recorded
  as SSO/external.
- Revisit only if babelforce publishes a client-credentials flow, which `OAuth2Spec`'s
  `ClientCredentials` grant already covers.

### 5.2 Operations — 9 selected of 163 available

**Selection principle:** include only what a *support flow executes at runtime*, and exclude
everything that is *account administration performed in the babelforce UI*.

The three groups that survive that test are **agents** (who is available), **calls** (what is
happening, and acting on it), and **sessions** (the variable bag a flow reads and writes). They are
also the groups that pair naturally with a Zendesk or Freshdesk ticket — correlating a call to a
ticket is the whole reason all three providers ship together.

| # | Operation | Method | Path | `operationId` | Spec line | Description |
|---|---|---|---|---|---|---|
| 1 | `babelforce.agent.list` | GET | `/api/v2/agents` | `listAgents` | `:84` | List and filter agents. |
| 2 | `babelforce.agent.get` | GET | `/api/v2/agents/{id}` | `getAgent` | `:550` | Get one agent. |
| 3 | `babelforce.agent.status.update` | PUT | `/api/v2/agents/{id}/status` | `updateAgentStatus` | `:687` | Update an agent's status. |
| 4 | `babelforce.call.list` | GET | `/api/v2/calls/reporting` | `listReportingCalls` | `:2472` | List and filter calls (reporting view). |
| 5 | `babelforce.call.get` | GET | `/api/v2/calls/{id}` | `getCall` | `:3833` | Get one call. |
| 6 | `babelforce.call.hangup` | POST | `/api/v2/calls/{id}/hangup` | `hangupCall` | `:3879` | Hang up a call. |
| 7 | `babelforce.call.session.set` | PUT | `/api/v2/calls/{id}/session/set` | `setCallSessionVariables` | `:3922` | Set session variables on a call. |
| 8 | `babelforce.session.get` | GET | `/api/v2/sessions/{id}` | `getSessionVariables` | `:8171` | Get IVR variables for a session. |
| 9 | `babelforce.session.update` | PUT | `/api/v2/sessions/{id}` | `updateSessionVariables` | `:8171` | Update user-scoped session variables. |

Operation 1 doubles as the **verification op** (cheap, read-only, fails loudly on bad credentials);
babelforce has no `/me` endpoint in this document.

#### Parameters

**1. `babelforce.agent.list`** — GET `/api/v2/agents` (`:84`). All 11 spec parameters kept; the list is
already small and every filter is useful.

| Name | In | Type | Required | Description |
|---|---|---|---|---|
| `page` | query | integer | no | Page number. |
| `max` | query | integer | no | Page size. |
| `q` | query | string | no | Free-text search over `name` and `group.name`. |
| `enabled` | query | boolean | no | Filter enabled/disabled agents. |
| `name` | query | string | no | Search by agent name. |
| `number` | query | string | no | Filter by the agent's number. |
| `sourceId` | query | string | no | Filter by integration source ID. |
| `source` | query | string | no | Filter by source integration. |
| `state` | query | string | no | enum `[available, busy, declined, unreachable, selected, ringing, in-call, wrap-up]` (`AgentLineStatus`) |
| `groupIds` | query | string \| array[string] | no | Filter by group ID. **`oneOf` scalar-or-array** — see §6.5. |
| `groups` | query | string \| array[string] | no | Filter by group name. **`oneOf` scalar-or-array.** |

**2. `babelforce.agent.get`** — GET `/api/v2/agents/{id}` (`:550`)

| Name | In | Type | Required |
|---|---|---|---|
| `id` | path | string | **yes** |

**3. `babelforce.agent.status.update`** — PUT `/api/v2/agents/{id}/status` (`:687`)

| Name | In | Type | Required | Description |
|---|---|---|---|---|
| `id` | path | string | **yes** | Agent ID. |
| `enabled` | body `enabled` | boolean | no | Enable or disable the agent. |
| `presence.name` | body `presence.name` | string | no | Presence label to set. |

Schema `UpdateAgentStatusRequest`. The request body is declared `required: false` with **every**
property optional, so an empty `PUT` is schema-valid and does nothing. Same class of problem as
Zendesk's empty-update, which the plugin solved with a preflight (`main.rs:201-209`) — a C-12 quirk
candidate.

**4. `babelforce.call.list`** — GET `/api/v2/calls/reporting` (`:2472`). **The spec declares 40 query
parameters; we select 13.**

| Name | In | Type | Required | Description |
|---|---|---|---|---|
| `page` | query | integer | no | Page number. |
| `max` | query | integer | no | Page size. |
| `id` | query | uuid \| array[uuid] | no | Filter by call ID. |
| `sessionId` | query | uuid | no | Filter by session ID. |
| `conversationId` | query | uuid | no | Filter by conversation ID. |
| `agentId` | query | uuid \| array[uuid] | no | Filter by agent. |
| `fromNumber` | query | string \| array[string] | no | Filter by originating number. |
| `toNumber` | query | string \| array[string] | no | Filter by destination number. |
| `type` | query | string \| array | no | enum `[inbound, outbound]` (`CallType`) |
| `state` | query | string \| array | no | enum `[init, scheduled, ringing, in-progress, queued, bridged, canceled, busy, no-answer, purged, completed, failed]` (`CallState`) |
| `finishReason` | query | string \| array | no | enum `[unknown, passive-hangup, system-hangup, failed, timeout, unreachable, busy, declined, canceled, transferred]` |
| `time.start` | query | integer (unix ts) | no | Window start. **Dotted name** — see §6.5. |
| `time.end` | query | integer (unix ts) | no | Window end. **Dotted name.** |
| `q` | query | string | no | Free-text search. |

Dropped from this operation, deliberately:

- **All 20 `filters.*` parameters.** The spec declares every filter **twice** — `sessionId` and
  `filters.sessionId`, `state` and `filters.state`, and so on — with identical schemas. Almost
  certainly one serializer emitting both a flat and a nested binding of the same filter object.
  Generating both would double the tool surface with exact synonyms. **Flagged in §6.5.**
- `from` / `to`, which appear to be legacy aliases of `fromNumber` / `toNumber` (identical
  `ReportingNumberFilter` schema). We take the explicit pair. **Unconfirmed — see §6.5.**
- `agent.id`, a dotted alias of `agentId`.
- `domain`, `source`, `anonymous`, `parentId` — real filters, but narrow; add on demand.

**5. `babelforce.call.get`** — GET `/api/v2/calls/{id}` (`:3833`)

| Name | In | Type | Required |
|---|---|---|---|
| `id` | path | string | **yes** |

**6. `babelforce.call.hangup`** — POST `/api/v2/calls/{id}/hangup` (`:3879`)

| Name | In | Type | Required |
|---|---|---|---|
| `id` | path | string | **yes** |

No request body. The only destructive operation in the selected set, and the only one that affects a
live caller — it should carry a write/`SendExternal`-class effect, never be marked read-only.

**7. `babelforce.call.session.set`** — PUT `/api/v2/calls/{id}/session/set` (`:3922`)

| Name | In | Type | Required | Description |
|---|---|---|---|---|
| `id` | path | string | **yes** | Call ID. |
| *(body)* | body | object (free-form) | no | Variables to set. **Spec description: keys "must start with `app`".** |

Schema `SetCallSessionVariablesRequest` is `{"type": "object"}` with **no properties** — a free-form
map. The `app` key prefix rule exists only in the operation's prose description (`:3922`ff) and is
unenforceable from the schema. A C-12 quirk: either validate the prefix in the generated op or
document that the API rejects other keys.

**8. `babelforce.session.get`** — GET `/api/v2/sessions/{id}` (`:8171`)

| Name | In | Type | Required |
|---|---|---|---|
| `id` | path | string | **yes** |

**9. `babelforce.session.update`** — PUT `/api/v2/sessions/{id}` (`:8171`)

| Name | In | Type | Required | Description |
|---|---|---|---|---|
| `id` | path | string | **yes** | Session ID. |
| *(body)* | body | object (free-form) | no | User-scoped session variables. |

Schema `UpdateSessionVariablesRequest`, also `{"type": "object"}` with no properties. Two of the nine
selected operations have completely untyped bodies — C-9 (bodies and responses) needs a
free-form-object representation, not just generated structs.

### 5.3 Exclusions — 154 operations, and why

Grouped, because the reasoning is per-group rather than per-operation.

| Group | Approx. ops | Excluded because |
|---|---|---|
| **Configuration CRUD** — applications, local/global automations, triggers, routings, queues + queue selections, calendars, business hours, prompts, babeldesk dashboards & widgets, integrations, phonebook, service numbers, global settings, outbound campaigns & lists | ~120 | Account **provisioning**, done in the babelforce UI by an admin. A flow reads call state; it does not create a routing. Generating them yields ~120 tools, dozens of them `DELETE`, offering an LLM the ability to delete a production queue. The cost/benefit is not close. |
| **User & role administration** — `/users`, `/users/delete`, `/users/disable`, `/users/enable`, `/users/reset-password`, `/users/roles`, `/users/roles/remove` | 9 | Identity administration, and **bulk-destructive by design** (`POST /users/delete` takes a list). This is the last surface that should be reachable from a generated tool. |
| **Observability & meta** — metrics push/reset/describe, live logs, audit logs, expressions list/evaluate, action & provider session variables | 9 | Operational telemetry that flux gets from its own plugins. `POST /expressions/evaluate` is a **remote expression evaluator**, which is exactly the "second little language" this repo exists to avoid ([AGENTS.md](../../AGENTS.md), *"No homegrown DSL"*) — and a poor thing to hand a model. |
| **Conversations, SMS, conferences** | ~13 | Genuinely plausible — `listConversations` was the closest call, since a conversation is the parent of a call. Held back only to keep the first cut to a handful. **The most likely v2 addition.** |
| **Bulk endpoints** — `DELETE /applications/bulk`, phonebook bulk up/download, `DELETE /outbound/lists/{id}/leads` | 4 | Unbounded destructive scope with no per-item confirmation. |
| **Creates/updates/deletes within the included groups** — `createAgent`, `updateAgent`, `deleteAgent`, agent groups CRUD, `createSession` | ~9 | The agent *roster* is provisioning; the agent *status* is runtime. We take the runtime half only. `createSession` mints state a flow should not be inventing. |

Two exclusions to revisit first, in order: `listConversations` (`/api/v2/conversations`, which already
has clean `page`/`max`/`phone`/`state` filters) and `listQueues` (`/api/v2/queues`), if flows need
queue context alongside agent availability.

---

## 6. Findings that change other stories

Everything here was found in the sources; none of it is speculation.

### 6.1 C-10's motivating AND-set example is now invalid

[C-10's Acceptance](../stories/C-10-auth-injection-and-manifest.md#L24-L26) reads:

> *"Babelforce sends `X-Auth-Access-Id` **and** `X-Auth-Access-Token` on the same request, so the
> emitter must handle an AND-set, not just a single credential."*

That pair is deprecated and excluded (§5.1.3). **Babelforce ships as single-purpose bearer and no
longer exercises AND.**

The **capability must stay** — AND-sets are real, and the babelforce spec's own root `security` array
still literally declares one, so ingest (C-4/C-5) must parse and represent it. What must change is
C-10's *test fixture*: the AND-set test needs a synthetic or different provider fixture, because
asserting it against the shipped babelforce connector will now fail. Same for
[C-17's first Acceptance item](../stories/C-17-provider-configs.md), which states babelforce's auth is
the two raw apiKey headers and needs **no** `$auth` seam — that is no longer true. Bearer needs the
`Bearer ` prefix, which is exactly what [auth-seam.md](auth-seam.md) says the `$secret` marker cannot
produce. **Babelforce is therefore blocked on the seam too, and this repo has no provider that is
executable against flux as it stands today.** That is a milestone-1 sequencing change, not a detail.

### 6.2 `AuthScheme::Basic` cannot express Freshdesk without downgrading the secret

Detailed in §4.1. In short: Freshdesk is `base64(<api_key>:X)`, and `AuthMethod::basic` composes
`base64(<user_env>:<env>)` where `user_env` is documented as non-secret config
(`../flux/crates/flux-plugin-protocol/src/lib.rs:433-434`). Using it as-is puts the API key outside
secret gating and outside redactor registration.

Options for C-16 to decide (this story does not decide it):

1. Let a Basic method declare **which position holds the secret**, with the other position taking a
   literal constant. Fits Freshdesk exactly; small change; no new scheme.
2. Add a distinct scheme for "API key as Basic username". Narrower, but a fourth Basic-ish variant.
3. Give `user_env` an optional "also gated as a secret" flag. Smallest diff, but leaves the
   secret in a field documented as non-secret — confusing rather than wrong.

Recommendation: **(1)**. It is the shape the API actually has, and it keeps one Basic scheme.
Whatever is chosen, the constant `X` is *not* a credential and must not be stored in an env var
pretending to be one.

### 6.3 action-proxy confirms the thesis it was mined from

Two artifacts worth keeping as evidence in the vision/pipeline argument:

- `freshdesk.yml:511` embeds an inline ternary in a config value:
  `per_page: ':params.max <= 100 ? params.max : 100'` — a homegrown expression language inside YAML,
  clamping a page size. In this repo that is Flux, with a parser and an analyzer behind it.
- The same collection needs **four** distinct sigils to mean different things (`:params.x`
  path-references, `{{params.x}}` template interpolation, `{consumer.number}` call-context
  substitution, and `$map`/`$modify`/`$emit`/`$array.map` pipeline directives), all in 649 lines. Two
  of them, `{{id}}` (`yml:93`) and `{{params.id}}` (`yml:288`), are used inconsistently for the same
  job **in the same file**.

### 6.4 Source defects in the freshdesk collection

Recorded so C-17 does not faithfully reproduce them:

| `path:line` | Defect | Our recording |
|---|---|---|
| `freshdesk.yml:69-74` | `updated` marked `required: true`; Freshdesk's `GET /tickets` does not require `updated_since`. A UI constraint leaking into the contract. | recorded **optional** |
| `freshdesk.yml:218-223` | `ticket.update`'s path parameter `id` is **not** required and defaults to a call-context variable; missing, it produces `PUT /tickets/`. | recorded **required** |
| `freshdesk.yml:312` | `ticket.addnote`'s path parameter `id` likewise not required. | recorded **required** |
| `freshdesk.yml:104-106` | The requester constraint — (`phone` AND `name`) OR `requester_id` — exists only as a prose `user_hint`; the `required:` flags contradict it by marking `phone` required and `requester_id` optional. | recorded as a **precondition**, §4.2 note ¹ |
| `freshdesk.yml:318` | `ticket.addnote`'s `body` parameter is described as *"Requester ID"* — copy-paste error. | described correctly |
| `freshdesk.yml:99-211` vs `:213-305` | `email` is updatable but has no create-time equivalent, though Freshdesk accepts it at create. | noted, not invented |
| `freshdesk.yml:111`, `:142`, `:156`, `:221` | Defaults are action-proxy call-context tokens (`{consumer.number}`, `{call.from}`) that mean nothing outside action-proxy. | **must be dropped**, §4.2 note ² |

### 6.5 Source defects in the babelforce spec

| Location | Defect | Impact |
|---|---|---|
| `manager-0.7.0.openapi.json:2472` (`listReportingCalls`) | **Every filter is declared twice**, flat and `filters.`-prefixed, with identical schemas — 40 parameters where ~20 are meant. Almost certainly a serializer emitting both bindings. | Un-curated ingest doubles the parameter surface with exact synonyms. We keep the flat form; §5.2. |
| same operation | `from`/`to` and `fromNumber`/`toNumber` share the `ReportingNumberFilter` schema; `agentId` and `agent.id` likewise. Which is canonical is **not stated anywhere in the document**. | We chose `fromNumber`/`toNumber`/`agentId`. **Unconfirmed — worth one question to the API owner before C-17.** |
| `:2472` (`time.start`, `time.end`, `agent.id`, `filters.*`) | Parameter names contain dots. | Not identifier-safe in Flux. C-8 needs an explicit wire-name-vs-symbol-name mapping; it cannot assume the parameter name is a usable identifier. |
| `:12` | `servers[0]` is **staging** (`latest.dev.babelforce.com`), not production. | A positional `servers[0]` default silently targets dev. Base URL must be explicit; §1.1. |
| `SetCallSessionVariablesRequest`, `UpdateSessionVariablesRequest` | Both are `{"type": "object"}` with no properties; the `app`-prefix rule for session variable keys lives only in prose. | C-9 needs a free-form-object body representation. The prefix rule is a C-12 quirk. |
| `UpdateAgentStatusRequest` | Body `required: false` with all properties optional — an empty PUT is valid and does nothing. | Preflight candidate, mirroring `main.rs:201-209`. |
| whole document | **Zero `deprecated` markers** in 17 281 lines, although at least one security scheme *is* deprecated. | Deprecation is not machine-readable for this provider. Overlays (C-6) are the only place it can live, which is why §5.1.3 is written as loudly as it is. |
| root `security` (`:17271`) vs the intended model | The document advertises a static header-pair alternative that the maintainers are removing. | An ingest that trusts the spec alone selects a credential type that is being switched off. |

### 6.6 The vendored spec is not cleared for publication

Detailed in §1.3, repeated here because it gates the merge: the babelforce spec embeds a
credential-shaped example (`accessId` + `accessToken` + a stream token for a `Testers Inc.` account)
and this repository is public. Confirm-and-rotate, then choose a vendoring policy, before
`specs/babelforce/manager-0.7.0.openapi.json` lands on a published branch. The recommended policy is
a **declared, reproducible scrub** with both hashes recorded, so drift-check stays honest.

Related and worth its own story: C-14's fetch path should refuse to vendor a spec containing
credential-shaped literals without an explicit acknowledgement.

### 6.7 Drift is undetectable for two of three providers

Only babelforce has a vendored spec, so only babelforce can be drift-checked (C-14). Zendesk and
Freshdesk are hand-derived, and a vendor-side change to either surfaces as a runtime failure rather
than a build-time diff. Two mitigations, neither in this story's scope:

- Zendesk: vendor the published OpenAPI document when C-14 lands (§1.2), which converts the §3 set
  into an overlay selection and makes drift visible.
- Freshdesk: with no official spec, the `freshdesk.test` op (§4.2) is the only automated signal, and
  it only proves auth works. The C-15 live end-to-end run is doing more work for Freshdesk than for
  the others and should be scoped accordingly.

---

## 7. Sources

All three were read directly, on disk. **No network access was used, and no source repository was
modified.**

| Provider | Source | Size | Read-only origin |
|---|---|---|---|
| zendesk | `../flux/plugins/zendesk/src/main.rs` | 687 lines | `/home/timo/projects/flux` — **not modified** |
| freshdesk | `action-proxy/dist/collections/freshdesk/freshdesk.yml` | 649 lines | `/home/timo/babelforce/projects/integrations/action-proxy` — **not modified** |
| freshdesk | `action-proxy/dist/collections/freshdesk/template.yml` | 19 lines | as above — **not modified** |
| babelforce | `babelforce-api/openapi/manager.openapi.json` → vendored as `specs/babelforce/manager-0.7.0.openapi.json` | 17 281 lines | `/home/timo/babelforce/projects/babelforce-api` — **not modified** |
| auth vocabulary | `../flux/crates/flux-plugin-protocol/src/lib.rs:344-470` | — | `/home/timo/projects/flux` — **not modified** |

### Selection summary

| Provider | Available | Selected | Ratio |
|---|---|---|---|
| zendesk | 7 | **7** | 100% — the plugin defines the requirement |
| freshdesk | 16 (12 HTTP + 4 forwarders) | **9** | 56% of HTTP actions |
| babelforce | 163 | **9** | 5.5% |
| **total** | **186** | **25** | **13%** |
