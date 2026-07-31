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
- **Sessions live in memory**, and — since C-207 — credentials no longer do. Restarting the process
  signs everyone out and leaves every connector still wired. That asymmetry is deliberate: a session
  is a bearer token somebody's browser is holding, so a restart *should* invalidate it, while a
  credential is a thing the operator deposited and expects to find again. Conflating the two stores
  would make a stolen cookie outlive the process. See [Where the credentials
  live](#where-the-credentials-live).

## Where the credentials live

**In one file, not encrypted, readable by you and by anyone who can become you.** That sentence is
the whole security argument and it is written first on purpose — the rest of this section is where
the file is, how to move it, and how to destroy it.

```
$XDG_DATA_HOME/connectors-api/credentials      # or ~/.local/share/... when XDG_DATA_HOME is unset
```

The host prints the exact path, and everything below, in its startup banner. It is assembled from
the store that was actually bound, so it cannot describe a different one.

| | |
|---|---|
| **What protects it** | `0600` on the file, `0700` on the directory — set in the `open(2)`/`mkdir(2)` call, not `chmod`-ed afterwards, and re-checked every time the host starts. A store whose mode has been widened is **refused, not repaired**: it was already exposed, and quietly tightening it would hide that. |
| | The write is **atomic** — a full rewrite into a sibling temporary, `fsync`, then `rename(2)` — so a crash mid-write leaves the previous file whole rather than a truncated one. |
| | The path is refused if it lies inside the directory the host runs from. One `git add -A` from a committed credential is not a risk this host leaves to attention. |
| **What does not** | **No encryption.** No key, no passphrase, no OS keychain. The values are hex-encoded, and hex is *framing* — it stops a value containing a newline from forging a second entry, and stops a careless `grep` from matching a token. `xxd -r` undoes it. |
| | Nothing stands between the file and `root`, a backup that copies it, or a process running as you. |

If that trade is wrong for the machine you are on, say so — the host does not decide for you:

```bash
# Somewhere else, on a volume you chose. Must be absolute.
export CONNECTORS_CREDENTIAL_STORE=file:/var/lib/connectors-api/credentials

# Nothing at rest at all. The pre-C-207 behaviour, now something you ask for rather than
# something that happens: every connector must be wired again after each restart.
export CONNECTORS_CREDENTIAL_STORE=memory
```

**To destroy them**, which is how an operator revokes:

```bash
rm ~/.local/share/connectors-api/credentials      # all of them
# or, for one connector, through the surface that stored it:
curl -X DELETE http://127.0.0.1:8787/v1/credentials/anthropic/anthropic.api_key -b "$COOKIE"
```

A `DELETE` rewrites the file immediately, so a revoked credential does not come back on the next
start.

**A bad value stops the host.** `CONNECTORS_CREDENTIAL_STORE=vault` — or a store that cannot be
opened, or a path inside the checkout — is a startup error that names what would have worked. There
is deliberately **no fallback to memory**: a host that fell back would start successfully, serve
every route correctly, look exactly like a working one, and lose everything on the next restart.
That is the failure this whole arrangement exists to end, and reaching it by accident would be no
better than reaching it by design.

**The address survives the round trip whole.** A credential is stored at
`tenants/<tenant>/<authority>/<service>/<credential>`, and the file holds that address verbatim —
parsed back and re-rendered on load, so a line that is not the canonical spelling of the address it
names is a loud error rather than a second entry for one credential. Two accounts do not share a
credential, and neither do two services of one vendor.

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

## The restart, performed and labelled

Same standard as the live leg above: the thing the story is about was done to the real binary, not
only to an `App` in a test. `tests/persistence.rs` rebuilds the host in-process, which is the whole
of what a restart does to its credentials; this is the process actually stopping and starting.

**2026-07-31 — a credential wired, the process stopped, the credential still wired.**

```console
$ cargo run -p connectors-api -- --dev
connectors-api listening on http://127.0.0.1:8787
  Credentials are kept in /home/…/.local/share/connectors-api/credentials and SURVIVE A RESTART.
  They are NOT ENCRYPTED. A 0600 file mode inside a 0700 directory is the whole
  of what protects them …
  To destroy them:          rm /home/…/.local/share/connectors-api/credentials

$ COOKIE=$(curl -si -XPOST localhost:8787/auth/dev | sed -n 's/^[Ss]et-[Cc]ookie: //p' | cut -d';' -f1)
$ curl -sX PUT localhost:8787/v1/credentials/anthropic/anthropic.api_key -b "$COOKIE" \
       -d '{"value":"SENTINEL-NOT-A-REAL-SECRET-handverify-C207"}'          # 204
$ curl -s localhost:8787/v1/connectors/anthropic -b "$COOKIE" | jq '.credentials[0]|{stored,address}'
{"stored": true, "address": "tenants/dev-local/com.anthropic.api/api_key"}

$ ls -ld ~/.local/share/connectors-api ~/.local/share/connectors-api/credentials
drwx------ 2 timo timo 4096 …  /home/…/.local/share/connectors-api
-rw------- 1 timo timo  176 …  /home/…/.local/share/connectors-api/credentials

$ kill -INT <pid>
stopping; sessions are discarded and every operator must sign in again

$ cargo run -p connectors-api -- --dev            # a new process
$ curl -s localhost:8787/auth/me -b "$COOKIE"                                # 401 — the session died
$ COOKIE=$(curl -si -XPOST localhost:8787/auth/dev | sed -n 's/^[Ss]et-[Cc]ookie: //p' | cut -d';' -f1)
$ curl -s localhost:8787/v1/connectors/anthropic -b "$COOKIE" | jq '.credentials[0]|{stored,address}'
{"stored": true, "address": "tenants/dev-local/com.anthropic.api/api_key"}
```

**The session did not survive and the credential did**, which is the split this host wants. Every
served surface of the second process was then swept for the value — `/`, `/v1/connectors`,
`/v1/connectors/anthropic`, `/v1/operations/anthropic-models-list`, `/auth/me`, `/auth/status`, and
the `401`/`400` refusals — and none carried it.

And the honest half, in the same transcript: the value **is** recoverable from that file by anyone
who can read it.

```console
$ cat ~/.local/share/connectors-api/credentials
# codewandler-connector-secrets file store, v1
tenants/dev-local/com.anthropic.api/api_key 53454e54494e454c2d4e4f542d412d5245…

$ chmod 644 ~/.local/share/connectors-api/credentials && cargo run -p connectors-api -- --dev
Error: … refused access to …/credentials: its mode is 0644, and a credential store must be no
wider than 0600 — run `chmod 600 …` once you are satisfied nobody else has read it
This host does not fall back to holding credentials in memory — that would start successfully
and forget everything on the next restart. …
```

The file's mode was **still 0644** afterwards. The host refuses a widened store; it does not tighten
it, because tightening would repair the symptom and hide the exposure.

## The automated leg: one request, to a vendor under test control

`tests/live_egress.rs` (C-202) sends. A loopback HTTP server records what arrives, a shipped
operation is projected onto the host's real `Egress`, and the assertion is **equality on all four
fields** — the `{ method, url, headers, body }` the vendor received against the ones
`Operation::build_authenticated_request` built. Nothing is stubbed: the same `HttpRequestTool`, the
same `Egress`, the same pack, the same bytes.

Two things had to be reconciled to get there, and both are recorded in that file rather than worked
around:

- **The host's own SSRF guard refuses loopback**, which is where a controlled vendor has to live.
  The test passes `PrivateNetAllow::Hosts(["127.0.0.1"])` through `App::with_web_options` — a grant
  for one host on one `App`, not `PrivateNetAllow::Any`, and the shipped default is untouched. The
  same file then runs the *same* operation under `App::new` and requires it to be refused with
  nothing on the wire, so the widening is proved to be the only reason the live test can send.
- **No shipped connector can be pointed at a loopback address**, deliberately: nine carry a
  `{placeholder}`, every one templates a label inside a fixed vendor suffix
  (`{subdomain}.zendesk.com`), and C-214's `Slot` guard exists to stop a configuration value from
  moving a request to another host. So the test rewrites **one string literal** — the origin — in
  the operation's own emitted Flux, and nothing else. The method, path, module-set header, body
  encoding, credential placement and `Bearer ` prefix are all the shipped operation's.

The bound is worth stating: this proves the pack's request survives the wire intact, not that
`api.openai.com` answers it. The leg against a **real vendor** stays manual, and the section above is
where it is recorded.

What the automated tests still stop before is a **route** reaching a controlled vendor — asserting
*"the request that reached the vendor carried tenant A's credential"* end to end through
`POST /v1/operations/…/execute` needs a catalogue entry that names a loopback host, which no shipped
connector does. `tests/host.rs` covers that half up to the socket: the address a credential resolves
at, the tenant it belongs to, whether a value can reach a surface, and that the transport really is
`http.request`.

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
