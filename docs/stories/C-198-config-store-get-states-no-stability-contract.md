---
id: C-198
title: "`ConfigStore::get` states no stability contract, so a mutable store can gate one host and call another"
pillar: Bridge
status: in-progress
priority: 2
design: docs/designs/connector-configuration.md
epic: tool-pack
areas: [bridge, connector-pack]
note: "the independent review of C-193. `permission_subjects` and `execute` each performed an independent `get`, and the pack bypasses `Executor::dispatch` — so the gate and the request could be shown different hosts. Plus two doc corrections the same review found: `Field::Endpoint` claims a per-service distinction the key does not make (C-197), and `test_configuration()` claimed completeness while omitting okta and statuspage"
---

# `ConfigStore::get` states no stability contract, so a mutable store can gate one host and call another

## Goal

Close the time-of-check/time-of-use hole the C-193 review found in the connection-configuration
port: make the pack read a tenant's configuration **once**, so a `ConfigStore` with interior
mutability cannot show flux's egress allow-list one host and then send the request to another. Two
doc corrections from the same review ride along, because both are cases of prose claiming something
the code does not do.

## 1. The contract gap — and why it is a gate bypass rather than a nitpick

`ConfigStore::get` (`crates/connector-pack/src/config.rs:106`) documented only *"the value bound to
`field` of `provider`, for `tenant`"*. Nothing said the answer had to be **stable**.

The pack calls `http.request`'s `execute` directly, bypassing `Executor::dispatch`
(`lib.rs:76-83`), so `Operation::permission_subjects` is the **only** place flux's egress
allow-list is consulted for the inner call. Before this story, `permission_subjects` and `execute`
each performed an *independent* `get`:

| path | reads | through |
|---|---|---|
| `permission_subjects` → `subjects` → `build_request` → `endpoints` | `Field::Endpoint(v)` | `Configuration::require` |
| `permission_subjects` fallback → `substituted_host` | `Field::Endpoint(v)` | `Configuration::lookup` |
| `execute` → `build_authenticated_request` → `build_request` → `endpoints` | `Field::Endpoint(v)` | `Configuration::require` |
| `execute` → `resolve` → `user_half` | `Field::Username(c)` | `Configuration::require` |

A store backed by a database, a cache with a TTL, or anything else that can answer differently on
two calls therefore gets the gate approved against `gate.example.com` and the request sent to
`elsewhere.example.com` — straight through the egress allow-list, with the audit record naming the
host that was never called.

The module prose already said a host *"resolves eagerly … and binds a snapshot"* (`config.rs:26-29`),
but that is advisory prose in a module doc. The requirement belongs on the trait method an
implementor actually reads.

**The decision this story records: enforce it, do not merely document it.** Enforcement is cheap —
the set of fields an operation can ask for is known at `Operation::project`, so the pack resolves
them once there and holds the snapshot. A documented invariant a caller can break silently is
weaker than one the type prevents, and here the type can prevent it without changing a single
public signature.

## 2. `Field::Endpoint`'s doc contradicted its own key

`config.rs:63` said the field is *"a `{var}` in a **service's** `base_url`"*, while the key is
`(tenant, provider, kind, name)` — **no service in it**. That sentence is what made a real defect
invisible: `contentful` declares `delivery_space_id` and `management_space_id`, two distinct
configuration fields both binding `endpoint.space_id` under different services
(`providers/contentful.toml:164-207`), and this port can hold only one of them. A tenant whose
delivery and management environments differ reads the wrong one and gets a `200`, not a refusal.

The fix is [C-197](C-197-config-collapses-across-services.md) and is **not attempted here**: it
requires adding `service` to `catalog::Operation`, which moves every generated artifact and breaks a
published type. This story only stops the doc claiming a distinction the code does not make.

## 3. `test_configuration()` overstated itself

`lib.rs:716` claimed the helper carries *"every templated connector's"* endpoint variables, and the
hand-written list at `:724-737` omitted okta's `domain` (`providers/okta.toml:76`) and statuspage's
`page_id` (`providers/statuspage.toml:150`) — both shipped after the list was written. A
hand-written list whose doc claims completeness is exactly what goes stale, and two integration test
files already discover the same set from the catalogue instead.

## 4. The Basic user half has no end-to-end test

`crates/connector-pack/tests/` drives only `slack-chat-post-message` (bearer) end to end; `auth.rs`'s
unit test supplies the user directly. C-193 moved the Basic user half from the process environment to
the configuration port, so it is a **new source of an authentication input with nothing asserting it
end to end**. Coverage did not regress — the base had none either — but this is the cheapest moment
to add it.

## Acceptance

- [x] **`ConfigStore::get` states the stability requirement**, on the trait method rather than in
      module prose: the same field must answer with the same value for the lifetime of the bound
      store, with the consequence named (gate and request are two reads, and the pack is the only
      egress gate for the inner call) so that someone writing a database-backed store knows what
      they are promising. → `crates/connector-pack/src/config.rs:174-197`
- [x] **It is enforced, not only documented.** Every configuration value an operation can ask for —
      its endpoint variables and the Basic user half of each credential its connector declares — is
      resolved **once**, at `Operation::project`, and every later read is of that snapshot. A
      mutable store is consulted once per field per operation and therefore cannot be consulted
      twice. → `config.rs::Configuration::snapshot` + `config.rs::Snapshot`, taken at
      `tool.rs:173-183`; `Operation` now holds a `Snapshot` in place of the `Configuration`
      (`tool.rs:98-105`), so it has no handle to the store at all
- [x] **Failing-first test:**
      `crates/connector-pack/tests/endpoint_configuration.rs::a_store_that_answers_differently_cannot_gate_one_host_and_call_another`.
      It binds a `ConfigStore` whose `get` returns a different subdomain on each call, and asserts
      that the permission subject and the request URL name the same host, and that the store was
      consulted exactly once for the field. → at `3127e00` it fails with
      `left: ["https://host-0.zendesk.com/api/v2/tickets/1.json"]` /
      `right: ["https://host-1.zendesk.com/api/v2/tickets/1.json"]` — the gate and the request,
      two hosts
- [x] No public signature moves. `Configuration`, `ConfigStore`, `Field`, `MemoryConfig`,
      `Operation::project` and `pack` keep their shapes, because the snapshot is taken inside the
      pack rather than asked of a host. `Snapshot` is `pub(crate)`; the only removals are
      `Configuration`'s own `pub(crate)` `require`/`lookup`, which moved onto it.
- [x] **`Field::Endpoint`'s doc states what the key does**: keyed by connector, not by service; two
      services of one connector spelling the same variable collapse to one value; `contentful` is
      the shipped case; and it points at C-197 as the fix. No code change. → `config.rs:79-98`
- [x] **`test_configuration()` is derived from the catalogue**, not hand-listed, so its doc claim of
      completeness is true by construction. → `lib.rs::test_configuration`, with
      `lib.rs::the_test_configuration_covers_every_templated_connector` as the guard — it projects
      every shipped operation and fails on any `MissingConfig`, and separately pins that okta still
      declares `domain` and statuspage still declares `page_id`, so the guard cannot go quietly
      vacuous the way the list it replaced did
- [~] **The Basic user half is asserted end to end** on `zendesk-ticket-show`. Both assertions
      landed, but **not from `tests/`**, and the reason is a catalogue fact rather than a choice —
      see Progress. → `src/credentials.rs::a_basic_user_half_reaches_the_header_from_the_configuration_port`
      and `::a_basic_credential_with_no_configured_user_is_refused_by_name`, with the public half of
      the wall pinned by
      `tests/credentials.rs::a_basic_connector_refuses_because_it_has_no_credential_address`
- [x] **No artifact moves.** `cargo run -p connector-cli -- diff` reports
      `479 artifacts up to date (44 providers checked)`, and `git status --short` names only
      `crates/connector-pack/**` and this story.

## Progress

Done on `impl/C-198`. Six files: four in `crates/connector-pack/src`, two in its `tests/`, plus this
story. No artifact moved and no public signature moved.

**The enforcement, and why it was cheap.** The set of fields an operation can ask for is knowable at
projection time — its endpoint variables come from its own emitted Flux
(`request::endpoint_variables`) and its Basic user halves from its connector's declared credentials,
both `&'static` catalogue data. So `Operation::project` calls `Configuration::snapshot` once and the
operation stores the resulting `Snapshot` **instead of** the `Configuration`. That substitution is
the whole property: an operation holding a port can read it twice, and one holding a snapshot cannot
read it at all. `Credentials::resolve`, `resolve_mechanism` and `user_half` take `&Snapshot` for the
same reason.

Only the *timing of the read* changed, not the timing of the refusal: a tenant that has configured
nothing still gets `Error::MissingConfig` at the first call, naming the field, exactly as C-193
shipped it. The requirement on `ConfigStore::get` is documented anyway, because one bound store
serves many operations and each projection is a fresh read — a store that drifts between two
projections gives two connectors two views of one tenant, which enforcement here cannot reach.

**`Field::Username` is snapshotted too**, though only `Field::Endpoint` can move a host. The contract
is stated over `get`, not over one variant, and a half-enforced invariant is the kind that gets
quoted as if it were total.

**The one deviation: the Basic end-to-end tests are in `src/`, not `tests/`.** The dispatch asked for
them under `crates/connector-pack/tests/`, and that is not reachable. **All three connectors
declaring a `BasicJoin` credential — `zendesk`, `jira` and `twilio` — declare `authority: None`**
(`crates/catalog/src/generated/{zendesk,jira,twilio}.rs:12`), so `Credentials::reference` refuses
with `Error::NoCredentialAddress` *before* the configuration port is consulted, and no shipped
connector can reach the Basic assembly through the public `Operation::build_authenticated_request` —
neither the positive case nor the `MissingCredentialConfig` one. That is C-92's gap, which `AGENTS.md`
records as intentional, and closing it means editing `providers/*.toml` and regenerating artifacts.

So the split is:

- `tests/credentials.rs::a_basic_connector_refuses_because_it_has_no_credential_address` pins the
  **public** behaviour that does exist: everything else supplied — subdomain configured, token in
  the store — and the refusal names the one missing fact. It is the fail-closed property `AGENTS.md`
  states, and it had no test.
- `src/credentials.rs` holds the two assertions the dispatch named, reached by `Box::leak`ing a copy
  of the shipped zendesk `catalog::Provider` with an authority filled in — the same trick
  `tool.rs::an_operation_with_no_declared_host_is_refused` already uses. Nothing else is faked: the
  credential, its `BasicJoin`, its `/token` suffix and its `Basic ` header placement are the
  catalogue's own, the URL is `Operation::build_request`'s, and the expected
  `Basic b3BzQGFjbWUudGVzdC90b2tlbjpTRU5USU5FTC1OT1QtQS1SRUFMLVNFQ1JFVC1DMTk4` is a literal computed
  by `base64(1)` rather than by this crate's encoder, so the assertion cannot agree with a wrong one.

**The day C-92 gives zendesk an authority**, the `tests/` refusal test fails and is the reminder to
move the two `src/` tests into it. That is deliberate: an inverted test that announces itself is
better than a gap nobody rediscovers.

**Base proof.** At `3127e00` (`git merge-base main HEAD`), with the new test present and the fix
absent:

```
$ cargo test -p connector-pack --test endpoint_configuration a_store_that_answers_differently
test a_store_that_answers_differently_cannot_gate_one_host_and_call_another ... FAILED
assertion `left == right` failed: the gate was shown a host the request did not reach:
`https://host-1.zendesk.com/api/v2/tickets/1.json` went out
  left: ["https://host-0.zendesk.com/api/v2/tickets/1.json"]
 right: ["https://host-1.zendesk.com/api/v2/tickets/1.json"]
```

`host-0` is the gate's read and `host-1` is the request's — the two independent `get`s, in one call.

**Gate.** `cargo build --workspace`, `cargo test --workspace --no-fail-fast` (no `FAILED`, no
`panicked at`), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`
and `cargo run -p connector-cli -- diff` (`479 artifacts up to date (44 providers checked)`) are all
green in this worktree. `cargo doc -p connector-pack --no-deps` still emits its four pre-existing
warnings and no fifth.

## Notes

- The enforcement point is `Operation::project` rather than `Configuration::new`, because only the
  operation knows which fields it can ask for: the endpoint variables come from its own emitted Flux
  (`request::endpoint_variables`) and the usernames from its connector's declared credentials.
- The snapshot deliberately does **not** turn a missing value into an install-time failure. A tenant
  that has configured nothing still gets `Error::MissingConfig` at the first call, naming the field —
  the same diagnostic C-193 shipped. Only the *timing of the read* changes, not the timing of the
  refusal.
- `Field::Username` is snapshotted too, even though only `Field::Endpoint` can move a host. The
  contract is stated over `get`, not over one variant of `Field`, and a partially-enforced invariant
  is the kind that gets quoted as if it were total.
- `push_user_half_fields` takes the *provider's* whole Basic credential set rather than the
  operation's. Filtering to `Operation::credentials` would duplicate `credentials.rs`'s
  alternative-selection rule at a second site, and the failure mode of a filter that drifts is a
  field the snapshot does not carry — a refusal on the authentication path, for a credential that is
  configured. An unused entry costs one `get`.

## Notes

- The enforcement point is `Operation::project` rather than `Configuration::new`, because only the
  operation knows which fields it can ask for: the endpoint variables come from its own emitted Flux
  (`request::endpoint_variables`) and the usernames from its connector's declared credentials.
- The snapshot deliberately does **not** turn a missing value into an install-time failure. A tenant
  that has configured nothing still gets `Error::MissingConfig` at the first call, naming the field —
  the same diagnostic C-193 shipped. Only the *timing of the read* changes, not the timing of the
  refusal.
- `Field::Username` is snapshotted too, even though only `Field::Endpoint` can move a host. The
  contract is stated over `get`, not over one variant of `Field`, and a partially-enforced invariant
  is the kind that gets quoted as if it were total.
