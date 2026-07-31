---
id: C-234
title: "The app cannot be used at all without a real Google registration, so trying it locally requires credentials the operator may not have"
pillar: Bridge
status: in-progress
priority: 1
design: docs/designs/connectors-api.md
epic: connectors-api
areas: [bridge, host]
note: "asked for by the owner 2026-07-31: every /v1 route refuses without a session, and the only way to get a session is a real Google OAuth registration. A developer with no Google client id cannot reach the app at all"
---

# A dev sign-in that needs no Google registration

## Goal

Let someone run `cargo run -p connectors-api` and actually use the app — wire a connector, store a
credential, rehearse an operation — without registering an OAuth client with Google first.

## Why it is needed

[C-204](C-204-google-signin-accounts.md) did exactly what it should: every `/v1` route refuses
without a session, and the tenant is resolved from the session rather than from anything a request
names. The consequence is that the **only** door into the app is a real Google registration. Without
a client id and secret the host starts, prints its setup message, serves its page, and every route
answers:

```
401 {"error":"sign in first: this host resolves the tenant from the session, and there is none"}
```

That is correct behaviour and an unusable first run.

## The shape

A `--dev` flag on the binary that turns on a **second, explicitly-fake identity provider**: a
`Sign in as developer` button on the index page that mints an ordinary session for an obviously
non-real account.

**It must mint a session through the same machinery a Google sign-in uses.** A dev mode that
special-cases the session type, or that bypasses the tenant resolution, would make every other route
behave differently under test than in production — which is the failure mode that makes dev modes
worth less than they cost.

## What makes this safe enough to exist

The host is already **loopback-only and deliberately not configurable** — `main.rs` says the first PR
adding a `--bind` flag is the one to refuse. That is the property this rests on, and it should be
stated in the code rather than assumed.

Beyond it:

- **Opt-in per process, never by default.** A flag the operator types, not an environment variable a
  deployment could inherit. `--dev` present, or the door does not exist.
- **The route must not exist when the flag is absent** — not "exists and returns 403". An absent
  route cannot be reached by a misconfiguration.
- **The identity must be unmistakably fake** where an operator sees it, so nobody ever wonders which
  account they are in. Not `dev@example.com` styled to look real.
- **Loud at startup.** The banner already warns that this host makes real calls with real
  credentials; dev mode has to say, in the same place, that authentication is off.

## Acceptance

- [x] **Failing-first test:** with `--dev` off, the dev sign-in route returns 404 — not 403, not 401
      — and no session can be obtained without Google. It must fail before the change. Name it.
      → `tests/dev_signin.rs::the_dev_sign_in_route_exists_only_under_the_dev_flag`. It asserts both
      halves in one test, because the `404` half alone passes on every host that ever existed and a
      test that cannot fail is not evidence.
- [x] With `--dev` on, a `POST` to the dev sign-in mints a session that is **byte-identical in kind**
      to a Google one: same cookie attributes, same opacity, same tenant resolution. Assert this
      against the existing session guarantees rather than writing new ones.
      → `auth/routes.rs::dev_signin` calls `Accounts::of_subject` → `Sessions::create` →
      `session_cookie`, the same three calls in the same order as `callback`'s tail;
      `tests/dev_signin.rs::a_dev_session_carries_the_same_cookie_guarantees_as_a_google_one`
      repeats `tenancy.rs::the_session_cookie_is_opaque_and_locked_down`'s assertions verbatim.
- [x] The tenant is derived the same way a Google tenant is, so a dev session and a real session
      cannot collide, and switching between them does not leak credentials across tenants. State
      what the dev tenant id is and why it cannot collide with a real one.
      → **`dev-local`.** `Account::from_claims` prepends the literal `google-` to every tenant it can
      make; `Account::developer` produces `dev-local` and takes no arguments. Those are `Account`'s
      only two constructors and its fields are private, so the namespaces are disjoint structurally
      rather than probabilistically. Proved by `a_dev_session_and_a_google_session_share_no_credential`
      (both doors on one process) and the unit test `a_dev_tenant_cannot_be_reached_from_any_id_token`.
- [x] `crates/connectors-api/src/index.html` shows the button **only** in dev mode, labelled so it
      cannot be mistaken for the real sign-in.
      → Drawn behind `status.dev`, which is the same value the router built its table from, so the
      page cannot offer a button that 404s. Labelled
      *"Sign in as DEVELOPER — fake account, no Google, dev mode only"*, styled as a secondary
      action, with the header note replaced by *"DEV MODE — authentication is disabled."*
- [x] The startup banner states that authentication is disabled when `--dev` is on.
      → `main.rs`, in the same block as the real-credentials warning.
- [x] The existing guarantee in `crates/connectors-api/tests/host.rs` is re-proved under `--dev`: no
      credential value reaches any served surface, including on error. Dev mode must not become a
      hole in the one invariant this host exists to keep.
      → `tests/dev_signin.rs::a_credential_stored_in_dev_mode_reaches_no_surface`, sweeping the same
      six surfaces plus `/auth/dev` itself and four error paths.
- [x] Every existing C-204 test still passes unchanged. If one needed changing, that is a finding
      about dev mode, not about the test.
      → `tests/host.rs`, `tests/tenancy.rs`, `tests/id_token.rs` and `tests/support/mod.rs` are
      untouched by this diff. `router(app)` kept its signature so that support needed no edit.
- [x] The gate is green.
      → `cargo fmt --all` · `build --workspace` · `test --workspace --no-fail-fast` (128 test
      binaries, zero failures) · `clippy --workspace --all-targets -- -D warnings` ·
      `fmt --all --check`.

## Notes

- **Do not weaken the login-CSRF binding to make dev mode simpler.** C-204's `connectors_login`
  cookie and its constant-time comparison are load-bearing, and a security re-review reproduced the
  original attack end to end before confirming the fix. Dev mode adds a door; it does not touch that
  one.
- Consider whether `--dev` should also be refused when a Google registration **is** configured, or
  whether both doors may stand open at once. Either is defensible; record the reason. Refusing is
  simpler to reason about, allowing both is friendlier when testing the real flow.
- A cargo feature rather than a runtime flag would make the code physically absent from a release
  binary, which is stronger. It also means `cargo run -p connectors-api -- --dev` does not work out
  of the box, which is the whole point of the story. Weigh it and record the choice.
- [C-207](C-207-the-host-forgets-every-credential.md) still applies: credentials live in memory and
  die with the process. Dev mode does not change that, and the banner should not imply otherwise.

## Progress

Implemented on `impl/C-234`. The door is `POST /auth/dev`, added to the route table in
`lib.rs::router` only when `App::dev_signin()` is true, which only `main.rs` sets and only when it
was handed `--dev`. No environment variable reaches it.

**The two judgement calls the notes asked for are recorded in
[`docs/designs/connectors-api.md`](../designs/connectors-api.md) §"The dev sign-in (C-234)".** In
short:

1. **`--dev` is not refused when Google is also configured.** Both doors may stand open, because the
   tenants they mint are disjoint by construction (`google-{sub}` vs `dev-local`) and refusing would
   make trying both flows on one machine a matter of unsetting environment variables — and would
   turn a stale `CONNECTORS_GOOGLE_CLIENT_ID` in a shell profile into a startup failure, which is
   C-204's rejected first-run stack trace arriving through another door.
2. **A runtime flag, not a cargo feature.** A feature would make the code absent from a release
   build, which is stronger — but this crate is `publish = false` and ships no release artefact, so
   there is nothing for it to be absent from, and it would turn the story's own deliverable into
   `cargo run -p connectors-api --features dev -- --dev`. The design doc records that this should be
   revisited the day this crate acquires a shipped binary or container image.

Two things beyond the acceptance, both deliberate:

- **`main.rs` now refuses unknown arguments** instead of ignoring them, so `--bind 0.0.0.0` is a
  startup error naming the reason rather than a word the binary shrugs at. The dev door is only
  defensible while the bind is loopback-only, so that property is now enforced as well as documented.
- **`POST /auth/dev` refuses `Sec-Fetch-Site: cross-site`.** This route mints a session from a
  request carrying no cookie, so `SameSite=Lax` does nothing for it and a cross-site form POST would
  otherwise put a browser into the dev tenant silently. Defence in depth only — an absent header is
  allowed, which is what keeps `curl` working.

Verified by hand as well as by test: `cargo run -p connectors-api -- --dev`, one `POST /auth/dev`,
then `/v1/connectors` → 200 with 53 connectors, a credential stored at
`tenants/dev-local/com.anthropic.api/api_key`, and `anthropic-models-list` rehearsed — which reached
`api.anthropic.com` and returned its real `401` for the fake key, so the whole path from button to
vendor is live. Without `--dev` the same binary answers `404` on `/auth/dev` with an empty body.

**Not done here:** the board, `CHANGELOG.md` and `docs/roadmap.md` are the coordinator's to write.

### Rework round 1 — closing two unpinned properties

Independent security review returned PASS (16 falsification mutations, 156 hand-written raw HTTP
requests, C-204 verified untouched by blob hash). Thirteen mutations went red; two of the three that
went green are closed here. Neither was a defect in the shipped code — both were properties the code
*had* but that no test would have defended if somebody changed them later.

**M16 — the dev identity is not steerable.** `auth/routes.rs` claims in prose that the dev route "is
not an impersonation primitive: there is no parameter that would let a caller ask to be somebody".
The review made `dev_signin` read `?tenant=` and route it through `Account::from_claims`, minting
`google-<attacker-chosen>` sessions straight out of the dev door — and the suite stayed green. Now
pinned by `tests/dev_signin.rs::the_dev_identity_cannot_be_steered_by_anything_a_caller_sends`, which
mints through four channels (query string, JSON body, form body, headers including
`X-Forwarded-User`/`X-Remote-User`) crossed with nine identity-shaped keys, and asserts the whole
`/auth/me` document is **byte-identical to the unsteered one** every time. The steering values are
deliberately *valid* subjects under `validate_subject`, so a handler that honoured them would
succeed and mint a different tenant rather than erroring and falling back to the dev account — a
test steering with `../../etc/passwd` would pass against a vulnerable handler. It closes the loop
through the store too: a credential written by the unsteered session must still be readable by a
steered one. Re-running the review's mutation turns this red with *"a query string carrying
tenant=… changed who the dev sign-in signed in as; the dev door is an impersonation primitive"*.

**M10 — unknown arguments are refused.** Deleting the `other => anyhow::bail!(…)` arm from
`options()` left the suite green, and `AGENTS.md:23` requires a failing-first test for a behavioural
change. `options()` is now split into a pure `options_from(args)` so the rule is reachable without
spawning a subprocess, and `main.rs` has the `#[cfg(test)]` module it lacked. Four tests;
`an_unknown_argument_is_refused` and `one_bad_argument_refuses_the_whole_command_line` both go red
under the mutation. This is the guard the whole dev-door design rests on, since the loopback bind is
what makes the door defensible.

**Also done:** `--help`/`-h` now prints usage and exits 0 instead of `unknown argument "--help"` with
exit 1. The usage text names `--bind` and `--port` as deliberately absent, because the person reading
`--help` is exactly the person about to look for them; `usage_names_the_arguments_that_deliberately_do_not_exist`
pins that.

**Not done, by agreement:** M15 (`index.html`'s `if (status.dev)` guard) needs a JS harness this
crate does not have — its own story. The server half is pinned; M12 turned `/auth/status` always-true
red.

**Recorded, not acted on:** the review could not rule out DNS rebinding — a rebound page sends
`Sec-Fetch-Site: same-origin`, so the cross-site check correctly does not fire, and this crate has no
Host/Origin allow-list anywhere. Pre-existing and not specific to the dev door, but it is the one
avenue by which a browser could reach a loopback-only host from off-machine, so it is worth its own
story now that a no-credential door exists.
