# Design: `connectors-app` — the reference host

**Status:** accepted in principle — **the charter was amended to permit it; read §The charter change
first** · **Pillar:** Bridge · **Resolves:** [C-34](../stories/C-34-proxy-charter-decision.md) as
**yes-narrowed** · **Supersedes:** [connectors-proxy.md](connectors-proxy.md)

> Every `path:line` in this repository was read on **2026-07-31**. Citations into `../flux` were read
> at `codewandler-flux-lang` **0.39.0**. Re-grep by symbol; line numbers move.

## Why

Everything needed to execute a connector operation for real **already exists in this repository**,
and nothing assembles it.

`connector-pack` takes an operation from the catalogue, projects a `ToolSpec`, parses the operation's
own emitted Flux, evaluates a `{ method, url, headers, body }` request out of it, resolves the
credential through a bound `SecretStore`, registers every resolved value with the host's redactor,
verifies the registration actually took, places the credential per its declared scheme, and delegates
the send to flux's `http.request`. That is the whole path. It is implemented, tested, and reachable
from `pack(&["slack"], http, credentials)` in one line
(`crates/connector-pack/src/lib.rs:517-529`).

What is missing is a **caller**. Grep the whole workspace for the three things a host must build:

- `ToolRegistry::new()` appears only in `crates/connector-pack/tests/{projection,network_gate}.rs`
  and inside `#[cfg(test)]` in `crates/connector-pack/src/lib.rs` (the module gate is at `:568`).
- `ToolContext::new(...)` appears **once**, at `crates/connector-pack/tests/credentials.rs:92`.
- `flux-system` is a **dev-dependency only** — `crates/connector-pack/Cargo.toml:44-46` says so in a
  comment: *"Test-only: no shipped code here links `flux-system`."*

And the transport itself is not even in the build. `codewandler-flux-web`, which owns
`HttpRequestTool`, is **absent from `Cargo.lock`** — the lock carries `codewandler-flux-{config,
core, evidence, lang, markdown, policy, runtime, secret, skill, spec, system}` and no `-web`. So the
one concrete implementation of `Egress` this repository names in its own doc-comments
(`crates/connector-pack/src/tool.rs:20-21`) cannot be constructed from this workspace at all.

The result is a claim nobody can check. Every safety property in
[connector-tool-pack.md](connector-tool-pack.md) — the redaction ordering, the per-operation network
gate, the credential that never reaches a surface — is asserted by unit tests against stubs. **No
byte has ever left this repository towards a vendor.** A reference host is what turns those from
tested propositions into demonstrated ones.

## The charter change, and what it does not license

`docs/vision.md:72-73` listed as a non-goal:

> **A runtime.** This repo compiles; flux executes. flux-connectors ships no server, no daemon, and
> no request path of its own.

The owner has amended it to:

> **A runtime for production traffic.** flux executes connectors in anger. This repo may ship a
> reference host (`crates/connectors-app`) that proves the seams end to end — OAuth callback,
> credential store, operation execution. Loopback-bound, never published, never a production request
> path.

**This resolves [C-34](../stories/C-34-proxy-charter-decision.md) as YES-narrowed**, and C-34's
acceptance requires the reasoning be recorded. Here it is.

### Why yes

C-34's Notes named what made a proxy tempting: *"Every provider is currently blocked on the `$auth`
seam landing in another repo on someone else's schedule."* **That is no longer true, and the reason
matters.** [C-115](../stories/C-115-request-delegation.md) and
[C-116](../stories/C-116-credential-store-port.md) both landed. `connector-pack` assembles the
`Bearer ` prefix, the basic-auth pair and the query placement itself
(`crates/connector-pack/src/tool.rs:213`), which is exactly the capability the `$auth` marker was
going to add to flux. The cross-repo blocker dissolved.

So the argument for a host is no longer "we need an execution path flux cannot give us". It is the
much narrower "we have an execution path and no way to run it once".

### Why narrowed, and how the narrowing is different from a proxy

[connectors-proxy.md](connectors-proxy.md) proposed a **credential-injecting proxy**: a service that
terminates a request naming a provider and an operation, injects a credential the caller does not
hold, and forwards. Its own §"The proxy must be authenticated" states the problem it could not get
rid of — *"a credential-injecting proxy is, by construction, a confused-deputy machine: its entire
job is to add authority a caller does not have."*

A reference host is not that, and the distinction is structural rather than a matter of degree:

| | the proxy (C-34's original subject) | `connectors-app` |
|---|---|---|
| callers | anything that speaks HTTP | the operator sitting in front of it |
| credential scope | every tenant's, held for the lifetime of the service | one operator's own, in one process they started |
| deputy problem | inherent — the whole feature | absent — the caller *is* the principal |
| binding | argued about; a token was the mitigation | **loopback only, no configuration to change it** |
| distribution | a deployed service | `path`-only, never on crates.io, not in the default workspace build |
| second request path | yes — "same IR, second backend", with a conformance test to stop it drifting | **no** — it calls `connector_pack::pack`, the same code a real host calls |

The last row is the one that matters most and is the reason this supersedes rather than amends the
proxy design. The proxy would have built vendor requests *itself* from the manifest — a second
implementation of request construction, which its own §Risks calls out as the thing most likely to
drift. `connectors-app` constructs nothing. It binds ports and calls `pack`.

### The relationship to the `$auth` seam, stated

C-34's acceptance requires this explicitly, because *"shipping both without saying which is primary is
the outcome to avoid"*.

**`connector-pack`'s credential placement is primary. The `$auth` seam is not obsolete, and it is not
this repository's to land.**

They solve overlapping problems at different layers:

- **`$auth`** ([auth-seam.md](auth-seam.md), [C-10](../stories/C-10-auth-injection-and-manifest.md))
  would let an *emitted `.flux` module* name a credential and have flux resolve and place it. That
  keeps `connectors/*.flux` executable **as Flux**, by a host that has never heard of
  `connector-pack`. `AGENTS.md`'s Intentional Gaps still names it, correctly: no generated provider
  can make a live call *through the module path*.
- **`connector-pack`** makes the *catalogue* executable, in Rust, by a host that links this
  workspace. It dissolves the blocker for its own path only.

`connectors-app` exercises the **pack** path exclusively, and should say so in its own README, or it
will be read as evidence that `$auth` is no longer needed. It is: the `.flux` module remains the
human-readable contract and the artifact flux loads from `~/.flux/flows`, and it is still
unauthenticated.

## What is already unblocked, measured

An operation can execute for real when two things hold: the provider declares an `authority`, and the
base URL it resolves to carries no unbound `{template}` placeholder.

The first is not a heuristic — it is enforced. `Credentials::reference`
(`crates/connector-pack/src/credentials.rs:126-149`) renders
`tenants/<tenant>/<authority>/<credential>` and returns `Error::NoCredentialAddress` when the
connector declares no `authority`, on the recorded reasoning that *"without one the second segment of
the path does not exist, so there is nothing to look up and the only honest answer is a refusal."* A
provider without an authority cannot resolve a credential at all.

Measured against `web/public/catalog.json` today, **resolving `base_url` per service** as
`Connector::base_url_of` requires — a service's own value overrides the connector's:

| provider | services | operations | resolved base URL |
|---|---|---|---|
| `anthropic` | `models`, `admin` | 5 | `https://api.anthropic.com` |
| `datadog` | `default` | 4 | `https://api.datadoghq.com` |
| `fly` | `machines` | 9 | `https://api.machines.dev/v1` |
| `postmark` | `server`, `account` | 6 | `https://api.postmarkapp.com` |
| `slack` | `default` | 4 | `https://slack.com` |
| `vercel` | `default` | 5 | `https://api.vercel.com` |

**6 providers, 33 operations.** Exactly seven providers carry an `authority`
(`grep -lc '^authority' providers/*.toml` → 7, agreeing with
`jq '[.providers[]|select(.authority!=null)]|length'` → 7), and the seventh — `contentful` — **drops
out**, which is worth recording because it is the case a shallower check gets wrong. Contentful's
*connector-level* `base_url` is a clean `https://api.contentful.com`, but **both** its services
override it with a templated one: `https://cdn.contentful.com/spaces/{space_id}/environments/{environment_id}`
and `https://api.contentful.com/spaces/{space_id}/environments/{environment_id}`. No contentful
operation ever uses the clean value, and none can be sent without tenant configuration. Reading
`base_url` off the connector and ignoring the service override gives 7 providers / 38 operations, and
that number is wrong — it is precisely the mistake `Connector::base_url_of` exists to prevent
(`crates/connector-spec/src/ir.rs:582-586`: *"Resolve it with `Connector::base_url_of` rather than
reading this directly — the connector's value is the default."*).

33 operations across 6 vendors is more than enough to prove a seam, and the six are a good spread:
bearer and header schemes, single- and multi-service providers, one vendor (`slack`) that answers
HTTP 200 on failure and therefore exercises the `error_envelope` prose.

## Vertical slice 1 — execute one operation, no OAuth

The smallest thing that is unambiguously real. No browser, no callback, no persistence.

1. **Bind the ports.** `MemoryStore` (re-exported from `connector-pack`,
   `crates/connector-pack/src/lib.rs:128-130`) as the `SecretStore`; `Credentials::new(store,
   tenant)`; `Egress::new(http)` over a real `http.request`.
2. **Paste a token.** One `SecretStore::put` at an address the app renders and shows, so the operator
   can see what `tenants/<tenant>/<authority>/<credential>` actually looks like. Never read from a
   file, never from an environment variable that outlives the process.
3. **List what is runnable.** `catalog::providers()` and `catalog::operations_of`
   (`crates/catalog/src/lib.rs:359`, `:376`) — the same embedded catalogue every other consumer
   reads, filtered by the two conditions above.
4. **Execute.** `pack(&[provider], http, credentials)` into a `ToolRegistry`, then dispatch. Show the
   response, and show the redactor's view of it.

### The transport problem, which slice 1 must solve first

`Egress` (`crates/connector-pack/src/tool.rs:43`) takes an `Arc<dyn Tool>` and the crate deliberately
does not link a client — the doc comment at `:24-28` explains that typing it as `dyn Tool` is what
keeps *"a whole HTTP client, a DNS resolver and an SSRF guard"* out of a library whose claim is that
it opens no socket.

`connectors-app` is where that client finally has to exist. Two options, and the first is strongly
preferred:

- **Depend on `codewandler-flux-web` and construct its `HttpRequestTool`.** This is what the pack's
  own doc-comment names, it inherits flux's egress allow-list and SSRF guard, and it makes the demo
  a demo of *flux's* request path rather than of a client this repository wrote. Cost: a new
  crates.io dependency that is not currently in `Cargo.lock`, pinned in `[workspace.dependencies]`
  next to `flux-runtime`.
- **Write a minimal `Tool` over `reqwest`.** Cheaper to land, and wrong for the same reason
  `Egress`'s doc comment gives: *"A stand-in that ignores `body`, or that resolves `url` against some
  base of its own, is not a substitute — it is a different connector."* A host demonstrating the seam
  with a substitute transport demonstrates the substitute.

Take the first. If `codewandler-flux-web` is not published at a version compatible with the pinned
`flux-runtime` 0.39, that is a finding worth recording loudly rather than routing around — it means
the `Egress` seam has no shipping implementation, which is a stronger statement than "we have no
host".

**Slice 1 needs nothing from the backlog.** Not C-87, not C-89, not `$auth`. That is the point of
doing it first.

## Vertical slice 2 — the OAuth callback

Adds the half slice 1 skipped: a credential the operator does not already hold.

- **A loopback callback route.** `http://127.0.0.1:<port>/oauth/callback`, bound for the duration of
  one grant and then dropped.
- **Adapt flux's implementation; do not write a fourth.** flux already has this working three times
  over — `login_claude`, `login_codex`, `login_plugin` — and the generic path is the one to reuse:
  `flux_credentials::generate_pkce` and the S256 authorize-URL builder
  (`../flux/crates/flux-credentials/src/lib.rs:1229`, `:1357-1380`),
  `oauth_token_grant_with_client` for the form-encoded exchange (`:566-589`), and the driver at
  `../flux/crates/flux-cli/src/auth_cmd.rs:366-485` (`login_plugin`) with its listener at `:520-575`
  (`wait_for_oauth_callback` — binds `127.0.0.1:{port}`, verifies the CSRF `state`, answers the
  browser with a completion page). A fourth implementation of PKCE in a repository whose sibling has
  three is how the two drift on a security-relevant detail.
- **Then `SecretStore::put`**, at the same address slice 1 pasted into, so the two paths converge on
  one store rather than growing two.

### What slice 2 is blocked on, honestly

Two backlog items, and neither is optional:

- **[C-87](../stories/C-87-configuration-codegen.md)** — the configuration surface reaches no
  artifact ([connector-surfaces.md](connector-surfaces.md) measures this), so the app cannot render
  a connect form at all. Worse for this purpose specifically: C-87's own acceptance records that
  `crates/connector-cli/src/site.rs` *"collapses the entire `OAuth2Spec` to `oauth2: bool`,
  discarding `scopes`, `grants`, `authorize_path`, `token_path`, `client_id` and `redirect`"*. **A
  host cannot build an authorize URL from the published catalogue today.** It would have to parse
  provider TOML, which is the thing the catalogue exists to make unnecessary.
- **[C-89](../stories/C-89-hosted-oauth-redirect.md)** — `OAuthRedirect { port, path }` describes
  flux's CLI login and nothing else. For a loopback app that is *almost* sufficient, which is a trap:
  slice 2 would work, and would prove only the loopback case, exactly as C-89's Notes warn about
  C-88. The app should therefore state in its output which case it demonstrated.

## The fences, and why a new crate trips neither

This is the section to read before writing a `Cargo.toml`.

Two tests guard "generation is explicit, committed, deterministic, and offline". **Both are scoped
narrowly enough that `crates/connectors-app` linking `reqwest` would pass them silently.**

- `crates/connector-cli/tests/no_network.rs:43-86` (`the_network_seam_is_the_only_door`) is a
  **source audit over one directory**. It reads `env!("CARGO_MANIFEST_DIR")/src` — that is
  `crates/connector-cli/src` and nothing else — and greps eight needles (`std::net`, `TcpStream`,
  `TcpListener`, `UdpSocket`, `reqwest`, `ureq`, `hyper`, `curl`), skipping `net.rs` and comments. A
  socket in a different crate is invisible to it, which its own module doc concedes when it says the
  invariant *"is therefore local to this crate"*.
- `crates/connector-cli/tests/dependency_fence.rs:22-27` is a **dependency-graph walk over a
  hard-coded list of four**: `connector-cli`, `connector-spec`, `connector-flux`,
  `connector-catalog`. It asserts none of them reaches `connector-secrets`. A fifth crate is simply
  not asked about.

So today the fence's guarantee is *"the compiler is offline"*, and it is a true and valuable
statement. Adding a network-capable crate does not weaken it. What it does is make the fence's
**silence** ambiguous: a reader who finds `reqwest` in `Cargo.lock` and a passing
`dependency_fence.rs` cannot tell whether that edge was considered or merely unexamined.

### The extension: allow the exception, visibly

Extend `dependency_fence.rs` with a second assertion and one new constant. Do **not** touch
`COMPILER_CRATES`.

- **`connectors-app` becomes an explicitly allowed network crate.** A named constant (something like
  `NETWORK_CRATES: &[&str] = &["connectors-app"]`) with a doc comment stating *why* the exception
  exists and what bounds it. The value of writing the exception down is that it converts an absence
  of evidence into a recorded decision — the same move `Egress`'s "the choice must be *stated*"
  comment makes for the transport.
- **The four compiler crates stay fenced exactly as they are**, against `connector-secrets` and now
  also against every name in `NETWORK_CRATES`. That second edge is the one that actually earns its
  keep: it makes `connector-cli -> connectors-app` a test failure, so the app can never become a
  dependency of the compiler by accident. The existing walk already reports the offending chain
  (`Lock::path_to`, and `the_walk_finds_an_edge_that_is_not_direct` proves the walk sees transitive
  edges), so the machinery is there.
- **A new crate that is neither a compiler crate nor on the allow-list should fail the test.** That
  is the assertion that makes the list load-bearing rather than decorative; without it, the next
  network-capable crate is again merely unexamined.

Two further constraints on the crate itself, which belong in its manifest rather than in a test:

- **`publish = false`.** The charter amendment says "never published"; a manifest key is how that is
  enforced rather than remembered.
- **It must not be a dependency of anything**, in this workspace or outside it. `connector-catalog`'s
  ownership row in `AGENTS.md` already establishes the shape of a crate that is a leaf; this one is a
  leaf too, at the other end of the graph.

### Loopback is a property of the code, not of a flag

[C-145](../stories/C-145-dry-run-transport.md)'s acceptance sets the standard this should follow:
*"It is structurally incapable of sending… a flag on a live client is something a caller forgets."*
The same reasoning applies to the bind address. `connectors-app` should have **no configuration
surface for its listen address at all** — not a flag with a loopback default, not an environment
variable. flux's own HTTP server refuses a non-loopback bind without a token, and it needs that
escape hatch because it is a real server. This is not, so it should not have one.

## The boundary with C-147, and the seam they share

[C-147](../stories/C-147-explorer-runs-an-operation.md) — "The explorer runs an operation" — looks
like the same feature and is **the inverse artifact by design**. The two must never merge, and
someone will propose it, because both end up rendering a request next to a response.

The reason is that `web/` is a **public GitHub Pages site**. C-147's acceptance is explicit and
non-negotiable on two points:

> - **It is unmistakably not a live call.** A reader must not come away believing the site called the
>   vendor. Label it, and do not use language like "sent" or "succeeded".
> - **No credential is ever collected.** No input field asks for a token, and the page cannot be made
>   to hold one.

`connectors-app` is the exact opposite on both: it *does* call the vendor, it *must* say "sent", and
collecting a credential is the point. Its safety comes from being loopback-only and unpublished —
which is a property the explorer structurally cannot have, and vice versa.

**The shared seam is [C-145](../stories/C-145-dry-run-transport.md)'s `Transport`.** C-145 already
defines it as one mechanism with two payloads, and `Egress`'s doc comment already anticipates the
substitution: *"it is the seam a non-vendor transport plugs into: a dry-run that renders the request
instead of sending it, or a recorded fixture, without either forking the request path."*

So the division is:

| | request construction | transport | credential |
|---|---|---|---|
| `connectors-app` | `connector_pack::request` | live `Egress` over `http.request` | resolved from a bound `SecretStore` |
| the explorer (C-147) | the same construction, rendered | **dry-run** — structurally cannot send | never collected; references only, per C-145's acceptance |

**Neither forks the request path**, which is the property worth defending. C-145's differential test
— *"for every shipped operation, the dry-run request and the emitted `.flux` module's request agree
on method, URL and body shape"* — is what keeps them honest, and it covers all 242 operations offline
without a vendor account.

One consequence to state now, because it will come up: **if the app and the explorer ever disagree
about a request, C-145's differential is the arbiter, not either surface.** They are two renderings
of one construction, and a third opinion about what a request should be is the drift
[C-117](../stories/C-117-pack-codegen.md) exists to catch.

## Risks, named

- **It becomes the primary execution path.** C-34's Notes called this "the quiet risk", and the
  narrowing is the mitigation rather than a promise: unpublished, loopback-only, no configuration to
  change either. Those are structural, and they should stay structural. The first PR that adds a
  `--bind` flag is the one to refuse.
- **It is the first component here that holds a plaintext credential at runtime.** That was true of
  the proxy too, and it remains a categorical change from "every artifact in this repo is inert
  text". What is different is the blast radius: one operator's own credential, in a process they
  started, reachable from nowhere else. `MemoryStore` rather than a file is the right default for
  exactly this reason — the process exiting is the cleanup.
- **A demo that is not run is worse than no demo.** A reference host proves a seam on the day someone
  runs it. Without a CI job it decays into a crate that compiles, which is the state the seam is
  already in. What CI can actually assert offline — that the app builds, binds, registers the
  catalogue and constructs the right request under C-145's dry-run transport — is worth wiring on day
  one; the live leg is manual and should be labelled manual.
  [C-149](../stories/C-149-vault-live-leg-reports-ok-when-it-skips.md) is the cautionary precedent: a
  live leg that reports OK when it skips.
- **`$auth` looks solved and is not.** Stated above; it belongs in the crate's own README too.

## Out of scope

- **Multi-tenancy.** One tenant, bound at construction, exactly as `Credentials::new` takes it.
- **Token refresh.** Out of scope since C-90 and still is
  (`crates/connector-pack/src/credentials.rs:49-52`): *"No cache, no expiry, no refresh."*
- **Serving anything to anyone else.** No API, no forwarding, no second caller. The moment a second
  principal can reach it, it is the proxy [connectors-proxy.md](connectors-proxy.md) describes, and
  that design's confused-deputy analysis applies again in full.
- **Channels and inbound delivery.** The pack's operation path is what this proves.
  [C-118](../stories/C-118-connector-channel-adapter.md) is the second surface and has its own host
  requirements.
