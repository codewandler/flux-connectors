---
id: C-109
title: Ship the Twilio connector
pillar: Spec
status: done
design:
epic: provider-fleet-2
areas: [providers]
note: "third basic-join vendor, and the one whose username half is an account identifier rather than an email — so it tests whether the config model generalises past the zendesk/jira shape"
---

# Ship the Twilio connector

## Goal
Messaging, and a third variation on the basic-auth archetype.

## Acceptance
- [x] A curated operation set: ~~send a message,~~ list messages, fetch a message, list calls (plus
      `twilio-account-get`, the `verify` read).
      → **Sending is deliberately excluded**, not curated away: every Twilio write is
      `application/x-www-form-urlencoded`, and `params.body_encoding = "form"` (C-144) interpolates
      form values verbatim because flux has no form encoder in the pinned `codewandler-flux-lang`
      release. The encoder exists upstream as flux's own `L-101`, but it reaches this repository only
      when that release publishes it. `providers/twilio.toml`'s header comment and `AGENTS.md`'s
      "Intentional gaps" precedent (`params.body_encoding = "form"`'s own entry) record the same
      constraint. Four reads ship: `twilio-account-get`, `twilio-message-list`,
      `twilio-message-get`, `twilio-call-list`, `twilio-call-get`.
- [x] **Auth is basic-join with an account SID as the username half.**
      → `[[auth]] twilio.basic_auth` (`scheme = "basic"`, `user_env = ["TWILIO_ACCOUNT_SID"]`,
      `env = ["TWILIO_AUTH_TOKEN"]`) and `[[config]] name = "account_sid"` binding
      `username.twilio.basic_auth`, with `label`/`help` addressed to a value copied from the Twilio
      Console rather than an already-known email.
      Asserted by `the_twilio_connector_loads_and_authenticates_with_an_account_sid_and_auth_token`
      in `crates/connector-flux/tests/twilio_connector.rs`.
- [x] The account SID also appears **in the path** of most endpoints, recorded without asking a user
      for it twice.
      → Every operation declares a required `account_sid` path parameter
      (`every_twilio_operation_requires_the_account_sid_in_its_path`). It is **not** also templated
      into `base_url` (`endpoint.account_sid`), because `ConfigField::binds` accepts exactly one
      destination per field and the same field is already spoken for by
      `username.twilio.basic_auth` — see the long comment above `[[operations]]` in
      `providers/twilio.toml` for the full reasoning, including why `base_url` absorption would fail
      independently on the account resource's own path shape. The config surface stays one visible
      field; the duplication is call-time (a real, declared path parameter), not config-time.
- [ ] `[[events]]` and a `webhook` binding for status callbacks, with Twilio's published signature
      scheme — a fourth row for C-60's matrix.
      → **`[[events]]` shipped** (`message.status_callback`, `call.status_callback`). **No
      `[[channels]]` binding.** Measured, not hand-waved: Twilio's `X-Twilio-Signature` signs the
      request URL concatenated with its POST parameters *parsed, sorted by name, and rejoined with no
      delimiter* — not a template over `HmacSpec::signed`'s only two placeholders, `{body}` (the raw,
      pre-parse bytes) and `{timestamp}`. No `{url}` placeholder exists, and even one would not help:
      a fixed template cannot re-sort a variable-length, variable-named field list, which is exactly
      the connector-specific-expression shape principle 2 refuses. This is the same finding
      `providers/stripe.toml` records for `Stripe-Signature`, and it is a gap in `HmacSpec` itself
      (parallel to Stripe's C-141), not something this file can work around. The signing credential
      (`twilio.webhook_signing_secret`) is declared and ready for when a verification model can
      express it. `twilio_declares_no_channel_binding_for_its_events` pins this.
- [x] A `[[config]]` surface, a `verify` operation, and a per-provider contract test.
      → Two `[[config]]` fields (`account_sid`, `auth_token`); `verify = "twilio-account-get"`;
      `crates/connector-flux/tests/twilio_connector.rs` (8 tests).

## Progress
- Shipped as a **reads-only** connector: `twilio-account-get` (verify), `twilio-message-list`,
  `twilio-message-get`, `twilio-call-list`, `twilio-call-get`. Basic auth with the Account SID as
  username and the Auth Token as the gated secret. `[[events]]` declared for both status-callback
  shapes; no `[[channels]]` binding, for the reason recorded above and in the provider file.
- Two findings recorded rather than hand-waved, both anticipated by this story's Notes:
  1. The Account SID cannot bind both `endpoint.account_sid` (for `base_url`) and
     `username.twilio.basic_auth` from one `[[config]]` field — `ConfigField::binds` is
     single-destination. Resolved by binding the one visible field to the username (the load-bearing
     acceptance item) and declaring the SID as a real path parameter on every operation instead of a
     `base_url` template.
  2. Twilio's inbound HMAC scheme (URL + sorted, reassembled form fields) cannot be expressed by
     `HmacSpec::signed`, which templates only over `{body}`/`{timestamp}` raw bytes. This is a gap in
     `HmacSpec` itself, not a per-provider workaround waiting to be found.
- Gate run in this worktree: `build --provider twilio` (8 artifacts written), `diff --provider
  twilio` (no drift), `cargo build --workspace` (clean), `cargo test --workspace --no-fail-fast`
  (exactly the 8 red tests `AGENTS.md` predicts for a new provider, across the same 5 binaries; the
  ninth, `the_recorded_floor_is_the_measured_figure`, stayed green), `cargo clippy --workspace
  --all-targets -- -D warnings` (clean), `cargo fmt --all --check` (clean after `cargo fmt --all`).

## Notes
- One value serving as both a credential component and a path parameter is genuinely new. If the
  model cannot express it without duplication, that is a finding for the configuration design, not a
  reason to hand-wave the connector.

### Coordinator note at integration

`PARTIAL` and correctly so. The read surface ships; the **webhook binding does not**, because
`HmacSpec::signed` cannot express Twilio's scheme. Filed as [C-188](C-188-hmac-cannot-sign-a-url.md).

The connector declares its `[[events]]` and **no** `[[channels]]` binding, with a test asserting that
absence. That is the right shape: the member contract's *"silence is never a verification answer"* rule
governs a binding that exists, and the honest move when the scheme is inexpressible is to omit the binding
rather than to declare one with a verification it cannot perform.

Two notes on judgement calls I checked rather than took on trust:

- **`twilio.webhook_signing_secret` deliberately reuses `TWILIO_AUTH_TOKEN`**, the same variable as the
  Basic credential's secret. That looks like a copy-paste error and is not: Twilio issues exactly one
  secret serving two roles. The implementor confirmed the loader has no rule against env reuse across
  credentials and documented the reuse as intentional.
- **It caught its own stale worktree base** — several commits behind `main` — confirmed only untracked
  files were present, and re-rooted before committing anything. Handled and reported rather than silently
  built upon, which is the coordination hazard that cost this run a wave of rework at the start.

The send surface stays excluded: form values interpolate verbatim until flux publishes `L-101`, so a value
carrying `&` or `=` would corrupt the body and could inject a field.
