# Inventory: Anthropic Managed Agents, before any TOML

**Status:** complete — **and it contradicts its epic's premise in three places; read §What the
inventory found first** · **Pillar:** Spec · **Epic:** `anthropic-managed-agents` ·
**Story:** [C-445](../stories/C-445-managed-agents-endpoint-inventory.md) ·
**Parent design:** [anthropic-managed-agents.md](anthropic-managed-agents.md)

> ## Two kinds of claim in this document, and they are marked
>
> **Vendor claims** — every endpoint, method, path, header, pagination scheme and lifecycle rule
> below — are sourced from the bundled `claude-api` skill reference, read **2026-08-02**:
> `shared/managed-agents-api-reference.md`, `shared/managed-agents-core.md`,
> `shared/managed-agents-environments.md`, plus `-events`, `-tools`, `-memory`, `-multiagent`,
> `-webhooks`, `-outcomes`, `-scheduled-deployments`, `-self-hosted-sandboxes` and
> `-client-patterns` for the cross-references. **There is no vendored document** —
> `ls specs/` → `anthropic/2023-06-01-excerpt.yaml` (3.6 KB, measured this session: no Admin API, no
> Managed Agents), `babelforce/`, `flux/`, `zendesk/`. Nothing here was derived from recollection and
> nothing was fetched from the network. These are claims **about a vendor**, timestamped; re-read the
> reference before quoting them.
>
> **Repository claims** carry the command that produced them, inline, per `AGENTS.md`
> § Before you assert anything. All were measured on 2026-08-02 at
> `f53f6cc`.

## What this document is

The written inventory a Managed Agents connector would be built from: what exists, what a curated
set carries, how it partitions into services, what each operation's `risk` and `idempotency`
actually are, and — for everything **not** carried — the reason.

**It is not a connector, and it deliberately writes no TOML.** That ordering is the point.
[C-130](../stories/C-130-ivr-atomics-inventory.md) is the precedent: its inventory was written from
the source before any TOML, it contradicted the epic's premise on five counts, the TOML was never
written, and a fence test shipped instead. That was the right outcome and it was only reachable
because the inventory came first. This surface has the same risk profile — large, unvendored, and
assumed.

**`providers/anthropic.toml` is not edited by this story.** C-441 holds that file until it
integrates, and the TOML is C-446's. Measurements of it below are reads at `f53f6cc`
and will be stale the moment C-441 lands.

## The constraint that dominates: nothing is vendored

Every operation would be hand-authored, as `anthropic`'s `admin` service already is. That makes
[C-126](../stories/C-126-response-schema-coverage.md)'s rule the sharpest constraint in the epic and
the one this document is most disciplined about:

> A response field not known with confidence is left out, or left untyped with a note saying so —
> never guessed into a `required` list.

Where the reference is silent, this inventory says **unknown** and moves on. §What is unknown
collects every one of them in a single list, because a gap that is visible is a finding and a gap
that is filled is a fabrication. **No response shape is proposed anywhere in this document.** The
reference documents request bodies and lifecycle semantics thoroughly and response payloads barely
at all, so proposing one would mean inventing it.

---

## What the inventory found

Five findings. Three of them contradict the parent design, and the third is the one that decides
whether this epic should exist.

### 1. The two event vocabularies **collide by name**, so "two bindings on one service" does not hold

The parent design says the epic's shape is *"two channel bindings over two event sets on one
service — the shape `slack` proved with `socket` + `events-api`, but here the two halves carry
different vocabularies, which the model has never been asked to express."* The model cannot express
it, and the reason is one this repository already states.

`AGENTS.md` § Member contract, **One namespace per service**: all five member kinds share one
namespace, and *"a within-kind duplicate is reported by that kind's own pass"*. Two `[[events]]`
with the same `name` on one service is a loud error.

The SSE stream vocabulary and the webhook `data.type` vocabulary are different namespaces **at the
vendor**, and they overlap at three names:

| name | on the SSE stream | on the webhook | same thing? |
|---|---|---|---|
| `session.status_terminated` | yes | yes | different payloads, same trigger |
| `session.status_rescheduled` | yes | yes | different payloads, same trigger |
| `session.thread_created` | yes | yes | different payloads, same trigger |

And a fourth pair differs by **one letter**, which is worse than a collision because it compiles:
the stream emits `session.status_idle`; the webhook emits `session.status_idled`. The reference
states the separation explicitly — *"These are **webhook** `data.type` values — a separate namespace
from SSE event types … Don't reuse SSE constants in webhook handlers."*

So one service cannot carry both sets under the vendor's own spellings, and the member-name rule
that admits `.` exists precisely so the vendor's spelling survives. The options are (a) split the
two bindings across two services, (b) rename one set and lose the vendor's spelling, or (c) file a
gap. **This is [C-446](../stories/C-446-managed-agents-events-and-verification.md)'s to resolve** —
enumerating and reconciling the two vocabularies is its Acceptance, not this story's. It is recorded
here because it is a **service-partition input**, and the partition is this story's.

### 2. The beta header is a `const_headers` case — but **not the provider-level one** the story assumes

C-445's own Acceptance says the surface is *"beta-gated by `anthropic-beta: managed-agents-2026-04-01`,
which is a `const_headers` case (`ir.rs:328`), not a parameter."* The mechanism is right and the
**level is wrong**, and getting it wrong would put a beta header on five operations that must not
carry one.

Measured this session:

```
$ grep -n "const_headers" providers/anthropic.toml
148:const_headers = { "anthropic-version" = "2023-06-01" }
$ sed -n '2010,2029p' crates/connector-spec/src/provider.rs   # distribute_const_headers
```

`distribute_const_headers` copies the provider's table onto **every** operation in the file, and an
operation's own entry replaces the provider's only when the two name the same header
(case-insensitively). So:

- **Provider level is wrong here.** `anthropic.toml` already declares `anthropic-version` at
  provider level, and adding `anthropic-beta` beside it would send
  `managed-agents-2026-04-01` on `anthropic-models-list`, `anthropic-model-get` and the three admin
  reads too — five operations that are not beta-gated.
- **Per-operation is right, and it composes.** Because distribution is additive per header name, a
  Managed Agents operation declaring only `anthropic-beta` in its own `const_headers` still inherits
  `anthropic-version = "2023-06-01"` from the provider. Both travel; neither reaches a signature.
- **The two-beta case is one entry, not two.** The session-scoped file list needs
  `files-api-2025-04-14` *and* `managed-agents-2026-04-01`; on the wire that is one header with a
  comma-joined value, so it is one `const_headers` entry, not a collision.

`anthropic-version: 2023-06-01` is correct for this surface — the reference states every Managed
Agents endpoint requires it alongside the beta header — so the inherited value needs no override.

### 3. Pagination is **cursor**, not page, and the vendor's spelling is a trap

C-445's Acceptance records that this surface uses `page`/`next_page` rather than the
`after_id`/`before_id` scheme elsewhere in Anthropic's API. That is right, and the consequence is
sharper than "a different scheme".

The reference: `page` is *"an opaque cursor from a previous response — pass a `next_page` or
`prev_page` value here"*. It is **not** a page number.

```
$ grep -n "enum Pagination" -A 24 crates/connector-spec/src/ir.rs
423:pub enum Pagination {
425:    Page { page_param, size_param, page_size, max_pages }   # page_param carries a page NUMBER
438:    Cursor { cursor_param, next_cursor_pointer, max_pages }
```

`Pagination::Page`'s `page_param` is documented as "the query parameter carrying the page number"
(`?page=2&per_page=100`). Declaring `page = { page_param = "page", … }` here reads perfectly correct
— the parameter really is spelled `page` — and would emit a loop sending `?page=2`, which is not a
cursor this API issued. The honest declaration is the **Cursor** variant:
`cursor_param = "page"`, `next_cursor_pointer = "/next_page"`, plus a `max_pages` cap.

Two things the model cannot carry, both **unknown-by-omission rather than gaps to fill**:

- **`prev_page`.** Only `GET /v1/sessions` returns it; every other `page`-scheme endpoint omits the
  field entirely (not `null`). `Pagination::Cursor` is forward-only, which matches the SDKs'
  forward-only auto-pagination — so backward paging is simply not declared.
- **Order-bound cursors.** *"A cursor encodes the `order` of the request that produced it — reusing
  it with a different `order` returns 400."* There is no shape in `Quirks` for a parameter whose
  legal values depend on a cursor's provenance.

And it is currently inert either way. Measured this session — `quirks.pagination` is declared by
**6 operations across 3 providers**, matching `AGENTS.md` § Intentional gaps:

```
$ grep -rn "quirks.pagination\]" providers/*.toml
providers/babelforce.toml:583, :697   (patch.operations)
providers/zendesk.toml:191, :265
providers/twilio.toml:235, :314
```

The gaps table lists `quirks.pagination` as reaching **IR and loader only**. Declaring it here is
honest and buys nothing today.

### 4. Two credential-writing operations are **withheld by rule**, because a parameter cannot be secret

`POST /v1/vaults/{vault_id}/credentials` and `POST /v1/vaults/{vault_id}/credentials/{credential_id}`
take the raw secret in the request body — `access_token`, `refresh_token`, `client_secret`, or
`secret_value`, depending on the credential type. So does `POST /v1/sessions`, via
`resources[].authorization_token` (a GitHub PAT), and so does
`POST /v1/sessions/{id}/resources/{id}`, whose documented purpose *is* rotating that token.

Measured this session:

```
$ grep -n "pub struct Param" -A 50 crates/connector-spec/src/ir.rs | grep -in secret
(no match)
```

`Param` has `name`, `wire`, `description`, `required`, `schema` — and **no `secret` flag**. Only
`[[config]]` fields carry `secret = true`. An operation parameter holding an OAuth access token
would therefore be an ordinary, model-visible, unredacted string: in the `ToolSpec` a model reads,
in the arguments a model writes, and in every log of that tool call. There is no redaction path for
it, because the redactor is wired to credentials the *host* resolves — `AGENTS.md`
§ Authentication contract: *"The host resolves the credential … and registers values with its
redactor."*

This is the same family as `providers/babelforce.toml`'s three withheld auth-flow endpoints
(`/oauth/token`, `/oauth/authorize`, `/oauth/revoke`): operations that describe **how to
authenticate** rather than something a caller invokes and reads a result from. Credential
provisioning is the host's, and this repository's model is `[[auth]]` plus host resolution.

Note the asymmetry this produces, and that it is correct: **reading** credential metadata is
carried, because the reference states secret fields *"are write-only — never returned in API
responses"*. `GET .../credentials` and `GET .../credentials/{id}` return metadata only and are safe.
`DELETE` and `archive` carry too. It is exactly the two write paths that are withheld.

### 5. The premise-contradicting one: **the two planes are not independently valuable**

The parent design proposes *"management-plane yes, session-plane decided separately"* and calls the
management plane *"ordinary SaaS. Squarely in charter, same as `anthropic`'s existing `admin`
service."* The inventory does not support the second half of that sentence, and this bears directly
on [C-444](../stories/C-444-decide-managed-agents-charter.md) without deciding it.

**The vendor publishes the split itself, and it is not the split the design assumes.** From
`shared/anthropic-cli.md`, § *When to use the CLI vs the SDK*:

> **CLI for the control plane, SDK for the data plane.** Agents and environments are relatively
> static resources you define, configure, and debug with `ant` — check the YAML into your repo,
> apply from CI, inspect from a terminal. Sessions are dynamic and driven by your application
> through the SDK.

`shared/managed-agents-core.md` states the same rule as an **anti-pattern warning**:

> **Anti-pattern:** calling `agents.create()` at the top of every script run. … If you see
> `agents.create()` in a function that's called per-request or per-cron-tick, that's wrong — hoist
> it to one-time setup and persist the ID.

So the management plane is, by the vendor's own instruction, **provisioning performed once from a
CLI or CI and then not called again**. That is not adjacent to something this repository already
excludes — it is the thing this repository already excludes.
`docs/designs/provider-operation-inventory.md:717` drops roughly 120 babelforce manager operations
and names *applications* first, on the ground that it is account provisioning done in the vendor's
UI. Measured this session:

```
$ sed -n '717p' docs/designs/provider-operation-inventory.md
```

The consequence is a fork, and it is worth stating plainly **because it is the outcome C-444 makes
possible and nobody has costed**:

- If C-444 says **yes** to the session plane, the management plane earns its keep as that plane's
  prerequisite, and the epic is coherent.
- If C-444 says **no**, what remains is 37 carried operations for provisioning agents, environments,
  vaults and memory stores that no flow calls — a connector whose entire surface the vendor tells
  you to drive from `ant` and CI. The design's *"defensible outcome"* (management yes, session
  separately) is therefore **not obviously the safe middle**; it may be the outcome that ships a
  catalogued, addressable, callable surface with no caller, which is precisely what C-413's rework
  was about.

**This inventory does not answer C-444 and takes no side.** It records that the two planes are
coupled in a way the design's table treats as independent, so that C-444 answers the question it is
actually facing. One genuine counterweight belongs in the same breath: `memory_stores` is
**workspace-scoped and outlives every session**, so a flow that reads or writes a memory store
without ever creating a session is a real use, and it is the one management-plane surface that
survives a "no". It is 14 endpoints of the 67.

---

## The endpoint inventory

**80 endpoints are listed in the reference.** 67 sit behind `anthropic-beta: managed-agents-2026-04-01`;
the remaining 13 are the Files (5) and Skills (8) APIs, which the reference documents alongside
because Managed Agents uses them but which carry **their own beta headers** and are a separate
product surface. They are inventoried at the end and are not part of this epic's count.

Three-category accounting, `providers/babelforce.toml`'s shape — **carry / inexpressible /
withheld**, and every non-carried endpoint states which and why:

| category | count | meaning |
|---|---:|---|
| **carry** | 37 | expressible today, in charter today |
| **carry, gated on C-444** | 22 | expressible, but the session plane is undecided |
| **inexpressible** | 2 | the model has no shape for it |
| **withheld** | 6 | expressible, deliberately not carried, reason stated |
| | **67** | |

Per group, so the total is checkable rather than asserted:

| group | endpoints | carry | gated | inexpressible | withheld |
|---|---:|---:|---:|---:|---:|
| Agents | 6 | 6 | — | — | — |
| Sessions | 6 | — | 6 | — | — |
| Session events | 3 | — | 2 | 1 | — |
| Session threads | 5 | — | 4 | 1 | — |
| Session resources | 5 | — | 3 | — | 2 |
| Environments | 8 | 6 | — | — | 2 |
| Deployments + runs | 7 | — | 7 | — | — |
| Vaults | 6 | 6 | — | — | — |
| Credentials | 7 | 5 | — | — | 2 |
| Memory stores + memories + versions | 14 | 14 | — | — | — |
| **total** | **67** | **37** | **22** | **2** | **6** |

`37 + 22 + 2 + 6 = 67`. The gated column is a separate category rather than folded into "carry"
because C-444's answer moves it wholesale, and hiding it inside "carry" would be pre-empting that
decision in the permissive direction.

Legend for the tables: **R** = `Risk`, **I** = `Idempotency`, in this repository's vocabulary
(`crates/connector-spec/src/ir.rs:126` and `:144`, read this session). `Conditional` is used in the
C-186 sense the doc comment records — *safely repeatable because of what the endpoint does, a target
state rather than a delta* — and every use of it states its condition.

### Agents — 6 endpoints, 6 carried

| method | path | purpose | carry | R | I |
|---|---|---|---|---|---|
| `GET` | `/v1/agents` | list stored agent configurations | ✅ | Low | Idempotent |
| `POST` | `/v1/agents` | create a versioned agent (model, system, tools, MCP servers, skills) | ✅ | Medium | NonIdempotent |
| `GET` | `/v1/agents/{agent_id}` | retrieve one agent | ✅ | Low | Idempotent |
| `POST` | `/v1/agents/{agent_id}` | update — mints a new immutable version | ✅ | Medium | Conditional † |
| `POST` | `/v1/agents/{agent_id}/archive` | **terminal** — read-only, no unarchive | ✅ | **Destructive** | unknown ‡ |
| `GET` | `/v1/agents/{agent_id}/versions` | list versions | ✅ | Low | Idempotent |

† **Conditional, condition stated:** only when `version` is omitted. The reference makes `version`
optional with two distinct semantics — supplied, it is optimistic concurrency and a mismatch returns
409 (so a replay of the identical request fails the second time); omitted, the update is
unconditional last-write-wins over a target state, and replaying it reaches the same state. Omitted
fields are preserved and array fields are replaced wholesale, so the body is a partial target state
rather than a delta. Declaring `Conditional` **requires** `repeatable_because` to say this;
declaring it without the condition would be the loosened rule the doc comment refuses.

‡ **Unknown, deliberately.** The reference states that re-archiving an already-archived
*environment* or *memory store* "emits nothing", and says nothing at all about re-archiving an
agent. Whether the second call returns 200 or 409 is not stated, so no idempotency value is
declared. `NonIdempotent` is the honest floor if one must be chosen.

### Sessions — 6 endpoints, all gated on C-444

| method | path | purpose | carry | R | I |
|---|---|---|---|---|---|
| `GET` | `/v1/sessions` | list sessions | 🔒 | Low | Idempotent |
| `POST` | `/v1/sessions` | create a session against an agent + environment | 🔒 § | High | NonIdempotent |
| `GET` | `/v1/sessions/{session_id}` | retrieve one session | 🔒 | Low | Idempotent |
| `POST` | `/v1/sessions/{session_id}` | update `title`/`metadata`, or override tools/MCP/vaults | 🔒 | Medium | Conditional ¶ |
| `DELETE` | `/v1/sessions/{session_id}` | delete session, event history, container, checkpoints | 🔒 | **Destructive** | NonIdempotent |
| `POST` | `/v1/sessions/{session_id}/archive` | archive — read-only, not reversible | 🔒 | **Destructive** | unknown |

🔒 = gated on [C-444](../stories/C-444-decide-managed-agents-charter.md). Inventoried, not argued
either way.

§ **`POST /v1/sessions` carries only if `resources[]` is curated out.** Three separate reasons, each
sufficient: (1) the `github_repository` variant carries a raw PAT in `authorization_token`, which
Finding 4 withholds; (2) `resources[]` is a discriminated union of three shapes (`file`,
`github_repository`, `memory_store`) in one array field, and `AGENTS.md` § Intentional gaps lists
"an ambiguous free-form body" among the operations refused during emission; (3) `memory_store`
resources are attachable **only** at session-create time, so curating the array out also removes the
only way to attach one — which is a real loss, not a free simplification, and belongs in the
carry/withhold record rather than in a footnote nobody reads.

`Risk::High` rather than Medium: creating a session provisions a container and, with
`initial_events`, starts an agent loop that bills inference — *"a reviewer would want to see first"*
(`ir.rs:131`). See §Risk has no word for spend.

¶ **Conditional, condition stated:** the provided `tools`/`mcp_servers`/`vault_ids` arrays are **full
replacements**, so the body is a target state and a replay reaches the same state. The session must
be `idle`; a replay while `running` is rejected rather than misapplied.

### Session events — 3 endpoints, 1 carried-if, 1 split, 1 inexpressible

| method | path | purpose | carry | R | I |
|---|---|---|---|---|---|
| `GET` | `/v1/sessions/{id}/events` | list past events, paginated | 🔒 | Low | Idempotent |
| `POST` | `/v1/sessions/{id}/events` | send events into the session | 🔒 ✂ | *varies* | *varies* |
| `GET` | `/v1/sessions/{id}/events/stream` | SSE event stream | ❌ **inexpressible as an operation** | — | — |

✂ **One path, six operations.** `POST /v1/sessions/{id}/events` takes an `events[]` array whose
members are a discriminated union — `user.message`, `user.interrupt`, `user.tool_confirmation`,
`user.custom_tool_result`, `user.define_outcome`, `system.message` — with different bodies and
**wildly different risk**. Declaring it as one operation with a free-form `events` array is the
ambiguous free-form body the emitter refuses, and it would also make `risk` a lie: `user.interrupt`
carries an empty body and stops work, while `user.define_outcome` starts a rubric-graded
iterate → grade → revise loop of up to 20 iterations, each billing inference.

The precedent for splitting is already in the catalogue and was measured this session:
`providers/zendesk.toml:269-271` records three operations that are all
`PUT /api/v2/tickets/{ticket_id}.json`, *"distinguished **only** by request body"*. One operation per
event type is the honest shape, and the `risk`/`idempotency` pairs then differ per operation as they
should.

❌ **`events/stream` is never an operation.** It is a long-lived SSE stream; `Operation` models one
request and one response, and `AGENTS.md` § Member contract refuses *"an event dressed up as a
pollable op"*. It is a `[[channels]]` declaration with `transport = "socket"` — declared here,
executed by flux — and it is gated on C-444 like the rest of the plane.

### Session threads (multiagent) — 5 endpoints, all gated, 1 inexpressible

| method | path | purpose | carry | R | I |
|---|---|---|---|---|---|
| `GET` | `/v1/sessions/{sid}/threads` | list subagent threads | 🔒 | Low | Idempotent |
| `GET` | `/v1/sessions/{sid}/threads/{tid}` | retrieve one thread | 🔒 | Low | Idempotent |
| `POST` | `/v1/sessions/{sid}/threads/{tid}/archive` | archive a thread | 🔒 | **Destructive** | unknown |
| `GET` | `/v1/sessions/{sid}/threads/{tid}/events` | list one thread's events | 🔒 | Low | Idempotent |
| `GET` | `/v1/sessions/{sid}/threads/{tid}/stream` | per-thread SSE | ❌ **inexpressible** | — | — |

**The per-thread stream is inexpressible in a way the session stream is not, and this is a distinct
finding.** A `ChannelBinding` is a *static declaration* — one binding, declared at compile time,
naming a transport and an event set. The thread stream is **one stream per thread, up to 25
concurrent threads per session**, each opened against an id that does not exist until the
coordinator spawns it. There is no declaration shape for "N of these, discovered at runtime", and
the reference is explicit that previews and events are **thread-scoped** — *"A child thread's
previews are delivered on that child's stream and are never cross-posted to the session-level
stream."* So a single session-level binding does not subsume them.

Threads also have **archive but no delete**, like agents.

### Session resources — 5 endpoints, 3 gated-carry, 2 withheld

| method | path | purpose | carry | R | I |
|---|---|---|---|---|---|
| `GET` | `/v1/sessions/{sid}/resources` | list attached resources | 🔒 | Low | Idempotent |
| `POST` | `/v1/sessions/{sid}/resources` | attach a `file` or `github_repository` (SDK: `add`) | ⛔ **withheld** | — | — |
| `GET` | `/v1/sessions/{sid}/resources/{rid}` | retrieve one resource | 🔒 | Low | Idempotent |
| `POST` | `/v1/sessions/{sid}/resources/{rid}` | update — documented purpose is token rotation | ⛔ **withheld** | — | — |
| `DELETE` | `/v1/sessions/{sid}/resources/{rid}` | detach | 🔒 | **Destructive** | NonIdempotent |

⛔ Both withheld under Finding 4: the `github_repository` variant of `add`, and the whole of
`update`, take a raw GitHub PAT as a caller-supplied parameter, and `Param` has no `secret` flag.
The `file` variant of `add` is safe in isolation and could be carried as a narrowed operation
declaring only `{type: "file", file_id, mount_path}` — recorded as an option, not proposed, because
narrowing a union endpoint into one variant is a curation decision C-446 should make with the TOML
in front of it.

### Environments — 8 endpoints, 6 carried, 2 withheld

| method | path | purpose | carry | R | I |
|---|---|---|---|---|---|
| `POST` | `/v1/environments` | create a container template | ✅ | Medium | NonIdempotent ⁂ |
| `GET` | `/v1/environments` | list | ✅ | Low | Idempotent |
| `GET` | `/v1/environments/{eid}` | retrieve one | ✅ | Low | Idempotent |
| `POST` | `/v1/environments/{eid}` | update — applies to **new** containers only | ✅ | Medium | Conditional |
| `DELETE` | `/v1/environments/{eid}` | delete; returns 204 | ✅ | **Destructive** | NonIdempotent |
| `POST` | `/v1/environments/{eid}/archive` | **terminal** — read-only, no unarchive | ✅ | **Destructive** | Conditional ⁑ |
| `GET` | `/v1/environments/{eid}/work/stats` | self-hosted work-queue depth | ⛔ **withheld** | — | — |
| `POST` | `/v1/environments/{eid}/work/{work_id}/stop` | stop a claimed work item | ⛔ **withheld** | — | — |

⁂ Environment **names must be unique**; creating one with an existing name returns 409. So a replay
fails rather than duplicating — but "fails on replay" is not `Conditional`, which means *safe to
repeat*, so `NonIdempotent` is the honest value.

⁑ `Conditional`, condition stated: the reference says re-archiving an already-archived environment
*"emits nothing"*. Note the pairing — `Risk::Destructive` and `Idempotency::Conditional` are not in
tension: risk is a damage claim, idempotency is a replay claim, and an irreversible operation can be
safely repeatable precisely *because* the first call already reached the terminal state.

⛔ **The two `work` endpoints are withheld by scope, not by expressibility.** They authenticate with
`x-api-key` and would compile. They are withheld because they are the control-plane half of a
mechanism whose other half is a **long-poll worker loop** — `EnvironmentWorker.run()`, an outbound
poller holding a queue lease, credentialed by a *different* credential kind
(`ANTHROPIC_ENVIRONMENT_KEY`, `sk-ant-oat01-…`) that this connector does not model and that the
reference warns must never sit on a host where agent tools can read it. Publishing the two
observability calls without the loop publishes a dashboard for a runtime this repository does not
and must not have: `AGENTS.md` — *"Compiling a scheduler here would make this repository a runtime,
which `docs/vision.md` forbids."* If self-hosted sandboxes are ever in scope, they are their own
epic with their own credential.

### Deployments and deployment runs — 7 endpoints, all gated on C-444

| method | path | purpose | carry | R | I |
|---|---|---|---|---|---|
| `POST` | `/v1/deployments` | create a cron schedule that fires sessions | 🔒 | High | NonIdempotent |
| `POST` | `/v1/deployments/{did}/pause` | suppress scheduled triggers (reversible) | 🔒 | Medium | Conditional |
| `POST` | `/v1/deployments/{did}/unpause` | resume; **no backfill** of missed runs | 🔒 | Medium | Conditional |
| `POST` | `/v1/deployments/{did}/archive` | **terminal** — schedule stops, immutable | 🔒 | **Destructive** | unknown |
| `POST` | `/v1/deployments/{did}/run` | fire a session immediately; works while paused | 🔒 | High | NonIdempotent |
| `GET` | `/v1/deployment_runs?deployment_id=…` | list run records | 🔒 | Low | Idempotent |
| `GET` | `/v1/deployment_runs/{drid}` | retrieve one run | 🔒 | Low | Idempotent |

**Gated on C-444 even though scheduling is not itself the session plane.** A deployment is a session
factory: each firing creates a session and bills it, and `POST …/run` creates one synchronously. If
the session plane is refused, a deployment creates the thing that was refused, on a cron, without a
caller present. Grouping these under the gate rather than under "carry" is the conservative reading
and it is deliberate.

Worth recording for whoever writes the TOML: **archiving an agent auto-archives its deployments**,
terminally, and deleting an agent archives them at the next scheduled trigger. That is a
cross-resource cascade `risk` cannot express — `Destructive` on the agent archive describes damage
to the agent, not to the schedules that silently die with it.

### Vaults — 6 endpoints, 6 carried

| method | path | purpose | carry | R | I |
|---|---|---|---|---|---|
| `POST` | `/v1/vaults` | create a vault (`display_name`, `metadata`) — **holds no secret itself** | ✅ | Medium | NonIdempotent |
| `GET` | `/v1/vaults` | list | ✅ | Low | Idempotent |
| `GET` | `/v1/vaults/{vid}` | retrieve one | ✅ | Low | Idempotent |
| `POST` | `/v1/vaults/{vid}` | update | ✅ | Medium | Conditional |
| `DELETE` | `/v1/vaults/{vid}` | delete — **cascades to every credential inside** | ✅ | **Destructive** | NonIdempotent |
| `POST` | `/v1/vaults/{vid}/archive` | archive | ✅ | **Destructive** | unknown |

The vault *container* carries cleanly: creating one takes a display name and metadata, not a secret.
It is the credentials inside it that Finding 4 withholds.

### Credentials — 7 endpoints, 5 carried, 2 withheld by rule

| method | path | purpose | carry | R | I |
|---|---|---|---|---|---|
| `POST` | `/v1/vaults/{vid}/credentials` | **create — takes the raw secret** | ⛔ **withheld by rule** | — | — |
| `GET` | `/v1/vaults/{vid}/credentials` | list — metadata only, secrets write-only | ✅ | Low | Idempotent |
| `GET` | `/v1/vaults/{vid}/credentials/{cid}` | retrieve metadata | ✅ | Low | Idempotent |
| `POST` | `/v1/vaults/{vid}/credentials/{cid}` | **update / rotate — takes the raw secret** | ⛔ **withheld by rule** | — | — |
| `DELETE` | `/v1/vaults/{vid}/credentials/{cid}` | delete | ✅ | **Destructive** | NonIdempotent |
| `POST` | `/v1/vaults/{vid}/credentials/{cid}/archive` | archive — purges the secret, frees the key | ✅ | **Destructive** | unknown |
| `POST` | `/v1/vaults/{vid}/credentials/{cid}/mcp_oauth_validate` | validate a stored MCP OAuth credential | ✅ | Low | Idempotent ⌘ |

⌘ Carried on the ground that it supplies **no** secret — it exercises one already stored. Its
response shape is **unknown** (see §What is unknown): the reference names the endpoint and its
purpose and documents no response body. It would ship with an untyped response and a note saying so,
per C-126, rather than with a guessed `{valid: boolean}`.

### Memory stores, memories, memory versions — 14 endpoints, 14 carried

The one management-plane surface that survives a "no" on C-444: memory stores are
**workspace-scoped and persist across sessions**, so reading and writing one is a use that needs no
session at all.

| method | path | purpose | carry | R | I |
|---|---|---|---|---|---|
| `POST` | `/v1/memory_stores` | create a store | ✅ | Medium | NonIdempotent |
| `GET` | `/v1/memory_stores` | list (`include_archived`, `created_at_{gte,lte}`) | ✅ | Low | Idempotent |
| `GET` | `/v1/memory_stores/{msid}` | retrieve one | ✅ | Low | Idempotent |
| `POST` | `/v1/memory_stores/{msid}` | update | ✅ | Medium | Conditional |
| `DELETE` | `/v1/memory_stores/{msid}` | delete — cascades to memories and versions | ✅ | **Destructive** | NonIdempotent |
| `POST` | `/v1/memory_stores/{msid}/archive` | **terminal** — read-only, no unarchive | ✅ | **Destructive** | Conditional |
| `GET` | `…/{msid}/memories` | list; `path_prefix`, `depth`, `view` | ✅ | Low | Idempotent ⊕ |
| `POST` | `…/{msid}/memories` | create at `path`; 409 if occupied | ✅ | Medium | NonIdempotent |
| `GET` | `…/{msid}/memories/{mid}` | read one (defaults to `view=full`) | ✅ | Low | Idempotent |
| `PATCH` | `…/{msid}/memories/{mid}` | update content and/or path | ✅ | Medium | Conditional ⊗ |
| `DELETE` | `…/{msid}/memories/{mid}` | delete | ✅ | **Destructive** | NonIdempotent |
| `GET` | `…/{msid}/memory_versions` | list immutable snapshots, newest first | ✅ | Low | Idempotent |
| `GET` | `…/{msid}/memory_versions/{vid}` | retrieve one version with content | ✅ | Low | Idempotent |
| `POST` | `…/{msid}/memory_versions/{vid}/redact` | scrub content, preserve actor + timestamps | ✅ | **Destructive** | Conditional |

`PATCH` is expressible — `HttpMethod::Patch` exists (`crates/connector-spec/src/ir.rs:110`, read this
session), and no operation in the catalogue would be the first to need it to work.

⊕ **Heterogeneous list.** `ListMemories` returns `Memory | MemoryPrefix`, discriminated by a `type`
field (`memory_prefix` entries are directory-like nodes carrying only a `path`). The precedent for
declaring this honestly is `providers/zendesk.toml:189` — `items = { type = "object" }` plus a
description naming the discriminator and saying *"Read `result_type` before reading anything else"*.
Same shape, same treatment.

⊗ **`Conditional`, and this one has a real condition to state:** `UpdateMemory` accepts a
`precondition` of `{type: "content_sha256", …}` and returns 409 on mismatch. With the precondition
supplied it is a compare-and-swap; without it, a target-state write. Either way a replay of the
identical request is safe.

**Memory versions have neither delete nor archive** — only `redact`. That is the fourth distinct
lifecycle shape on this surface, and §Archive is terminal tabulates all of them.

### Adjacent: Files and Skills — 13 endpoints, not this epic's

Both carry **different beta headers** (`files-api-2025-04-14`, `skills-2025-10-02`) and are separate
product surfaces the reference documents alongside because Managed Agents consumes them. They are
inventoried for completeness and excluded from the 67.

| method | path | disposition |
|---|---|---|
| `POST` | `/v1/files` | ❌ **inexpressible** — `multipart/form-data` |
| `GET` | `/v1/files` | expressible; `scope_id` variant needs both beta headers, one comma-joined `const_headers` entry |
| `GET` | `/v1/files/{fid}` | expressible (metadata) |
| `GET` | `/v1/files/{fid}/content` | ❌ **inexpressible / unknown** — arbitrary non-JSON bytes |
| `DELETE` | `/v1/files/{fid}` | expressible |
| *(8)* | `/v1/skills…` | ⛔ withheld by scope — separate beta, separate surface, upload-based create |

**The upload is inexpressible for a reason already settled.** `BodyEncoding` is `Json | Form` and
this IR has no third value; `providers/babelforce.toml`'s header records that
[C-426](../stories/C-426-multipart-body-encoding.md) established this is *not ours to close* — flux cannot
carry a multipart body at all, so describing one in the IR would produce a module that fails on a
real call. Five babelforce operations are named and not emitted for exactly this; `POST /v1/files`
is the sixth of its kind.

**The download raises an open question this repository has asked before and not answered.** C-130's
re-scope note ends on it verbatim: *"may an operation declare a non-JSON response (`image/png`,
`audio/wav`)?"* It is still open, and `GET /v1/files/{fid}/content` is now a second surface waiting
on it. Recorded as a finding, not resolved here.

---

## Proposed service partition

**Two services: `agents` (management plane) and `sessions` (session plane).**

A service is load-bearing in three ways, and only one of them discriminates here:

| what a service owns | does it force a split on this surface? |
|---|---|
| `base_url` | **No.** All 67 endpoints are `https://api.anthropic.com`. |
| `api_version` | **No.** All 67 are `/v1/`. The dated axis is the `anthropic-version` header, already carried in `const_headers`. |
| credential addressing — `tenants/<tenant>/<authority>[/@instances/<uuid>][/<service>]/<credential>` (`AGENTS.md` § Credential addressing contract) | **No.** All 67 use the ordinary `x-api-key` / `ANTHROPIC_API_KEY`, not the admin key. |

That last row is worth dwelling on, because it is exactly why `anthropic` is split today. Measured
this session at `providers/anthropic.toml:163-181`: `models` uses `anthropic.api_key` and `admin`
uses `anthropic.admin_key` — *"A second, distinct secret for the Admin API only"*. **The existing
split exists because the credentials differ. Managed Agents introduces no third credential**, so the
mechanism that justified the last split does not justify this one.

### So why split at all — one reason, and it is decisive

**Draw the service boundary exactly on C-444's boundary, so the decision moves whole services rather
than carving a hole through the middle of one.**

If the whole surface is one service and C-444 says no, 26 of its 67 endpoints vanish and the
remainder is a service defined by what was removed from it. If the boundary is the plane boundary,
"management-plane only" is expressible as *the `sessions` service is not declared* — no orphaned
operations, no partial service, and the epic's scope change is one deletion.

This matters more than usual because **an address, once published, is not reused**
(`AGENTS.md` § Service contract). A service name reaches the emitted file name
(`anthropic-<service>.flux`) and the rendered address (`com.anthropic.api/<service>:v1`). Getting the
partition wrong is not a refactor; it is a deprecation plus a re-provisioning for every tenant.

### The allocation

Counted as **endpoints the service covers**, carried or not — a withheld endpoint still belongs to a
service, it just emits nothing:

| service | endpoints | of which carried | contents |
|---|---:|---:|---|
| `agents` | 41 | 37 | agents (6), environments (8), vaults (6), credentials (7), memory stores + memories + versions (14) |
| `sessions` | 26 | 22 gated, 0 unconditional | sessions (6), session events (3), threads (5), resources (5), deployments + runs (7) |

`41 + 26 = 67`. Note what the second row says: **the `sessions` service has no unconditionally
carried endpoint at all.** Every one of its 26 is gated on C-444 or inexpressible. That is the
partition argument restated as a measurement.

### Alternatives considered, and why not

- **One service (`managed-agents`).** Simplest, one settings page, one install unit. Rejected on the
  C-444 argument above: the answer would carve the service rather than remove one.
- **Five services (`agents`, `sessions`, `environments`, `vaults`, `memory`)** — the split C-445's
  Acceptance floats. Finer install granularity, and `memory` genuinely stands alone (Finding 5).
  Rejected for now: it mints five permanent addresses and five emitted modules for one product
  surface, on a `base_url`/`api_version`/credential axis that discriminates on none of them.
  **`memory` is the one worth revisiting** if C-444 says no, because it is then the only surface with
  a caller.
- **A third service for the webhook binding.** Not proposed here, but Finding 1 means *something*
  must give, and a `notifications` service is one of the three options. That is C-446's call.

### One naming caution

`agents` as a service name sits beside `agents` as a resource, so
`com.anthropic.api/agents:v1#anthropic-agents-list` reads redundantly. `management` avoids it but
reads worse next to the existing `models` and `admin`. Recorded as a choice with a cost, not a
blocker — and one that cannot be revisited after publication.

### And the member-namespace constraint the partition must respect

One namespace per service, across all five member kinds. The `sessions` service would hold its
operations **and** both channel bindings **and** both event vocabularies — which is where Finding 1
bites. Whatever C-446 concludes, the partition and the event naming have to be decided together, not
in sequence.

---

## `archive` is terminal, and the lifecycle shapes are not uniform

**On agents, environments and memory stores, `archive` is terminal: the resource becomes read-only,
existing sessions continue, new sessions cannot reference it, and there is no unarchive.** Every
such operation's `risk` is `Destructive` in the tables above, and this section says so explicitly
rather than leaving a reader to infer irreversibility from the word `destructive` — which reads as
"deletes something" and here means "removes a resource from all future use while leaving it
visible".

`Risk::Destructive` is defined as *"Deletes or otherwise irreversible"* (`ir.rs:133-134`, read this
session). The **"or otherwise irreversible"** clause is what carries archive, and it is the half
people forget. A reviewer seeing `Destructive` on `anthropic-agent-archive` should understand: the
agent is not deleted, it is frozen forever.

Four distinct lifecycle shapes, from the reference:

| resource | delete | archive | note |
|---|:-:|:-:|---|
| Agents | — | ✅ terminal | archive is the **only** removal, and it is permanent |
| Session threads | — | ✅ | archive-only |
| Environments, Sessions, Vaults, Credentials, Memory stores | ✅ | ✅ | both; archive terminal for environments and memory stores |
| Session resources, Files, Skills, Memories | ✅ | — | delete-only |
| Memory versions | — | — | neither — only `redact` |

The reference states the operational consequence in its own voice, and it belongs in whatever TOML
eventually ships: *"Never archive a production agent as routine cleanup — confirm with the user
first."* Sessions are the exception — per-run and disposable, archiving one is routine — and the
reference warns explicitly against generalizing that to agents or environments.

---

## `Risk` has no word for spend

C-444 already names this; the inventory supplies the concrete instances, because they are sharper
than the abstract statement.

`Risk` is `Low | Medium | High | Destructive`, and every value is a claim about **damage or
reversibility**. None of them describes an operation that is fully reversible, destroys nothing, and
spends unbounded money:

- `POST /v1/sessions` with `initial_events` provisions a container and starts an agent loop.
- `POST /v1/sessions/{id}/events` with `user.define_outcome` starts an iterate → grade → revise loop
  of up to **20 iterations**, each running the agent *and* a separate grader model.
- `POST /v1/deployments` creates a **cron schedule** that does the first of these repeatedly, with
  nobody watching, until archived.

`High` — *"Writes a reviewer would want to see first"* — is the closest honest fit and is what the
tables declare, but it under-describes the third case badly: a reviewer approving one deployment
creation is approving an unbounded series of future billed runs. Not this story's to fix. Filed as a
finding against `Risk` for whoever picks up C-444, since C-444 is where the cost question already
lives.

---

## What is unknown

Everything the reference does not state with confidence, in one place. **None of these was filled
in.** Per C-126, a gap that is visible is a finding; a gap that is filled is a fabrication that looks
derived.

**Response shapes — the largest category.** The reference documents request bodies and lifecycle
semantics thoroughly and response payloads barely at all. It gives field tables for the **Session**
object (`shared/managed-agents-core.md`) and the **Agent** object, partial JSON for a
`deployment_run` and a `span.outcome_evaluation_end` event, and effectively nothing for the rest. So:

- **No `response_schema` is proposed in this document for any endpoint.** Not one. A connector built
  from this inventory would ship most operations with an untyped response and a note saying the
  vendor reference does not document it — which is what C-126's rule prescribes and what
  `providers/calendly.toml:214` already does in prose for a location field
  (*"no single object shape is declared here rather than guessing one"*).
- Specifically unknown: the response of `mcp_oauth_validate`; the response of every `archive`
  endpoint; the body (if any) of `DELETE /v1/environments/{id}` beyond its 204; the `MemoryPrefix`
  variant's full field set; the `work/stats` payload beyond the four field names quoted.

**Idempotency on re-archive.** Stated for environments and memory stores (*"emits nothing"*).
**Not stated** for agents, sessions, threads, vaults, credentials or deployments. Six endpoints
carry `unknown` above rather than an inferred `Conditional`.

**Idempotency keys.** The reference documents none anywhere on this surface. That is an absence of
evidence, not evidence of absence — recorded as unknown rather than as "there are none".

**Rate-limit quirks.** The reference gives per-organization RPM figures (300 create / 600 other on
agents, sessions and vaults; 60 RPM and 5 concurrent on environments). Whether these belong in
`quirks.rate_limit` is a live question this inventory does not settle: `providers/hubspot.toml`
records a **deliberate non-declaration**, and `AGENTS.md` § Intentional gaps lists
`quirks.rate_limit` as having *no consumer **and** no producer*. Declaring the first one in the
catalogue on a surface nobody has called yet is not obviously right.

**Whether `expose = false` should apply.** 59 carried-or-gated endpoints is far past the point
where `providers/babelforce.toml`'s header argues *"389 LLM tools is not a catalogue, it is a denial
of service against a model's context"*. Some subset should be catalogued-but-not-exposed. Which
subset is a curation decision needing the TOML in hand; the field exists
(`crates/connector-spec/src/ir.rs:1116`, read this session) and the question is flagged, not
answered.

---

## What this hands to which story

| finding | goes to |
|---|---|
| The two planes are not independently valuable (Finding 5) | [C-444](../stories/C-444-decide-managed-agents-charter.md) — as input, not as an answer |
| `Risk` has no word for unbounded spend | C-444, where the cost question already lives |
| Event-name collision across the two vocabularies (Finding 1) | [C-446](../stories/C-446-managed-agents-events-and-verification.md) — it owns the event set |
| Per-operation `const_headers`, not provider-level (Finding 2) | C-446, when the TOML is written |
| `Pagination::Cursor` not `::Page`, despite the spelling (Finding 3) | C-446 |
| Vault credential writes withheld by rule (Finding 4) | C-446 |
| `POST …/events` splits into six operations | C-446 |
| Per-thread SSE has no declaration shape | C-446, as a model gap in the shape C-141/C-188 used |
| Non-JSON response bodies still undecided | still open; C-130's re-scope note asked first |
| `expose = false` curation for ~57 operations | C-446 |

## Notes for whoever writes the TOML

- **Managed Agents is Claude API only** — and Claude Platform on AWS, which is Anthropic-operated
  with same-day parity and bare model ids. It is **not** on Amazon Bedrock or Google Vertex AI, and
  `shared/platform-availability.md` marks Microsoft Foundry unsupported while labelling that row
  **inferred** ("not in Foundry docs either way") — so Foundry is *unknown*, not *no*, and this
  document does not upgrade the vendor's own hedge. `anthropic.toml`'s `base_url` is
  `https://api.anthropic.com` (read this session at `:145`), which implies nothing wider, so the
  Notes item in C-445 is satisfied by the existing declaration — no correction needed.
- **`content-type: application/json`** remains hard-coded in the emitter and undeclarable (C-144).
  It does not arise for the GETs; it does arise for every POST and PATCH here, which is a change from
  `anthropic.toml`'s current state where *"every operation below is a parameterless or path/query-only
  GET and carries no body"* (read this session at `:41-42`). This connector would be the first
  Anthropic operation with a request body.
- **Op ids stay hyphen-separated**, not dotted — flux-lang's `is_valid_decl_name` admits ASCII
  alphanumerics, `_` and `-` only (C-8/C-23). `anthropic-agent-list`, not `anthropic.agents.list`.
  **Event names are the exception** and keep the vendor's dots, because a member name is wider than
  an operation id.
- **Do not add `anthropic-beta` to the provider-level `const_headers` table.** Finding 2.
