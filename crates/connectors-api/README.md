# connectors-api — the host

The caller this repository has never had.

Everything below this crate already worked and was tested: `connector-pack` projects a catalogue
operation onto a flux `ToolSpec`, evaluates `{ method, url, headers, body }` from the operation's own
emitted Flux, resolves the credential, registers it with flux's redactor, verifies the registration
took, places it per its declared scheme, and delegates the send. What was missing was something to
bind the ports and run the loop. This is that.

```bash
cargo run -p connectors-api
# connectors-api listening on http://127.0.0.1:8787
```

Open it, sign in with Google, pick a connector, paste a credential, run an operation.

## Setting up sign-in

Everything this host holds belongs to a signed-in account, so a bare `cargo run` will start and
then tell you it cannot sign anybody in. It does not panic and it does not serve a broken page —
the process binds, the page renders, and both the console and `/auth/signin` name the two variables
that are unset.

**In [Google Cloud Console](https://console.cloud.google.com/apis/credentials):**

1. Pick or create a project, then *APIs & Services → Credentials → Create credentials → OAuth
   client ID*.
2. Application type: **Web application**.
3. Under *Authorized redirect URIs* add exactly:

   ```
   http://localhost:8787/auth/callback
   ```

   It must match character for character; Google compares the whole string. If you change the port,
   change it in both places.
4. On the OAuth consent screen, the only scopes needed are `openid`, `email` and `profile`. That is
   all sign-in asks for — see *[Signing in is not connecting](#signing-in-is-not-connecting)*.

**Then, in the shell you run the host from:**

```bash
export CONNECTORS_GOOGLE_CLIENT_ID='<the client id>.apps.googleusercontent.com'
export CONNECTORS_GOOGLE_CLIENT_SECRET='<the client secret>'
# Optional; this is the default:
export CONNECTORS_GOOGLE_REDIRECT_URI='http://localhost:8787/auth/callback'

cargo run -p connectors-api
```

**The client secret comes from the environment and from nowhere else.** Not a provider TOML, not a
generated artifact, not a committed file. `crates/connectors-api/src/auth/oidc.rs` reads it once at
startup and hands it to exactly one caller — the token exchange — and its `Debug` prints
`<redacted>`. Nothing in this repository may contain a value shaped like a real one; GitHub push
protection has blocked a release here before.

Then open <http://127.0.0.1:8787>, click **Sign in with Google**, and the header will show your
address and your tenant.

### Signing in is not connecting

Signing in proves **who the operator is**. It mints no token for `google-gmail-message-get`. That is
[C-207](../../docs/stories/C-207-oauth2-connect-flow.md) — a different flow, different scopes, and a
consent screen the operator sees when they connect a provider rather than when they arrive.

### Two things worth knowing about running it locally

- **The session cookie is `Secure`**, and that is not relaxed for local use. Chrome and Firefox
  treat `http://localhost` as a trustworthy origin and accept `Secure` cookies there, so this costs
  nothing on those two. A browser that does not — Safari has historically been strict — will not
  hold the session. The alternative was an environment variable that drops `Secure`, which is a
  switch somebody forgets to unset in front of a real deployment.
- **Sessions live in memory**, like credentials. Restarting the process signs everyone out, which
  is the same cleanup story the rest of this host has.

## The tenant comes from the session

Every port is bound per tenant — `Credentials::new(store, tenant)`, `Configuration::new(values,
tenant)` — and the tenant is derived from the OIDC `sub` of the signed-in account. **It is never
read from a path segment, a body field, a header or a query parameter.**

That is enforced by shape rather than by convention. The tenant is reachable only through
`auth::Principal`, an axum extractor whose sole constructor is a live session cookie: there is no
`Principal::from(&str)` and no tenant path segment for a caller to set. A handler that needs a
tenant names it in its signature; one that does not cannot reach it.

- **The account is keyed on `sub`, never on email.** An address is mutable and reassignable, so a
  host keying on email hands the next holder of `alice@example.com` everything the last one
  connected — silently, on their first sign-in.
- **The session token is opaque and server-side.** 32 bytes of OS entropy naming a record held
  here; the record is stored under the SHA-256 of the token, so the store never holds a usable
  cookie. No credential material and no tenant secret is in it.
- **Sign-out revokes server-side**, so a copy of the cookie taken by somebody else stops working
  too — not only the browser that asked.
- **A sign-in is bound to the browser that began it.** `/auth/signin` sets a short-lived
  `connectors_login` cookie carrying the OAuth `state`, and `/auth/callback` refuses unless that
  cookie is present and agrees with the `state` in the URL.

### The login-CSRF hole this crate shipped once

Worth stating plainly, because the shape of the mistake is more instructive than the fix.

The first version had no `connectors_login` cookie. The `state` lived only in a server-side map, so
**any** browser presenting **any** live `state` could redeem it. An attacker began a sign-in in
their own browser, kept the `state`, and got a victim to fetch the callback URL — a top-level `GET`,
so a link is enough. The victim was silently signed in *as the attacker*, saw a perfectly healthy
page, and every credential they pasted went to the attacker's tenant.

The code consumed the `state` on first use and a comment called that "the login-CSRF defence". It is
not. Single-use is a **replay** defence — it stops one callback being redeemed twice. Binding is a
**CSRF** defence — it stops a callback being redeemed by a browser that did not start the flow.
RFC 6749 §10.12 asks for the second explicitly, requiring the value be kept *"in a location
accessible only to the client and the user-agent"*. Both are needed and both are now enforced;
`tests/tenancy.rs:a_state_issued_to_one_browser_cannot_be_redeemed_by_another` is the regression
test, and it asserts on the resulting **identity** rather than a status code.

`SameSite=Lax` on that cookie is not a weaker choice than `Strict` — it is the only workable one.
The callback arrives as a cross-site top-level `GET` from Google, and `Strict` withholds cookies on
exactly that navigation.

### The two requests that bypass the egress guard

The token exchange and the JWKS fetch do not go through flux's `Egress`, because their URLs are
operator configuration rather than anything a caller chose. That is deliberate, and it means the
bounds `Egress` would have supplied are applied directly: a 10-second total timeout, a 5-second
connect timeout, **no redirect following** — a redirect on the token call would re-send the client
secret wherever it pointed — and a hard cap on response size, checked against `Content-Length` and
then enforced chunk by chunk so a lying length is caught too.

`crates/connectors-api/tests/tenancy.rs` asserts it adversarially: a request that names tenant B in
a body field, two headers and a query parameter, while carrying tenant A's session, resolves to
**A**. `tests/id_token.rs` asserts each `id_token` check separately by feeding a token that fails
exactly one of signature, `iss`, `aud`, `exp` and `nonce`.

## What it does not do

**It constructs no request.** Every route ends in `connector_pack::pack`, and the bytes that reach a
vendor are the ones the pack evaluated from the operation's Flux. A host that built its own requests
would be a second opinion about what an operation *is* — the drift
[C-117](../../docs/stories/C-117-pack-codegen.md) exists to catch, and the structural reason
`connectors-app` superseded `connectors-proxy`.

**It ships no transport.** `flux_web::http::HttpRequestTool` is flux's, configured once and handed to
every operation as an `Egress`, so connectors inherit the host's egress allow-list and SSRF guard
rather than a policy this crate invented. The default is `PrivateNetAllow::None` — private, loopback
and link-local hosts are refused.

**It is never published.** `publish = false`, and it is a leaf: nothing in the workspace may depend
on it. `crates/connector-cli/tests/dependency_fence.rs` holds both directions — the four compiler
crates cannot reach it, and a new workspace member that is neither compiler, host library, nor
declared network crate fails the build.

## The live leg, performed and labelled

`docs/designs/connectors-app.md` asks for this to be recorded rather than claimed, and
[C-149](../../docs/stories/C-149-vault-live-leg-reports-ok-when-it-skips.md) is the cautionary
precedent: a live leg that reports OK when it skips is worse than none.

**2026-07-31 — the first byte this repository has ever sent to a vendor.**

```console
$ curl -sX POST localhost:8787/v1/operations/anthropic-models-list/execute -d '{}'
{"error":"config error: `anthropic-models-list` needs a credential and none is stored at
 `tenants/local/com.anthropic.api/api_key` — the request was not sent (1 address(es) tried)"}

$ curl -sX PUT localhost:8787/v1/credentials/anthropic/anthropic.api_key \
      -d '{"value":"NOT-A-REAL-KEY-connectors-api-smoke-test"}'          # 204

$ curl -sX POST localhost:8787/v1/operations/anthropic-models-list/execute -d '{}'
{"tool":"anthropic.models.list",
 "content":"HTTP 401 Unauthorized\n… request-id: req_011Cda4zdnZpLkt5XU2zDYA8 … cf-ray: …\n
            {\"type\":\"error\",\"error\":{\"type\":\"authentication_error\",
             \"message\":\"invalid x-api-key\"}}"}
```

The `request-id`, the `cf-ray` and the Cloudflare `server` header are Anthropic's real API answering.
Both halves matter: without a credential the pack **refuses by address** and sends nothing, and with
one the request goes out through flux's own `http.request`.

## Why the automated tests stop before the socket

To assert *"the request that reached the vendor carried tenant A's credential"* a test needs a vendor
it controls, which means a loopback address — and **no shipped connector can be pointed at one**.
Nine carry a `{placeholder}`, but every one templates a label inside a fixed vendor suffix
(`{subdomain}.zendesk.com`), never the whole host.

The alternative was a substitute `Egress` that records instead of sending, which is exactly what
`connector-pack`'s tests already do for want of a transport — and `Egress`'s own documentation says
what is wrong with calling that proof: *"a stand-in that ignores `body`, or that resolves `url`
against some base of its own, is not a substitute — it is a different connector."* A second stubbed
suite here would grow the count of green tests without growing what is known.

So `tests/host.rs` asserts everything that happens **before** the socket, which is where this crate's
own defects would live: the address a credential resolves at, the tenant it belongs to, whether a
value can reach a surface, and that the transport really is `http.request`. The live leg is manual,
and this file is where it is recorded.

## Where it is going

| Slice | State |
|---|---|
| 0 — `flux-web` in the graph, fence extended | **done** |
| 1 — the service, ports, catalogue routes, first live call | **done** |
| 2 — explorer + playground UI | **done** (single page, served from this binary) |
| 3 — Google sign-in, accounts, sessions | **done** (C-204) |
| 4 — OAuth2 connect flows (Google, Slack, GitHub, Notion, HubSpot) | next |
| 5 — OAuth2 into provider TOML; persist secrets | after 4 |

Slice 1 threaded the tenant through every port as a parameter while it was still a single constant
(`local`), rather than adding it later — "the tenant comes from the session, never from the request"
has to hold at every call site, and retrofitting it is how one of them gets missed. Slice 3 made the
substitution, and the constant is gone.

What is still in memory, deliberately, is the *storage*: accounts, sessions and credentials all go
with the process. Persisting them is slice 5's, and until it lands the honest summary is that this
host is durable across requests and not across restarts.

## The charter

This crate contradicts `docs/vision.md`'s current non-goal and the loopback narrowing in
`docs/designs/connectors-app.md`. That is owner-directed, and
[C-201](../../docs/stories/C-201-charter-multi-tenant-host.md) is where the amendment and the redone
confused-deputy analysis land. Until it does, this README is the only place saying so — read
[C-200](../../docs/stories/C-200-connectors-api-epic.md) for the shape of the whole thing.
