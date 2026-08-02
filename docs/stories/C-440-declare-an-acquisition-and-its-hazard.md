---
id: C-440
title: "An `[[auth]]` block can declare an acquisition the host performs, and the hazard it carries"
pillar: Spec
status: backlog
design: docs/designs/connector-security-posture.md
epic: connector-security-posture
areas: [connector-spec, catalog]
note: "the fourth route C-432 did not list, and the one the authentication contract already implies: a token exchange is not an operation to mark, it is an ACQUISITION to declare — plus a hazard field, so a host can refuse the OAuth2 password grant by property rather than by connector name. Also the first `[[auth]]` quirk: babelforce's token endpoint reads expires_in per-grant and switches accounts on account_id, and no document declares either"
---

# An `[[auth]]` block can declare an acquisition the host performs, and the hazard it carries

## Goal
A provider file can state *how* a credential is obtained — not only that one exists — and can name a
**declared weakness** in that acquisition, so a host refuses it by property rather than by connector
name.

## Why now

Owner-raised in flux-exchange, 2026-08-01: babelforce supports the OAuth2 **password grant**, and a
flux-exchange operator should be able to use it — with the weakness stated, and refused in production
unless somebody opts in.

The motivation is concrete and it is this repository's problem too. `Acquisition` ships one usable
value, `Static` — *the stored secret, unchanged* — and `providers/babelforce.toml`'s only `[[auth]]`
block says what that means:

> `description = "SSO-issued babelforce access token, minted outside flux and supplied through the
> environment"`

**Minted outside flux.** A babelforce user has an email address and a password, not a token. So the
provider with 389 catalogued operations is the one a host cannot connect without an out-of-band step
nobody has documented.

## This is the fourth route, and it needs no decision from flux

[[C-432]] is blocked on an owner choice between three routes, all of which treat the token exchange as
an **operation** whose response must be marked. This is the option none of them is: **do not make it an
operation at all.** `AGENTS.md` § Authentication contract already says so, twice, in the owner's own
words:

> An authentication endpoint is never a connector operation. […] That is a property of the connector's
> authentication surface — `OAuth2Spec`, the grant, the redirect — and it is what the *host* performs.

and

> The host resolves the credential, **performs effectful acquisition such as OAuth2**, applies the
> placement scheme, and registers values with its redactor.

Everything blocking C-432 is downstream of emitting a `.flux` module that binds a raw token to a
model-visible symbol. **A declaration emits no module.** The three withheld paths stay withheld,
babelforce's `389 + 5 + 3 = 397` accounting is untouched, and nothing new reaches `connector_pack::resolve`.

## What to declare

Three additions to the `[[auth]]` surface. The second is the one flux-exchange is waiting on; the
third is the one that keeps the second from growing fields it should not have.

1. **The acquisition.** Enough for a host to perform an OAuth2 grant without guessing: the grant kind,
   the token endpoint, and which inputs it needs. The vendored document is the source —
   `specs/babelforce/auth-2026-06-25.openapi.yaml` declares `securitySchemes.oauth2.flows` with both
   `authorizationCode` and `password`, and `OAuthTokenRequest`'s `grant_type` enum carries
   `authorization_code | refresh_token | password | client_credentials`.
2. **The hazard.** A closed vocabulary of named weaknesses in *how* a credential is obtained, whose
   first value is the resource owner's password reaching a client that is not the authorization
   server. flux-exchange spells the same value `AuthHazard::ResourceOwnerSecretShared` and cites
   **RFC 9700 §2.4** — the OAuth 2.0 Security BCP, which says the resource owner password credentials
   grant **MUST NOT** be used, because it exposes the owner's credentials to the client, widens where
   they leak, and cannot carry two-factor authentication — plus **RFC 6749 §4.3** (the client MUST
   discard them once a token is obtained) and **CWE-522**. OAuth 2.1 drops the grant entirely.

**A hazard is a kind, not a level.** It does not belong on `risk`, which is an ordered damage claim
about what an operation does to vendor data, decided per (document, method class) by
`[[patch.select]]`. A password grant that buys a read-only token is low risk *and* hazardous.

3. **Quirks, on the auth surface.** This repository already carries the word and its discipline —
   `quirks.pagination` and `quirks.rate_limit`, *declarations, not behavior*, IR and loader only. What
   is new is the **scope**: today `Quirks` hangs off an operation, and a token endpoint is not one. It
   has to hang off `[[auth]]`.

   Owner-decided 2026-08-02, in flux-exchange: **if it is not in the specification, it does not become
   a general thing.** The occasion was a token lifetime. The owner said babelforce's token endpoint
   accepts one; `OAuthTokenRequest` in `specs/babelforce/auth-2026-06-25.openapi.yaml` declares eleven
   properties and none is a lifetime. **Both were true** — `AuthController.token()` reads `expires_in`
   directly out of `params`, which is exactly why no generated document shows it. Measured 2026-08-02:

   | Grant | `expires_in` on the request | Semantics |
   |---|---|---|
   | `client_credentials` | read, defaulting to `-1` | `-1` means *never expires* |
   | `password` | read when present | otherwise the service default |
   | `refresh_token` | read, passed into the refresh | — |
   | `link` | read, then clamped to **at most 60s** | a fifth grant, also undeclared |
   | `authorization_code` | **not read** | only `access_type`, default `offline` |

   And the precedent the owner named: on `refresh_token`, **`account_id` switches the account** the new
   token belongs to. Nothing in RFC 6749 makes a refresh change whose token it is.

   That table is the argument. One field, five behaviours, one vendor — a general `requested_ttl` would
   be a hard cap here, ignored there, and the difference between an hour and forever somewhere else,
   while inviting the other fifty-three providers to be assumed to honour something none of them
   declares.

## Acceptance
- [ ] `[[auth]]` accepts an acquisition declaration and a hazard, in the IR, the loader and the
      generated catalogue, with the JSON schema updated in the same change.
- [ ] The hazard is a **closed set**: an unrecognised spelling **refuses at the loader**, naming the
      value. A free-form string makes a typo read as *no hazard declared*, which admits.
- [ ] **Failing-first test** — a provider declaring `hazard = "resource_owner_secret_sharing"` (the
      near-miss spelling) is refused, and the refusal names the value. Watch it fail first.
- [ ] `providers/babelforce.toml` declares the password grant and its hazard on `[[auth]]`, with the
      comment stating that the three `/oauth/*` paths remain withheld and why.
- [ ] `crates/connector-spec/tests/babelforce_coverage.rs` is **unchanged and green**: the accounting
      stays `389 + 5 + 3 = 397`. A declaration is not a selection, and this is the test that proves it.
- [ ] Nothing is emitted into any `.flux` module. `connector-flux`'s refusal of `produces_credential`
      is untouched — this route does not need it lifted.
- [ ] `[[auth]]` accepts **quirks**, and babelforce declares its token endpoint's — per grant, with the
      grant that ignores `expires_in` recorded as ignoring it. An empty cell in that table is a
      measurement, not a gap.
- [ ] **Failing-first test** — a quirk declared on one connector's auth surface does not reach
      another's. Write the leak first.
- [ ] Every quirk carries its **attribution and the date it was measured**. It is asserted against the
      vendor's implementation and contradicted by the vendor's own document, so a reader a year from
      now needs to know which of the two this repository checked and when. `providers/babelforce.toml`'s
      unanswerable `X-Auth-Access-Id` question is what an unattributed one costs.
- [ ] `AGENTS.md`'s quirks table (§ the `quirks.pagination` / `quirks.rate_limit` inventory) gains the
      new scope, so the count stays a measurement rather than becoming stale on the day this lands.

## Progress
- 2026-08-01 — filed from flux-exchange's `credential-acquisition` epic (X-72…X-76). The consuming
  side is X-73 (the hazard vocabulary) and X-74 (the deployment filter that refuses it unless the
  operator opted in).

## Notes
- The consumer is **flux-exchange**: `docs/designs/credential-acquisition.md` there records why the
  host performs the grant, and why a port in `exchange-host` with its HTTP binding in
  `exchange-server` is the shape — the same split `TokenExchange` / `http_exchange.rs` already uses
  for sign-in.
- **The vendored documents are incomplete, and that is now a measured claim rather than a suspicion.**
  Beyond the request-side `expires_in` and `account_id` above: the token response carries
  **`expire_time`** (absolute UTC milliseconds) beside the standard `expires_in`, `GET /oauth/tokeninfo`
  exists, and `link` is a fifth `grant_type`. None of the five vendored documents declares any of them.
  Raise it with the babelforce API owners as a **documentation gap** — a quirk that becomes spec should
  stop being a quirk, and this repository's whole vendoring discipline is built on the document being
  the truth. `providers/babelforce.toml` already carries one open question for those owners, about
  `X-Auth-Access-Id` / `X-Auth-Access-Token`; this is the second, and unlike the first it has an answer
  attached.
- **What deliberately does not come across from the vendor's source**: internal client names and their
  scope assignments sit beside the code that was measured, and they are nobody's business here.
