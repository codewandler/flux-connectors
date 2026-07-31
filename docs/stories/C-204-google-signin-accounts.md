---
id: C-204
title: "Google sign-in, accounts and sessions"
pillar: Bridge
status: in-progress
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
- The `state` is **single-use** (`take_login` removes it), which is the login-CSRF defence; a test
  asserts an unsolicited callback is refused.
- The session record is keyed by **SHA-256 of the token**, so the store never holds a usable cookie.
- The `sub` is **validated as a path segment** before it becomes a tenant — `AGENTS.md`'s "validate
  any new path segment at construction", with action-proxy as the cautionary precedent.
- The token-exchange error path reports the **status only, never the body**: a token endpoint that
  echoed the request back would put this host's `client_secret` into a response body.
- The nonce comparison does not exit early.
- `tests/host.rs`'s no-leak sweep was extended to the auth routes, to error paths, and to two more
  secrets — the Google client secret and the session token.

### Not done here

- **`docs/designs/connectors-api.md` does not exist**, though this story's frontmatter names it.
  `connectors-app.md` and `connectors-proxy.md` were read instead. The frontmatter is left pointing
  at the missing file rather than repointed, because the epic (C-200) names the same path and
  changing one without the other would be worse.
- Accounts and sessions are **in memory**, like credentials. Persistence is slice 5.
- Nothing here touches `CHANGELOG.md`, the board, or `docs/roadmap.md`.
