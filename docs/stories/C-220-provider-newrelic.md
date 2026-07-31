---
id: C-220
title: Ship the New Relic connector
pillar: Spec
status: done
priority: 4
design:
epic: provider-fleet-2
areas: [providers]
note: "PROBE: US and EU accounts live on DIFFERENT hosts (`api.newrelic.com` vs `api.eu.newrelic.com`) and the key does not say which. A wrong choice fails as an auth error, not a routing error"
---

# New Relic — a region-selected host chosen by the operator, not the vendor

## Goal

Ship a curated `newrelic` connector, and use it to answer the question in *Why it earns its place*.

## Why it earns its place

[C-105](C-105-provider-fleet-2-epic.md) requires it: *"Each one earns its place by exercising
something the model has not met. A connector that only adds a row is a row."*

Nine shipped connectors template a host, but every one of them templates a *label the operator
owns* (`{subdomain}.zendesk.com`). New Relic is different: the operator picks between two
**vendor-owned** hosts, and the credential does not disclose which one it belongs to. Choosing wrong
returns a 401 that looks exactly like a bad key.

That makes it the case for a config field whose value comes from a **closed set** rather than free
text. Nothing in the IR expresses "one of these two", and shipping this connector will either
produce that expression or record honestly that an operator can type anything into a field where
only two values work.

## Acceptance

- [x] `providers/newrelic.toml`, hand-authored and **curated** — a small set of operations worth
      exposing, not every endpoint the vendor documents. Endpoints deliberately excluded are named,
      not silently absent.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written
      as a contract a model reads.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with
      `binds`. **No realistic-looking `example` on a secret field** — a token-shaped placeholder has
      tripped GitHub push protection and blocked a release here before.
- [x] A `verify` operation that is an argument-free read and runs unattended.
- [x] `crates/connector-flux/tests/newrelic_connector.rs` — a per-provider contract test asserting the
      probe below, not merely that the TOML parses.
- [x] **Failing-first test:** the contract test must fail before `providers/newrelic.toml` exists.
- [x] The scoped gate is green: `build --provider newrelic`, `diff --provider newrelic` reporting no
      drift, and the emitted Flux parsing, analyzing and being a fixed point of flux's own formatter.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness
      checks `AGENTS.md` tabulates. Do not run a full build; the coordinator resolves them at
      integration.

## Notes

- The enum question is the story's finding, not a side note. If `[[config]]` cannot express a closed
  set, **file it** rather than working around it with `help` text — a free-text field with two valid
  values is the same class of defect as the `no-credential` conflation C-206 just fixed.
- Prefer the REST v2 API over NerdGraph for the first pass. NerdGraph is GraphQL and belongs with
  [C-110](C-110-provider-linear.md)'s decision, not ahead of it.
- Do not model the query-language endpoints (NRQL) as operations. A free-form query string is not a
  curated operation and would make the connector's surface unbounded.
- Follow the shape the shipped fleet established. `providers/trello.toml` is the most recent example
  and is the one to read first; it also records what it deliberately left out, which is the habit to
  copy.
- **Vendor API shapes are hand-authored and drift is undetectable by machine** ([C-14](C-14-fetch-and-drift-check.md)).
  Cite the vendor documentation you worked from in the provider header, with the date.

## Progress

**Shipped.** `providers/newrelic.toml` — 6 operations (5 reads, 1 write), one `X-Api-Key` credential,
two `[[config]]` fields, `verify = "newrelic-application-list"`. Contract test:
`crates/connector-flux/tests/newrelic_connector.rs`, 5 tests. Scoped gate green,
`diff --provider newrelic` reports `9 artifacts up to date (1 provider checked)`, and exactly the
eight whole-catalogue staleness tests `AGENTS.md` tabulates are red — reported, not silenced.

### The finding: `[[config]]` cannot express a closed set of values, and this is the first vendor that needs it

**The question is answered, and the answer is no.** The connector ships with
`base_url = "https://{host}/v2"` and a `host` field bound `endpoint.host` with `format = "hostname"`.
Exactly two values work — `api.newrelic.com` (US) and `api.eu.newrelic.com` (EU) — and nothing in the
IR can say so:

- `Format` (`crates/connector-spec/src/config.rs:104-123`) is a closed enum of value **shapes**, not
  of values. Every variant answers "what does a well-formed value look like"; none answers "which
  values are permitted". `hostname` is the nearest fit and admits every syntactically valid host.
- `ConfigField` (`config.rs:501-551`) has no other field for it and is `#[serde(deny_unknown_fields)]`,
  so the `values = [...]` key an author reaches for is a load error rather than a key quietly ignored.
- `Binding::Request`/`Position` (C-187) do not help: they constrain a pinned value's *characters* so
  it cannot reshape a request. The hazard here is the opposite — a perfectly well-formed request sent
  to a real server that does not hold this account.

Both halves are measured rather than described, in
`newrelic_connector.rs::the_closed_set_of_two_hosts_is_not_expressible_and_the_field_admits_any_host`:
the shipped file still loads when its `example` names a host with no relationship to New Relic, and
the closed-set declaration is refused as an unknown field.

**Why it matters more here than for the nine templated hosts already shipped.** Those template a
label the *operator* owns (`{subdomain}.zendesk.com`), where the answer set is genuinely unbounded.
The nearest precedent is `providers/docusign.toml`'s `account_host`, also a vendor-owned host bound
through `endpoint.` — but DocuSign's value is *discoverable* (a field of the OAuth UserInfo
response), so a wrong answer is a transcription error. New Relic's is a choice from two made before
any call succeeds, and **a wrong choice returns `401` on every call, indistinguishable from a bad
key**. An operator debugging it rotates the key first and fixes nothing. The only mitigation shipped
is prose in `help`, and prose is not a constraint. `providers/intercom.toml` has recorded the
identical vendor shape as an unclosed "SCHEMA GAP: the regional hosts are not selectable" since it
shipped, and ships US-only; this is the first connector to actually bind such a host.

**Filed as a follow-up story** (not opened by this implementor — the board is coordinator-owned): a
closed value set on `ConfigField` — an enumerated list of permitted values, each with its own
`label`, so a renderer shows a two-item dropdown rather than a text box and the loader checks
`example` against membership rather than against shape. It should also decide whether such a field
still needs a `format`, and what a host does with a stored value that is no longer in the set after a
vendor adds a region.

### Curation, and what was deliberately left out

Named in full in the provider header. In summary: **NRQL and NerdGraph excluded** (a free-form query
language is an unbounded surface; GraphQL waits on C-110); **no query parameter anywhere**, so
`filter[name]`, `only_open`, `start_date`/`end_date` and `page` are all absent while the emitter
still interpolates query values verbatim (`op.rs:138-143`); `DELETE`/`PUT /applications/{id}.json`,
the metrics endpoints, `users.json`, `key_transactions.json` and the alert-policy writes excluded
with a reason each; `[[events]]`/`[[channels]]` absent because New Relic's webhook notifications
carry no HMAC this file can name, so the binding would have to state `verification = "none"` — a real
answer that deserves its own story.

The `only_open` exclusion has a caller-visible consequence and is stated in the operation's own
description in the imperative: `newrelic-alert-violation-list` returns closed violations alongside
open ones, and `closed_at` being null is the only thing that means still open.
