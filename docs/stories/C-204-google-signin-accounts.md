---
id: C-204
title: "Google sign-in, accounts and sessions"
pillar: Bridge
status: done
priority: 3
design: docs/designs/connectors-api.md
epic: connectors-api
areas: [bridge, host]
note: "the principal. Until this lands the service has no answer to 'who is asking', which is the question connectors-proxy.md says a credential-injecting service must answer before it injects anything"
---

# Google sign-in, accounts and sessions

## Goal

Give the service a principal: a person signs in with Google, gets an account, and every subsequent
request resolves to exactly one tenant on the strength of that session.

## Why it is not optional, and not last

[connectors-proxy.md](../designs/connectors-proxy.md)'s rejection turned on the deputy problem — a
service that adds authority a caller does not have. The answer to it is not "the service is
authenticated", which that design already dismissed as insufficient; it is **the caller is the
principal whose credential is being used**. That equation is what this story establishes, and every
credential the service holds before it lands is held for nobody in particular.

## Acceptance

- [x] Sign-in with Google via OIDC authorization-code + PKCE. `id_token` is verified — signature
      against Google's JWKS, `iss`, `aud`, `exp`, and `nonce`. A test asserts each check by feeding a
      token that fails exactly one of them.
      → `crates/connectors-api/src/auth/oidc.rs:verify_id_token`; `tests/id_token.rs` (12 tests, one
      per check plus `alg`/`kid`/`sub` and a passing control).
- [x] An account is created on first sign-in and keyed by the OIDC `sub`, **not** by email. Email is
      mutable and reassignable; `sub` is the stable subject identifier, and keying on email is how one
      person inherits another's credentials.
      → `src/auth/session.rs:Account::from_claims`;
      `tests/id_token.rs:two_subjects_sharing_an_email_are_two_tenants`.
- [x] A session is an opaque server-side token, `HttpOnly`, `Secure`, `SameSite=Lax`, with an
      expiry and a revocation path. No credential material and no tenant secret is ever in a cookie.
      → `src/auth/mod.rs:session_cookie`, `src/auth/session.rs:Sessions`;
      `tests/tenancy.rs:the_session_cookie_is_opaque_and_locked_down`.
- [x] **Failing-first:** a test proving a request carrying tenant B's identifier but tenant A's
      session resolves to tenant **A** — the tenant comes from the session, never from the request
      body, the path, or a header.
      → `tests/tenancy.rs:a_request_naming_another_tenant_resolves_to_the_session_s_tenant`.
- [x] Sign-out revokes server-side, so a stolen cookie stops working.
      → `src/auth/routes.rs:signout`;
      `tests/tenancy.rs:signing_out_revokes_the_session_for_a_stolen_copy`.
- [x] The Google client secret is resolved from the environment or the secret store, never from a
      provider TOML, a generated artifact, or a committed file.
      → `src/auth/oidc.rs:Settings::from_env`; swept alongside the connector credential in
      `tests/host.rs:a_stored_credential_reaches_no_surface`.

## Notes

- `codewandler-flux-credentials` (0.42.1) already implements PKCE generation, the S256 authorize-URL
  builder and the form-encoded token exchange, and flux's CLI drives it three times over
  (`login_claude`, `login_codex`, `login_plugin`). **Reuse rather than write a fourth** — a fourth
  PKCE implementation in this ecosystem is how two drift on a security-relevant detail. Its 0.42
  version against this workspace's 0.41 pin is the open question; see
  [C-202](C-202-flux-web-egress.md)'s skew note.
- Google's OIDC sign-in and the `google` **connector** are different things and must not be conflated.
  Signing in proves who the operator is; it does not mint a token for `google-gmail-message-get`.
  That is [C-207](C-207-oauth2-connect-flow.md), with different scopes and a different consent screen.

## Progress

**2026-07-31 — implemented.** New module `crates/connectors-api/src/auth/` (`oidc`, `session`,
`routes`, and the `Principal` extractor); five routes; `api.rs`'s `tenant_of()` constant deleted.

### The 0.42 skew question, measured — and the note above is wrong on a checkable fact

The Notes say `codewandler-flux-credentials` **0.42.1** has the PKCE pieces and flag its skew
against the 0.41 pin as the open question. Both halves turned out differently:

1. **0.42.1 does split the flux line.** It requires `flux-core ^0.42.1` and `flux-provider ^0.42.1`,
   which are semver-incompatible with the workspace's 0.41. A scratch resolve carrying this
   workspace's four flux pins plus flux-credentials 0.42.1 puts `codewandler-flux-core` **0.41.1 and
   0.42.1** in one `cargo tree -d`, along with two `flux-provider`s and two `flux-credentials`.
2. **0.41.0 has the same three functions, and is already in `Cargo.lock`.** `generate_pkce`,
   `generate_state` and the generic `oauth_authorize_url` (S256) all ship in 0.41.0, which
   `flux-web` already drags in via `flux-plugin`.

So the reuse the story asks for costs **zero new crates and zero new duplicates**, and the skew does
not apply to the code being reused. `cargo tree -d | grep codewandler` is empty.

**What is not reused, and why:** `oauth_token_grant` returns an `OAuthToken` that *drops the raw
`id_token`*, keeping only an OpenAI-specific account id read out of it with an **unverified** base64
decode. The raw token is the artefact this story must verify, so the code exchange is ~15 lines of
`reqwest` form encoding here. The PKCE half — where a fourth implementation would actually be a
hazard — is still `flux-credentials`'.

### Dependencies added

- `codewandler-flux-credentials` 0.41 — PKCE, `state`, authorize URL. Already in the lock.
- `jsonwebtoken` **`~10.3.0`**, `default-features = false`, `features = ["aws_lc_rs"]` — the
  `id_token` verifier. `aws_lc_rs` over `rust_crypto` because `aws-lc-rs` is already in the lock
  under rustls; `rust_crypto` would add `rsa`, `p256`, `p384`, `ed25519-dalek` and a second `hmac`.

  **A finding, caught only by checking the resolved lock rather than the requirement.** 11.0.0
  declares `rust-version = 1.88.0`, so the requirement was first written `"10.3"` to stay under this
  workspace's declared 1.87 — and **10.4.0 declares 1.88.0 too**. A caret requirement resolved
  straight to it, and because `resolver = "2"` does no MSRV-aware resolution, nothing warned: the
  workspace silently stopped being buildable on the toolchain it advertises, while the manifest
  comment claimed the opposite. `~10.3.0` (`>=10.3.0, <10.4.0`) keeps patch releases of the 10.3
  line floating in and excludes the MSRV bump.

  **The pin is against a version, not a judgement that 1.87 is right.** Raising the workspace MSRV
  to 1.88 may well be correct, but `rust-version` sits in `[workspace.package]` and is inherited by
  four *published* crates, so moving it is a repository-wide decision and not a side effect of a
  sign-in story. **Left for the coordinator.**
- `aws-lc-rs` 1.17 as a **dev**-dependency — RSA keygen for the tests, so no private key is
  committed. Already in the lock.

C-203's manifest recorded "no JWT library" as deliberate; that comment is rewritten rather than
left standing, because this story's acceptance requires the signature checked and a
TLS-authenticated userinfo read proves who *answered*, not who *signed*.

### Where the owner's goal outranked a literal reading

The Acceptance says the session cookie is `Secure`, and the owner's goal is a locally deployable
app. Those pull against each other only if `Secure` breaks `http://localhost` — it does not, because
Chrome and Firefox treat localhost as a trustworthy origin. So `Secure` is set unconditionally, with
**no environment variable to drop it**: a downgrade switch is one somebody forgets to unset in front
of a real deployment. Safari has historically been stricter, and that is recorded in the crate
README rather than worked around.

Serving the goal also decided the unconfigured case. A host with no Google registration **starts**,
serves its page, and names the two unset variables at the console, at `/auth/signin` (`503`), at
`/auth/status` and on the page itself. Panicking would turn a first `cargo run` into a stack trace;
starting silently would turn it into a button that leads nowhere.

### Beyond the Acceptance, because the review is adversarial

- `alg` is **pinned** to RS256 rather than read from the token — `alg: none` and
  HMAC-with-the-public-key are both complete bypasses. Two tests.
- The `state` is **single-use** (`take_login` removes it), which is the **replay** defence; a test
  asserts an unsolicited callback is refused. *(This bullet originally called single-use "the
  login-CSRF defence". It is not, and that error is what left the route exploitable — see the
  rework section below.)*
- The session record is keyed by **SHA-256 of the token**, so the store never holds a usable cookie.
- The `sub` is **validated as a path segment** before it becomes a tenant — `AGENTS.md`'s "validate
  any new path segment at construction", with action-proxy as the cautionary precedent.
- The token-exchange error path reports the **status only, never the body**: a token endpoint that
  echoed the request back would put this host's `client_secret` into a response body.
- The nonce comparison does not exit early.
- `tests/host.rs`'s no-leak sweep was extended to the auth routes, to error paths, and to two more
  secrets — the Google client secret and the session token.

### Not done here

- Accounts and sessions are **in memory**, like credentials. Persistence is slice 5.
- Nothing here touches `CHANGELOG.md`, the board, or `docs/roadmap.md`.

## Progress — rework, 2026-07-31

**The first landing shipped a critical login-CSRF hole. An independent adversarial review
reproduced cross-account credential capture end to end, and this is the fix.**

### What was wrong

`/auth/signin` set no cookie. The pending login lived in a process-global map keyed only by the
`state`, with nothing tying an entry to the user-agent that began the flow, and `/auth/callback`
redeemed it with `take_login(&state)` without ever asking which browser was in front of it.

So a `state` issued to an *attacker's* browser could be redeemed from a *victim's*. The victim —
one fetch of a callback URL, a top-level `GET`, so a link or an `<img>` suffices — is silently
signed in **as the attacker**, lands on a working page, and every credential they then paste is
written to the attacker's tenant, where the attacker reads it back and runs operations with it. The
reviewer's reproduction, with a victim client that never called `/auth/signin` and carried no
cookie:

```
victim's session resolves to: {"subject":"ATTACKER","tenant":"google-ATTACKER"}
victim stored credential: 204 No Content
attacker sees api_token: {"address":"tenants/google-ATTACKER/com.anthropic.api/api_key",
                          "stored":true, ...}
```

### Why my own tests did not catch it

**I conflated two different properties.** The Progress note above claimed single-use `state` was
"the login-CSRF defence". It is not — it is a *replay* defence, and it stops the same callback
being redeemed twice, which is a different attack. RFC 6749 §10.12 requires the binding value be
kept *"in a location accessible only to the client and the user-agent"*, i.e. a cookie, and there
was none.

The guard test `a_callback_with_an_unknown_state_is_refused` could not see the hole because it only
ever presents a state that was **never issued**. Nothing in the suite ever presented a *live* state
from a *different* browser, which is the whole attack. A test written from the mechanism I believed
I had, rather than from the attack, agreed with me.

### The fix

`/auth/signin` now sets `connectors_login`, and `/auth/callback` requires it and compares it
against the `state` query parameter **before `take_login` is consulted** — so a cross-site attempt
cannot even consume somebody else's live state as a side effect of being refused. The cookie is
`HttpOnly`, `Secure`, `SameSite=Lax`, `Path=/auth/callback`, `Max-Age` = `LOGIN_TTL`, and is
cleared on every exit from the callback. Single-use is kept as well; both properties are needed.

**`Lax`, not `Strict`, and it is load-bearing**: the callback arrives as a cross-site top-level
`GET` redirected from the identity provider, and `Strict` withholds cookies on exactly that
navigation — it would turn the fix into a sign-in that can never complete.

Four new tests in `tests/tenancy.rs`, and the first is the reviewer's scenario:

- `a_state_issued_to_one_browser_cannot_be_redeemed_by_another` — asserts on **the identity that
  results**, not the status code, because the failure mode is a healthy `200` belonging to somebody
  else. Verified failing against the vulnerable code first.
- `a_callback_whose_cookie_and_state_disagree_is_refused` — the cookie is *present* but from
  another flow.
- `the_login_cookie_is_scoped_short_lived_and_locked_down`
- `completing_a_sign_in_clears_the_login_cookie`

Two existing tests were also repaired: they drove the flow inline without the new cookie, and
`the_session_cookie_is_opaque_and_locked_down` was additionally reading "the first `Set-Cookie`" —
which after the fix is the *cleared login* cookie, carrying every attribute it checks and an empty
value. It would have passed while proving nothing.

### Also in this pass

1. **Both back-channel clients are bounded.** `reqwest::Client::new()` had no timeout, no connect
   timeout, no redirect policy and an unbounded `text()`. These two calls — the token exchange and
   the JWKS fetch — are deliberately outside flux's `Egress`, so the bounds `Egress` would have
   applied are applied here: a 10s total and 5s connect timeout, **no redirect following** (a
   redirect on the token call would re-send the `client_secret` wherever it pointed), and
   `read_bounded` caps the body at 256 KiB / 1 MiB by checking `Content-Length` and then
   accumulating chunk by chunk, so a lying or absent length is caught too. One client, built once,
   rather than a fresh connection pool per request.
2. **`/v1/operations/{operation}` is now gated.** The router comment claimed every other route took
   a `Principal` and this one did not. It serves only published catalogue data, so nothing
   tenant-scoped was leaking — but "all of them except one, and that one is fine" is a rule nobody
   can check at a glance, and the exception is what a later change inherits. **The route was gated
   rather than the comment softened**, since the catalogue is public through `catalog.json` anyway.
3. **`nbf` and `iat`, decided rather than left unstated.** `validate_nbf` is turned **on**: Google
   does not currently send `nbf`, and `jsonwebtoken` only checks a claim that is present, so it
   costs nothing today — which is exactly why leaving a standard temporal claim unenforced because
   the current issuer happens not to send it is a bad trade. `iat` is now required and rejected if
   more than the leeway in the future, which `exp` cannot catch (a token issued an hour early with
   a one-hour life is unexpired for two). Deliberately **no** maximum-age check on top: that is
   `exp`'s job, and a second differently-tuned freshness rule refuses valid tokens for reasons
   nobody can reconstruct. New variants `NotYetValid` and `IssuedInTheFuture`, three tests.
4. **The key-confusion test now does what its name says.** It previously signed RSA bytes under an
   `HS256` header — merely a broken signature, which a still-exploitable verifier would also
   refuse. It now forges a genuine `HMAC-SHA256` tag keyed on the **published RSA modulus**, which
   is the actual attack, and asserts the header really says `HS256` before asserting the refusal.
5. **`docs/designs/connectors-api.md` exists now** (C-201 created it); the earlier note saying it
   did not is removed.
