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

Open it, pick a connector, paste a credential, run an operation.

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
| 3 — Google sign-in, accounts, sessions | next |
| 4 — OAuth2 connect flows (Google, Slack, GitHub, Notion, HubSpot) | after 3 |
| 5 — OAuth2 into provider TOML; persist secrets | after 4 |

The tenant is a parameter of every port already, and is a single constant (`local`) until slice 3
replaces it with the session's account. It was threaded through from the first commit rather than
retrofitted, because "the tenant comes from the session, never from the request" has to hold at every
call site and adding it later is how one of them gets missed.

## The charter

This crate contradicts `docs/vision.md`'s current non-goal and the loopback narrowing in
`docs/designs/connectors-app.md`. That is owner-directed, and
[C-201](../../docs/stories/C-201-charter-multi-tenant-host.md) is where the amendment and the redone
confused-deputy analysis land. Until it does, this README is the only place saying so — read
[C-200](../../docs/stories/C-200-connectors-api-epic.md) for the shape of the whole thing.
