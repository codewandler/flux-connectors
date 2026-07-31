---
id: C-162
title: Ship the PagerDuty connector
pillar: Spec
status: done
priority: 2
design:
epic: provider-fleet-2
areas: [providers]
note: "PROBE: `Authorization: Token token=<key>` — a prefix containing `=`, so the credential is a *substructure* of the header value, not a suffix"
---

# Ship the PagerDuty connector

## Goal

Add PagerDuty to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**A credential embedded in structured header syntax.** PagerDuty wants `Authorization: Token token=<key>`. The credential is not the tail of the value, it is a field inside it.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** `Authorization: Token token=<key>`, plus a required `From` header carrying an actor email on writes.

**Curated operation set (a starting point, not a mandate):** list incidents, get an incident, list services, list on-call, acknowledge/resolve an incident (a write)

## Hazards specific to this one

The `From` header is **not** a credential and must not be modelled as one; it is operator configuration or a caller argument, and which it is, is a real decision — say which and why. Do not put a person's email in the TOML: an example address is customer data, and this repository authors no personal data.

## Acceptance

- [x] `providers/pagerduty.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
- [x] A `verify` operation that is a read and runs unattended.
- [x] `crates/connector-flux/tests/pagerduty_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses.
- [x] **Failing-first test:** the contract test must fail before `providers/pagerduty.toml` exists.
- [x] The scoped gate is green: `build --provider pagerduty`, `diff --provider pagerduty` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. They are red because you correctly did not write a coordinator-owned
      artifact. Report the eight; if the number differs, that is the finding.

## Notes

- **Charter fit.** PagerDuty is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
  non-goals exclude technology adapters and the inference path. If authoring reveals it is really a
  technology rather than a service, stop and say so rather than shipping it.
- **Provenance is hand-authored and drift is undetectable by machine**, the caveat zendesk, freshdesk,
  github and notion already carry. Record it in the TOML's header comment.
- **Do not invent an endpoint.** A confident set of four operations beats a guessed set of ten — the
  repository's rule is that a loud refusal beats plausible-but-incorrect output, and a hallucinated
  path ships as a `404` a contract test cannot catch. Where you are unsure of a path, shape or required
  field, leave the operation out and name it in `## Progress` as unverified.
- **No credential value, and no realistic-looking `example` on a secret field.** A placeholder shaped
  like a real token has already tripped GitHub's push protection and blocked a release.
- Whole-catalogue artifacts are coordinator-owned: `crates/catalog/src/generated.rs`,
  `web/public/catalog.json`, `web/public/v1/**`, `assets/readme-snippet-*.svg`. The per-provider
  `crates/catalog/src/generated/pagerduty.rs` is **not** in that set and is yours to commit.

## Progress

**Shipped.** Six curated operations, four reads and two writes, over the prefix axis C-184 built.

### The probe's answer: the framing in this story's `note` was wrong, and C-184 already settled it

This story was filed on the claim that `Authorization: Token token=<key>` makes the credential a
*substructure* of the header value rather than a suffix, and that PagerDuty would therefore force a
richer placement model. **It did not.** C-161 measured three vendors side by side and found one
shape — in Okta, Statuspage and PagerDuty alike the credential *ends* the header value — and C-184
(`f282e0a`) built a `prefix` axis alone, deliberately with no `suffix` and no value template, for
reasons `docs/designs/unified-auth.md` §"The prefix axis, as built" records. `=` is a character in a
prefix. So this connector needed no change to `connector-spec`, and it spells its auth in one line:

```toml
scheme = { header = { name = "Authorization", prefix = "Token token=" } }
```

It is the **first shipped provider to use the axis**; until now only
`crates/connector-spec/tests/auth_prefix.rs` exercised it, against a fixture.

### The `From` header is a caller-facing parameter, and the model decides that, not taste

The story asks which of operator configuration or caller argument is right, and the answer is
forced rather than chosen: `parse_binding` (`crates/connector-spec/src/config.rs:239-267`) admits
exactly five destinations — `endpoint.*`, `credential.*`, `username.*`, `oauth.client_id`,
`oauth.client_secret` — and **there is no `header.*` binding**. An operator-configured `From` is
unspellable, not merely unwise. Nor can it be a `const_headers` entry: that is emitted as a literal,
and this value identifies a person. What remains is `params.header`, which is also what it *is* — the
acting user is a fact about the call, not about the connection. Same shape as
`providers/stripe.toml:411`'s `Idempotency-Key`. Declared on the two writes only; a read does not
need it, and one asking for somebody's address to fetch a list would be collecting a personal
identifier it cannot use.

C-144's form-encoding gap does not apply here — PagerDuty's writes are JSON — so unlike C-109 this
connector is not reads-only.

### No pagination quirk is declared, and that absence is pinned by a test

PagerDuty pages with `limit`/`offset`. `Pagination` (`crates/connector-spec/src/ir.rs:355-378`) has
`Page` and `Cursor` and nothing else, and `limit`/`offset` is neither: `Page` describes a page
*number* the next request increments by one, while `offset` is a row count the next request advances
by `limit`, so `page_param = "offset"` would build, read correctly to a reviewer, and record
something false about the vendor. `Cursor` needs a next-cursor pointer PagerDuty does not send — its
responses carry `more`, a flag.

**This is a claim about a declaration, not about emitted code, and the first draft of this note got
that wrong.** Nothing emits a pagination loop today: `ir.rs:352` says compiling the enum into Flux
control flow is C-12's work, and `connector-flux` reads only `quirks.error_envelope`. The absence is
still worth pinning — a false declaration left in the file becomes a wrong loop the moment C-12
lands, and `max_pages` is mandatory on every variant (`ir.rs:350-353`) so that loop would be bounded
and silently wrong rather than obviously hung, which is the harder failure to spot.

So the query parameters ship and the quirk does not, following `providers/launchdarkly.toml:123`, and
`no_pagerduty_operation_declares_a_pagination_quirk` exists so the absence cannot later be
"completed" into a declaration the IR cannot honestly make. **A `limit`/`offset` variant is a real
gap in `Pagination`** and is worth its own story; it is not a change a provider story makes in
passing.

### Acknowledge and resolve are two operations over one vendor endpoint — deliberately

PagerDuty exposes one `PUT /incidents/{id}` taking `incident.status`. Declaring it once with a
`status` enum would need one `risk` covering both outcomes, and the only honest value would be the
higher. That is coarser than the facts: acknowledging stops the escalation clock and says a human is
on it (`risk = "medium"`), while resolving closes the incident so the next alert opens a *new* one
with no history of this (`risk = "high"`). Both bodies are pinned with JSON Schema `const`, so the
status stays out of the op signature entirely — the operation *is* the status, which also removes the
failure where a model acknowledges when it meant to resolve by passing the wrong enum word.

### One shared test needed a fix, and it is the only file touched outside this story's own

`crates/connector-spec/tests/services.rs::every_shipped_service_is_spellable_and_a_single_service_provider_declares_none`
asserted that a single-service provider encodes no service field by scanning the canonical JSON for
the **substring** `"service"`/`"services"`. PagerDuty is the first shipped vendor whose own domain
noun is *service*: `GET /services` answers `{"services": [...]}` and every incident carries a
`service` reference, both inside `response_schema`. The scan was reporting a collision of words, not
the property it guards.

Renaming those response fields was not an option — it would make the declared response shape wrong
about what PagerDuty sends. The scan was replaced with a recursive walk over the same canonical
encoding that finds a `service`/`services` key at any depth **except inside a JSON Schema subtree**
(`schema`, `response_schema`, `body_schema`), which is vendor data by definition. That covers all six
IR positions carrying such a field (`Connector::services`, and the `service` on an operation, a config
field, an event, a channel and a graph) and needs no hard-coded list, so a future IR struct is checked
automatically. `the_service_key_walk_still_finds_an_ir_service_key_at_every_depth` was added to prove
the guard still bites.

Two caveats belong on the record rather than in a reviewer's head, both raised in review:

- **The walk tolerates two things, not one.** Besides the schema case, it tolerates
  `service`/`services` as a string **value** anywhere — it tests keys only, so a query parameter
  *named* `service` on a default-only connector now passes where the scan flagged it. That widens the
  text matched, not the property guarded: every IR field spelled `service`/`services` serialises as an
  object key and never as a value.
- **`SCHEMA_KEYS` is not the complete set of vendor-controlled keys.** `InboundEvent::when`
  (`inbound.rs:262`), `Condition::right` (`graph.rs:171`), `NodeKind::Literal::value` (`graph.rs:220`)
  and `NodeKind::Object::fields` (`graph.rs:215`) all carry vendor- or author-chosen names and are
  descended into, so a future webhook narrowed by `when = { service = ... }` would trip this test. The
  substring scan tripped on those too, so nothing was widened; it is left alone because every key added
  to that list is a place the guard stops looking.

### Left out, and why — none of it guessed at

- **`since`/`until`** on `/incidents` and `/oncalls`: ISO 8601 instants, whose `:` and `+` are exactly
  what this emitter's query assembly does not percent-encode (C-29).
- **Every `[]` array filter** — `statuses[]`, `service_ids[]`, `team_ids[]`, `urgencies[]`,
  `user_ids[]`, `include[]`: the brackets live in the parameter *name*, which nothing encodes.
- **`sort_by`** (`created_at:desc`, the `:` again) and **`query`** on `/services` (free text — the
  open `zendesk-ticket-search` defect).
- **Creating, merging, snoozing or reassigning an incident**, and everything under alerts, log
  entries, escalation policies, schedules, users and webhook subscriptions. **Unverified, therefore
  absent.**
- **A `[[channels]]` binding and any `[[events]]`.** PagerDuty v3 webhook subscriptions sign with
  `X-PagerDuty-Signature`; that scheme has not been verified against `HmacSpec` here, so no signing
  credential and no events are declared rather than describing verification this story did not check.
  That is a separate story's work.

### No personal data

PagerDuty's on-call and incident payloads are full of it. No email address appears in
`providers/pagerduty.toml` — the contract test asserts the file contains no `@` at all — no field
carries an `example`, and the `user` reference is described by shape rather than exemplified.

### Gate

Scoped build and diff clean (`9 artifacts up to date (1 provider checked)`); workspace build, clippy
and fmt green. `cargo test --workspace --no-fail-fast` leaves **exactly the eight** whole-catalogue
staleness reds `AGENTS.md` tabulates, across five binaries, and nothing else — the ninth,
`the_recorded_floor_is_the_measured_figure`, is green, because all six operations carry response
shapes and this story alone fits inside the `COVERED_FLOOR` slack. `COVERED_FLOOR` was not touched;
it is the coordinator's to raise at integration if the wave's accumulation crosses it.
