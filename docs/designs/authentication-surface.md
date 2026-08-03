# Design: authentication as a connector surface — a login that cannot leak

**Status:** proposed · **Pillar:** Spec · **Stories:**
[C-134](../stories/C-134-authentication-surface-epic.md) … C-136, plus
[C-79](../stories/C-79-sensitive-response-fields.md) and [C-88](../stories/C-88-prove-oauth2.md)

## Why

Authentication is currently something the **host** does *around* a connector: `OAuth2Spec` declares
which grants a host may run, and the host runs them. There is no way to say "this vendor exposes a
login you can *trigger*", so an operator flow that needs a token has nowhere to put that step.

The ask is a triggerable one: `oauth2.login(grant: password, username: …, …)` and its sibling grants,
as declared members of a service, in an `authentication` category.

That is a reasonable and in-charter thing to want. It also introduces the single most dangerous
operation shape this repository has ever modelled, and the design is almost entirely about that.

## The danger, stated precisely

**An operation's result becomes a value in the session.** It is bound to a symbol, interpolatable
into `{{…}}`, visible to the model that called it, and eligible for a log line or an error message.
That is what an operation *is*.

A login returns a **bearer token**. So the naive shape — a normal operation that returns what the
vendor sent — hands a long-lived credential to a language model as a string, and the requirement
("these ops cannot be used to extract or see or expose those credentials") is violated by the
operation's very success.

### Redaction is not the answer, and this repo already knows why

The tempting fix is "redact it". [C-79](../stories/C-79-sensitive-response-fields.md) already records
why that is insufficient, in the concrete: Zoom's `start_url` carries a host-privileged token and
**the redactor cannot see it**.

Redaction is *string matching against values it was already told about*. A token minted **by this
very call** is, by construction, unknown to the redactor until after it has arrived — and by then it
is already in a response body that something has to handle. Redaction is a mitigation applied after
the fact. It cannot be a guarantee.

## The design: divert, never return

**A credential-producing operation does not return the credential.** Its declared output is a
**handle**:

```
oauth2.login(grant: "password", username: …, password: …)
  ->  { "credential": "tenants/<tenant>/<authority>/<service>/<name>" }
```

The token travels from the HTTP response **directly into the credential store** and never enters the
session at all. What comes back is a [`CredentialRef`](credential-addressing.md) — C-90's type, which
has had no consumer until now — naming where the value was put.

Everything downstream already works by reference: an operation that needs the token names the ref,
and the host resolves it at request-assembly time through the `CredentialStore` port
([C-116](../stories/C-116-credential-store-port.md)). A caller can **use** a credential it can never
**read**.

That is the property, and it is *structural*. There is no policy to configure, no redaction pattern
to maintain, and no code path on which the secret reaches a session symbol — because the operation's
declared output type does not contain one.

### The refusals that keep it true

Every rule here is a refusal, in this repo's tradition:

- An operation declaring `produces_credential` **must** name which field of the vendor response holds
  the secret, so the extractor knows what to divert.
- That field **must not** appear in the operation's published `response_schema` or its effective
  output. A credential-producing operation whose declared output exposes the secret field is
  **refused at load** — this is C-79's mechanism, generalised from "a field is sensitive" to "this
  operation's whole purpose is to mint one".
- A credential-producing operation must be **non-idempotent** and carry a risk that says so: minting
  a token is a write, and some vendors invalidate the previous one.
- The store is a **bound port**, never a global. An operation cannot mint a credential into a store
  the host did not supply.

### As built (C-136)

The design above is what landed, with two decisions worth recording because neither is guessable from
it.

**The fact reaches the runtime on the *credential*, not on the operation.** An author declares
`produces_credential` in the `[[operations]]` block — that is where it belongs and that is what the
loader validates — and the catalogue emitter joins it onto the credential as
`catalog::Acquisition::Minted { by, from }`. The reason is mechanical rather than conceptual: this
repository is a compiler whose output is committed, so a new field on `catalog::Operation` rewrites
all 45 generated tables, every artifact hash under them and `connectors.lock`, for a fact no shipped
connector declares. An enum variant costs nothing until something uses it. It also reads correctly on
its own axis — acquisition answers "how does stored material become the value that is placed", and
for a minted credential the answer is "one of this connector's own calls put it there".

**Nothing derived from the vendor's answer is returned, on any path through the host** — which is
more than "the success path returns a handle", and is the half that took the design work. A login
whose call fails after the token arrived cannot quote its own response, because for this one
operation shape the response *is* the credential: several vendors answer a failed grant with `200`
and a body still carrying a token for another scope, and a `401` body is where the rest put their
explanation. So the refusal carries the operation, the credential and at most the HTTP status. The
cost is real and is accepted deliberately: an operator debugging a failing login reads a status
rather than a vendor's reason, and the request — never the answer — is in the host's evidence log.

**"Through the host" is a real qualifier, and the first version of this sentence did not carry it.**
The diversion is a `connector-pack` mechanism, and this repository has a second execution surface:
the emitted `connectors/<provider>.flux` module a flux runtime lifts and runs. There is no diversion
there and there cannot be — an emitted `op` ends `response = http.request(…)` / `return response`,
and Flux holds no handle on the credential store, so a module carrying a login would perform it and
bind the raw token to a model-visible symbol. That is the thing `AGENTS.md` § Authentication contract
has forbidden since long before this epic: *"Generated Flux names a credential and nothing more. It
must not … perform session login. … Putting acquisition in Flux would expose raw tokens in
model-visible symbols."*

**So the module path is closed, not covered.** `connector-flux` refuses to emit any operation
declaring `produces_credential` (`Error::CredentialProducingOperation`), so such a connector does not
build. Three answers were available and two were rejected:

- *Teach the emitter* — forbidden by the invariant above, and unimplementable regardless: the
  diversion is a write to a bound port and Flux has none.
- *Emit into the catalogue but withhold from the module* — not available. `emit_operation` produces
  **one** rendering, and `connector-cli`'s seam feeds that same text to the module, to the
  per-operation `.flux` file and to `web/public/catalog.json`, deliberately, so that
  module-and-catalogue agreement is a property rather than a coincidence. Splitting it would publish
  a login's Flux in the public catalogue anyway, and would break the three coherence checks that make
  the split unnecessary.
- *Refuse* — what landed. It states the true thing: **this repository's execution format cannot
  express a credential-producing operation.** The declaration, the derived handle output and the
  host-side diversion all exist and are tested; what does not exist is a way to emit one.

That refusal is not a placeholder for "teach the emitter later". Read alongside the open question
below, the likely resolution is that no such **operation** should exist at all — the trigger belongs
on `OAuth2Spec`, where the host performs the acquisition and nothing is emitted, and the refusal
becomes permanent rather than temporary.

**The open question, recorded here because it decides the paragraph above.** `AGENTS.md`
§ Authentication contract says, owner-stated, that an authentication endpoint is **never** a
connector operation — and calls the credential-response rule a *"second, independent test"*. So
C-136 clears one of two gates and leaves the other standing, and this epic's `oauth2.login(…)` ask
sits on the wrong side of it. The reading argued in
[C-136](../stories/C-136-credential-diversion.md) § Open question is that the rule should stand and
the **trigger** should move to `OAuth2Spec`: the host performs the acquisition, as the contract
already requires, nothing is emitted, and the diversion built here is reused unchanged with `by`
naming a declared grant rather than an operation id. That is with the owner.

**The four withheld operations are not all unblocked by this.** babelforce's `POST /oauth/token` is
the shape this mechanism is for — subject to that open question. `zoom-meeting-get`, `zoom-meeting-create`, `postmark-server-get` and
`postmark-server-list` return a credential *incidentally*, alongside the meeting or the server that
is the operation's actual result — diverting the field would delete the answer. Those are C-79's.

### What it does *not* protect against, said plainly

The **inputs** are still inputs. `grant: password` takes a username and a password, and those are
caller-supplied values that exist in the session before the call. This design keeps the *minted
token* out of the session; it does not make a resource-owner password grant safe to hand a model.

That is an argument for preferring `client_credentials` and `authorization_code`, and for the
operator level — [connector-configuration.md](connector-configuration.md)'s `Level::Operator` — being
where a password-grant login is configured rather than a model-callable operation. **The category
should default to operator-level, and a model-triggerable login should be the deliberate exception.**

## The category

Roles land in [C-119](../stories/C-119-provider-roles-epic.md), so `authentication` is a role a
service claims, with required members per supported grant. That reuses the mechanism instead of
inventing a second one, and it means "which of my providers can mint a token, and how?" is a
catalogue query.

`OAuthGrant` already exists in `crates/connector-spec/src/auth.rs` with `Password` (babelforce's
flow) and `ClientCredentials`. [C-88](../stories/C-88-prove-oauth2.md) already records that
**`OAuth2Spec` is a landed type no shipped provider uses**, so half the configuration model is proven
only by a fixture. This epic gives it its first real consumer, and C-88 is where that lands.

## On the A2A and MCP connectors

C-495 supersedes the old repository-boundary argument: a protocol integration can be a connector,
and stdio or streaming selects a runtime rather than another repository. That does not automatically
make a duplicate implementation useful:

- **A2A**: flux already ships `crates/flux-a2a` — `client.rs`, `server.rs`, `types.rs` — plus a
  `flux-channels` adapter. It is implemented, not missing. A connector would be a second, worse copy.
- **MCP**: a JSON-RPC protocol with its own transports, discovery and session lifetime. A generic
  MCP connector may be the uniform runtime binding C-46 proposes, but it must project an existing
  tool catalogue without compiling a second, stale catalogue.

Neither belongs in this authentication epic. C-46 and the C-495 runtime program own the decision and
mechanism; nothing here is blocked on them.

## Out of scope

- **Token refresh and expiry.** Out of scope since [C-90](../stories/C-90-credential-addressing.md)
  and still is. This mints; it does not maintain.
- **Implementing a vault.** The store is a port; a production Vault implementation is
  [C-91](../stories/C-91-connector-secrets-crate.md).
- **Making a password grant safe for a model.** See above — that is a level decision, not a mechanism.
