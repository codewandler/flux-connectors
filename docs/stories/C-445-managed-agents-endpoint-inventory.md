---
id: C-445
title: "Inventory the Managed Agents surface before any TOML is written"
pillar: Spec
status: done
priority: 2
design: docs/designs/anthropic-managed-agents.md
epic: anthropic-managed-agents
areas: [providers]
note: "C-130's lesson applied up front — that inventory contradicted its own epic's premise and the TOML was never written. No vendored document exists here, so this is hand-authored and C-126's do-not-invent rule binds hardest"
---

# Inventory the Managed Agents surface before any TOML is written

## Goal

Produce the written inventory the connector would be built from: which endpoints exist, which belong
in a curated set, how they partition into services, what each one's risk and idempotency actually
are, and — for every endpoint **not** carried — the reason.

## Why an inventory story at all

[C-130](C-130-ivr-atomics-inventory.md) is the precedent and the warning. Its inventory was written
from the source rather than from the epic's assumptions, and it **contradicted the premise**: five
independent findings, no TOML written, and a fence test shipped instead. That was the right outcome
and it was only reachable because the inventory came first. This epic has the same risk profile —
a large surface, no vendored document, and an assumed shape.

## The constraint that dominates: nothing is vendored

`specs/` holds `anthropic/2023-06-01-excerpt.yaml` (3.6 KB — no Admin API, no Managed Agents),
`babelforce/`, `flux/`, `zendesk/`. Builds are hermetic and offline, so there is no spec route here:
every operation is hand-authored, as `anthropic`'s `admin` service already is.

**This makes [C-126](C-126-response-schema-coverage.md)'s rule the sharpest constraint in the epic.**
A response field not known with confidence is left out, or left untyped with a note saying so — never
guessed into a `required` list. An invented schema that looks derived is worse than no schema. Where
the reference is silent, the inventory says "unknown", and that is a finding rather than a gap to
fill.

## Acceptance

- [x] Every Managed Agents endpoint is listed with method, path, and a one-line purpose, sourced from
      the bundled `claude-api` skill reference (`shared/managed-agents-api-reference.md`) — invoke the
      skill; do not answer from memory.
      → [`docs/designs/managed-agents-inventory.md`](../designs/managed-agents-inventory.md)
      §The endpoint inventory. **80 endpoints**: 67 behind `managed-agents-2026-04-01`, plus Files (5)
      and Skills (8), which carry their own beta headers and are inventoried separately as adjacent.
      The skill was invoked (`Skill: claude-api`); nothing was WebFetched and nothing was recalled.
- [x] Each endpoint is marked **carry / withhold**, and **every withheld one carries its reason**.
      `providers/babelforce.toml` is the model for this three-category accounting (emitted /
      inexpressible / withheld).
      → four categories rather than three, and the fourth is the point: **carry 37 / gated-on-C-444
      22 / inexpressible 2 / withheld 6 = 67**, with a per-group table so the total is checkable.
      "Gated" is separate from "carry" so that folding it in does not pre-empt C-444 permissively.
- [x] A proposed **service partition** with its rationale — one service or several (`agents`,
      `sessions`, `environments`, `vaults`, `memory`). The service is load-bearing: it owns
      `base_url` and `api_version`, and it keys credential addressing. Note the whole surface is
      beta-gated by `anthropic-beta: managed-agents-2026-04-01`, which is a `const_headers` case
      (`ir.rs:328`), not a parameter.
      → §Proposed service partition: **two services, `agents` (41) and `sessions` (26)**, drawn on
      C-444's boundary so the decision moves a whole service. All three load-bearing axes were
      checked and **none** discriminates here — one `base_url`, one `api_version`, one credential —
      so the partition rests solely on the C-444 argument, and the five-way alternative is recorded
      with its cost. **The `const_headers` half of this item is wrong as written and Finding 2
      corrects it**: measured at `provider.rs:2010`, provider-level `const_headers` distributes onto
      *every* operation, which would beta-gate the five existing non-beta ops. It must be
      per-operation.
- [x] **`archive` is terminal on agents, environments and memory stores — no unarchive.** Every such
      operation's `risk` reflects that, and the inventory says so explicitly rather than leaving a
      reader to infer it from `destructive`.
      → §`archive` is terminal. Every archive endpoint is `Risk::Destructive` in the tables, and the
      section states the irreversibility in prose plus a four-shape lifecycle table (archive-only /
      both / delete-only / redact-only), because `Destructive` reads as "deletes" and here means
      "frozen forever while still visible".
- [x] Session-plane endpoints are inventoried but flagged as **gated on
      [C-444](C-444-decide-managed-agents-charter.md)**; the inventory does not pre-empt that decision.
      → all 26 marked 🔒, including deployments (a deployment is a session factory on a cron).
      **Finding 5 is input to C-444 and deliberately answers nothing**: it records that the two planes
      are not independently valuable — the vendor's own CLI-vs-SDK split puts the whole management
      plane in the provision-once-from-CI bucket this repository already excludes
      (`provider-operation-inventory.md:717`) — and names `memory_stores` as the one surface that
      survives a "no". Both branches are costed; neither is chosen.
- [x] Pagination is recorded: this surface uses `page`/`next_page`, unlike the `after_id`/`before_id`
      scheme elsewhere in Anthropic's API. Note `quirks.pagination` reaches no artifact today.
      → Finding 3, and it is sharper than the item assumed: despite the spelling, `page` carries an
      **opaque cursor**, so `Pagination::Page` (whose `page_param` is documented as a page *number*,
      `ir.rs:425`) is the wrong variant and would emit `?page=2`. `Pagination::Cursor` with
      `cursor_param = "page"`, `next_cursor_pointer = "/next_page"` is the honest declaration.
      `prev_page` and order-bound cursors have no shape. Re-measured this session: 6 declarations
      across 3 providers (zendesk 2, twilio 2, babelforce 2), reaching IR and loader only.
- [x] Anything the reference does not state with confidence is recorded as **unknown**, not filled in.
      → §What is unknown. **No `response_schema` is proposed anywhere in the document, for any
      endpoint** — the reference documents request bodies thoroughly and response payloads barely at
      all, so every one would have been a guess. Also unknown and left so: re-archive idempotency on
      six resources, idempotency keys (absence of evidence, not evidence of absence), and whether
      rate limits belong in `quirks.rate_limit`.
- [x] **No `providers/anthropic.toml` edit in this story.** The inventory is a document; the TOML is
      C-446, and C-441 holds that file until it integrates.
      → `git status --short` shows exactly two paths, both under `docs/`. `providers/anthropic.toml`
      was **read** (to measure the existing services, credentials and `const_headers`) and not
      written; those measurements are labelled as reads at `f53f6cc` and will be stale once C-441
      lands.

## Progress

**Done, and it contradicts the epic's premise in three places — which is the outcome C-130's
precedent exists to make legible.** The inventory is
[`docs/designs/managed-agents-inventory.md`](../designs/managed-agents-inventory.md); no TOML was
written and none should be until C-444 and C-446 have read §What the inventory found.

The five findings, one line each:

1. **The two event vocabularies collide by name, so "two channel bindings on one service" does not
   hold.** `session.status_terminated`, `session.status_rescheduled` and `session.thread_created`
   each appear in *both* the SSE and webhook vocabularies with different payloads, and
   `session.status_idle` / `session.status_idled` differ by one letter. One service is one member
   namespace (`AGENTS.md` §Member contract), so a within-kind duplicate is a loud error. The parent
   design's "the shape `slack` proved" is wrong: slack's two bindings shared *one* vocabulary.
   Resolution is C-446's; it is recorded here because it constrains the partition.
2. **The beta header is per-operation `const_headers`, not provider-level.** Provider-level would
   beta-gate `anthropic-models-list`, `anthropic-model-get` and the three admin reads. Distribution
   is additive per header name, so a per-operation `anthropic-beta` still inherits
   `anthropic-version`.
3. **Pagination is cursor, not page, despite being spelled `page`** — the correct-reading wrong
   declaration is the trap.
4. **Four credential-writing endpoints are withheld by rule**, because `Param` has no `secret` flag
   (only `[[config]]` does): a raw OAuth token or GitHub PAT would be a model-visible, unredacted
   string in the `ToolSpec`. Same family as babelforce's three withheld auth-flow endpoints. Reads of
   credential *metadata* are carried, since the vendor makes secret fields write-only.
5. **The two planes are not independently valuable.** The vendor instructs that agents and
   environments are control-plane resources applied once from `ant`/CI, and calls per-request
   `agents.create()` an anti-pattern. A management-plane-only connector is therefore a provisioning
   surface — the category `provider-operation-inventory.md:717` already drops ~120 babelforce
   operations for. So the design's "defensible outcome" is not obviously the safe middle; it may ship
   a catalogued, callable surface with no caller (C-413's shape). `memory_stores` (14 endpoints) is
   the one part that survives a "no".

**Also filed, smaller:** `POST /v1/sessions/{id}/events` is six operations, not one (a discriminated
union of event types with wildly different risk — `zendesk.toml:269-271` is the split precedent); the
per-thread SSE stream is inexpressible in a way the session stream is not (N-per-session, discovered
at runtime, and `ChannelBinding` is static); `Risk` has no word for unbounded spend
(`user.define_outcome` runs up to 20 billed iterations from one request); and non-JSON response
bodies are still undecided — C-130's re-scope note asked the same question first.

**Gate:** docs-only diff, so no behavior changed and no failing-first test exists to write.
`cargo test --workspace` was run anyway and is green.

## Notes
- Write the inventory into `docs/designs/anthropic-managed-agents.md` or a sibling doc; do not put it
  in the story body.
- Managed Agents is not a Bedrock/Vertex/Foundry surface — it is Claude API (and Claude Platform on
  AWS) only. If the connector's `base_url` implies otherwise, say so.
