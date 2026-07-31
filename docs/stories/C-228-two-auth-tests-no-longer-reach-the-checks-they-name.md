---
id: C-228
title: "Two auth tests no longer reach the checks they name, one gated route has no negative test, and three documents describe the login flow as it was before C-204"
pillar: Bridge
status: in-progress
priority: 2
design: docs/designs/connectors-api.md
epic: connectors-api
areas: [bridge, host]
note: "found by C-204's independent security re-review 2026-07-31, which passed the fix itself — the exploit was reproduced at the base and proved dead on main. These are the residue: coverage that moved when the guard moved, and prose that did not"
---

# Two auth tests no longer reach the checks they name

## Goal

Restore the coverage C-204's fix silently displaced, and make three documents describe the flow that
actually ships.

## Why this is not "C-204 was wrong"

C-204's re-review returned **PASS** on evidence, not on reading: the reviewer reproduced the
cross-account capture at the base (`me=200 OK {"email":"ATTACKER@example.test","tenant":"google-ATTACKER"}`),
then drove merged `main` and got victim callback `400`, no session set, and nothing in the attacker's
tenant. Absent cookie, mismatched cookie, empty-cookie-and-empty-state, uppercase cookie name,
duplicate `state` parameters, replayed flow, `POST`, and three path variants all fail closed.

**The fix is sound. This story is the residue** — when a new guard runs *before* an old one, tests
aimed at the old one stop reaching it, and they keep passing.

## What was measured

**1. A test that no longer reaches its own subject.**
`crates/connectors-api/tests/tenancy.rs:578` — `a_callback_with_an_unknown_state_is_refused` presents
no cookie, so it now stops at the *binding* check and never reaches `take_login`. Proven by the
host's own messages differing:

| request | message |
|---|---|
| no cookie + unissued state | `this callback did not come from a browser that started a sign-in here` |
| cookie == unissued state | `this callback does not correspond to a sign-in this host started` |

The test's name claims the second; it exercises the first. The route-level `take_login` refusal is
now covered only by store-level unit tests at `crates/connectors-api/src/auth/session.rs:400-430`.

**2. A gated route with no negative test.** `/v1/operations/{operation}`
(`crates/connectors-api/src/api.rs:196-206`) was newly gated. `tests/tenancy.rs:103-108` enumerates
four *other* routes, and `tests/host.rs:91` fetches this one **with** a session and only greps the
body for secrets — so a `401` would satisfy it too. Nothing fails if the gate comes off.

**3. Prose that describes the previous behaviour.**
- `docs/stories/C-204-google-signin-accounts.md:201` says the login cookie "is cleared on every exit
  from the callback". It is not: the provider-error branch (`auth/routes.rs:97-103`) and the
  missing-code branch (`:104-106`) return via `refuse` with no `Set-Cookie`. The doc comment on
  `refuse_and_clear` (`:329-334`) states the real rule — *from the binding check onwards*. **No
  security effect** — neither branch consumes the state and the same browser can retry — but the
  story and the code disagree.
- `crates/connectors-api/README.md:68-72` still says a browser refusing the `Secure` attribute "will
  not hold the session". Now that the *login* cookie is also `Secure`, such a browser fails earlier
  and harder: the callback returns `400` and sign-in cannot complete at all.

## Acceptance

- [x] **Failing-first test:** a callback carrying a cookie that matches an *unissued* state is
      refused by `take_login` at the route level. Today no route-level test reaches that branch.
      Name it. Keep `a_callback_with_an_unknown_state_is_refused` as the no-cookie case, renamed to
      say so — two branches, two tests, each named for what it exercises.
- [x] `/v1/operations/{operation}` gets a negative test asserting `401` without a session, sitting
      with the four routes already enumerated at `tests/tenancy.rs:103-108` rather than somewhere new.
- [x] `tests/host.rs:91`'s no-secrets sweep is made to fail if the response is a `401`. A guarantee
      that passes on an error page proves nothing about the surface it claims to cover.
- [x] The three prose statements above are corrected to describe the shipped flow. Prefer deriving
      the claim from the code where it is cheap.
- [x] The residual is written down where an operator reads it, not only in a review: the binding is a
      **double-submit whose cookie value is the URL-visible state**, with no `__Host-` prefix
      (`crates/connectors-api/src/auth/mod.rs:113-118`), so cookie integrity is the whole defence.
      Presenting `Cookie: connectors_login=<attacker state>` alongside the attacker's state does
      yield a session — it requires a cookie-injection foothold (sibling subdomain, XSS), which is
      outside C-204's link-or-`<img>` threat model and is true of most double-submit
      implementations. Record why `__Host-` is foreclosed (`Path=/auth/callback`) so the next reader
      does not re-derive it.

## Notes

- **The pattern is the point, and this wave hit it four times.** C-204's fix already repaired two
  tests passing for the wrong reason (`the_session_cookie_is_opaque_and_locked_down` read "the first
  `Set-Cookie`", which after the fix is the *cleared login* cookie); C-216's `Bot`/`Bearer` scan was
  found inert because `crates/connector-flux/src` never references `AuthScheme`; and this is two
  more. A test that stops reaching its subject does not fail — that is what makes it expensive.
- Not in scope, recorded so it is not misread as introduced by C-204: the `error` query parameter is
  reflected into the refusal body at `auth/routes.rs:100`, served as `application/json` with serde
  escaping and no `X-Content-Type-Options: nosniff`.
- **No real-browser verification exists for any of this.** Every probe was curl or reqwest, so cookie
  ordering under a shadowing `connectors_login`, Safari's handling of `Secure` on `http://localhost`,
  and actual `SameSite=Lax` delivery on the provider's cross-site redirect are unconfirmed. Settling
  those needs a browser against a real Google registration and is worth its own story if it does not
  fit here.

## Progress

**2026-07-31 — implemented on `impl/C-228`. Full gate green, zero red across 131 test binaries.**

### The base measurement, which came out worse than the story predicted

The story says the route-level `take_login` refusal is "covered only by store-level unit tests".
Measured at the merge base (`8706fe2`) by deleting the entire guard —

```rust
let (verifier, nonce) = app.sessions().take_login(&state).unwrap_or_default();
```

— **all 60 tests in `connectors-api` passed**, across all six binaries. Not merely the one test that
had drifted: nothing anywhere in the crate observed that the issued-here/single-use check had been
removed from the route. That is the failing-first evidence for this story.

### What the two tests now pin, and why the message is the assertion

Every refusal in `callback` answers `400` and clears the binding, so **the status cannot distinguish
the three branches** — the message is the only observable difference. The three messages are now
`pub const`s in `auth::routes` (`NO_BINDING`, `BINDING_DISAGREES`, `NO_SUCH_SIGN_IN`) and the tests
name the constant. A test holding its own copy of the string would have drifted exactly as the
original did; with one string there is nothing to drift from.

- `a_callback_with_no_login_cookie_is_refused` (renamed from
  `a_callback_with_an_unknown_state_is_refused`) — no cookie, asserts `NO_BINDING`.
- `a_callback_whose_cookie_matches_an_unissued_state_is_refused` (new) — cookie == an unissued
  state, asserts `NO_SUCH_SIGN_IN`. Its `code` deliberately carries a nonce, so a host that skipped
  `take_login` proceeds into the exchange and fails *later* with a different message rather than not
  failing at all — which is what the mutation below actually observed.

### Every test verified by mutation

One fact changed in the code each time; the right test went red and the others stayed green.

| mutation | expected red | observed |
|---|---|---|
| `take_login` guard deleted | the new unissued-state test | **red**, body `the id_token's nonce does not match this sign-in` — it reached the exchange. No-cookie test stayed green, reproducing the original defect exactly |
| missing cookie defaulted to the URL `state` | the renamed no-cookie test | **red**, body `…does not correspond to a sign-in this host started` — it fell through to the next guard. C-204's `a_state_issued_to_one_browser_cannot_be_redeemed_by_another` also went red, correctly |
| `_principal: Principal` removed from `api::operation` | `an_unauthenticated_request_is_refused` | **red** on `GET /v1/operations/anthropic-models-list answered without a session` |
| `api::operation` returns `401` | `a_stored_credential_reaches_no_surface` | **red** on the new status assertion — and the *base* version of that test passed under the same mutation, which is the defect item 3 names |

The two `api.rs` mutations were temporary, applied and reverted in this worktree; `api.rs` is
untouched by this story (C-212 owns it).

### The three prose corrections

1. `C-204:201` — "cleared on every exit from the callback" → "from the binding check onwards", with
   a note that the two branches in front of it return through `refuse` and set no cookie, and that
   this has **no security effect** because neither consumes the `state`.
2. `C-204:127` — the `Secure`/Safari paragraph reasoned about the session cookie alone. Noted that
   `connectors_login` being `Secure` too moves the failure from "does not stay signed in" to
   "cannot sign in at all".
3. `README:69` — "will not hold the session" → **cannot sign in at all**, and named the actual
   mechanism (cookie dropped at `/auth/signin`, callback then answers `400`). The attribute list was
   *removed* rather than corrected: it now points at `session_cookie`/`login_cookie` and at the test
   that asserts them, so there is no second copy to go stale.

### The residual, recorded for an operator

`crates/connectors-api/README.md` gains **"What the binding does not cover"**: the binding is a
double-submit whose cookie value *is* the URL-visible `state`, so cookie integrity is the whole
defence and a cookie-injection foothold defeats it — outside C-204's link-or-`<img>` threat model
and ordinary for double-submit. It records that `__Host-` is **foreclosed by `Path=/auth/callback`**
(the prefix requires `Path=/`), that narrow scoping was the deliberate choice, and what the price of
revisiting it would be. `auth::login_cookie`'s doc comment carries the short form and points here.

### Not done, and deliberately

No test asserts the residual itself (that an injected cookie *does* yield a session). Pinning a
weakness as expected behaviour makes it harder to remove later; the README records it instead.
