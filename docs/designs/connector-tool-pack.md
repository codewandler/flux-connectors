# Design: the connector Tool pack — the flux interop layer

**Status:** approved · **Pillar:** Bridge · **Stories:** [C-113](../stories/C-113-tool-pack-epic.md) … C-118

> This design describes a new crate **in this repository** plus a small set of stories on **`../flux`**'s
> board. Every `path:line` below was read in `/home/timo/projects/flux` at `codewandler-flux-lang`
> **0.39.0**. Symbol names are stable and line numbers are not — re-grep by symbol rather than
> trusting a number that does not land.

## Why

This repo compiles vendor specs into Flux modules and a catalogue, and **nothing consumes them at
runtime**. `install` is unimplemented (C-15), so the only route into flux is a human copying `.flux`
files into `~/.flux/flows`.

flux is blocked on this by name. Release 0.38 **removed** `flux-plugin-zendesk` before its first
release, "to be superseded by a flux-connectors interop layer". `D-200`, `D-201` and `D-202` are
`blocked`, and `A-136`'s reference flow is retained-but-unrunnable, all waiting on that layer.
`examples/zendesk.triage.flux` is kept deliberately as *"the authored shape the replacement has to
satisfy"* — a written acceptance target. It calls `zendesk.test`, `zendesk.ticket.show`,
`zendesk.ticket.search` and `zendesk.ticket.comment.list`.

## The runtime already exists, and it is not ours

`flux_sdk::ClientBuilder` (`crates/flux-sdk/src/lib.rs:371`) already is the runtime-construction API,
with the ports and configuration bound at build time:

| concern | what flux already offers |
|---|---|
| bind ports/adapters | `approver(Arc<dyn Approver>)` · `with_authorization` · `with_redactor` · `storage` · `with_sandbox` · `try_with_live_datasource` |
| register operations | `register_pack(FnOnce(&mut ToolRegistry))` · `try_register_pack` · `register_op_from(source, tool)` · `with_plugin_tools_from` |
| configuration | `max_tokens` · `max_iterations` · `context_budget(bytes)` · `with_compaction` · `max_calls` · `allow`/`deny` |

`ToolRegistry::try_register_all_from` (`crates/flux-runtime/src/lib.rs`) installs a pack atomically
under one auditable source label: if any declaration is invalid or collides, none of the pack lands.

So **this repo builds no runtime.** It supplies a pack; flux constructs and runs it. `vision.md`'s
non-goal stands unamended — *"This repo compiles; flux executes. flux-connectors ships no server, no
daemon, and no request path of its own."*

## Why a Tool pack rather than composite `.flux` text

### The naming asymmetry is decisive

`crates/connector-flux/tests/op_emitter.rs` already asserts that **a dotted op *declaration* name
does not parse in flux-lang**, which is why this repo emits `zendesk-ticket-show`. But every flux
**tool** is dotted — `http.request` (`crates/flux-web/src/http.rs:83`), `command.invoke`,
`op.register`, `skill.load`.

flux's reference flow calls `zendesk.ticket.show`. It was written against a *tool* surface, and only
a tool surface can spell it.

### The safety argument is stronger

`ToolSpec` (`crates/flux-spec/src/lib.rs:289`) carries `name`, `description`, `input_schema`,
`output_schema`, `effects`, `risk`, `idempotency`, `access` and `group` — and this repo's IR already
holds every one of those, per operation.

As a composite, an operation inherits whatever gating `http.request` happens to get. As a Tool, each
operation is gated **individually** by flux's permission and approval envelope, at the risk level the
connector author declared. That is a capability the composite path cannot have.

## This dissolves the `$auth` blocker

[auth-seam.md](auth-seam.md) and C-26 exist because flux's `{"$secret": "ENV"}` marker is
*whole-value, headers-only, no prefix, no encode* — so `Bearer <token>` and basic-auth base64 cannot
be expressed, which blocks every provider from making a live call.

**A Tool builds its own header value in Rust.** The prefix and the base64 happen here, before
`http.request` ever sees the request, so the marker never needs to grow those capabilities. The
secret is kept off every surface with `ctx.redactor.add_secret(...)` — `pub redactor: Redactor` at
`crates/flux-runtime/src/lib.rs:1226` — which is exactly what `flux-web` does at
`crates/flux-web/src/http.rs:248`.

The seam design is not deleted; a composite-based connector still wants it. But **milestone 1 no
longer waits on a flux release**, and C-26's 11 paste-ready drafts should not be filed as written.

## The shape

```rust
let client = flux_sdk::Client::builder()
    .try_register_pack(connector_pack::pack(&["zendesk", "slack"]))
    .build()?;
```

Each Tool is thin and holds no transport of its own:

```rust
impl Tool for Operation {
    fn spec(&self) -> ToolSpec { /* projected from the catalogue entry */ }

    // MUST mirror http.request's own gate — see below.
    fn permission_subjects(&self, params: &Value) -> Vec<String> { vec![self.url(params)] }
    fn intents(&self, params: &Value) -> IntentSet { /* NetworkFetch */ }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> Result<ToolResult> {
        let request = self.build_request(ctx, params)?;   // url, method, headers, body
        self.http.execute(ctx, request).await             // flux owns egress
    }
}
```

`flux_web::http::HttpRequestTool` is public (`crates/flux-web/src/http.rs:38`), so delegation is a
plain method call passing the **same** `ctx`.

### Two safety must-dos, or delegation silently loses a gate

**1 · Mirror the network gate.** `HttpRequestTool::permission_subjects` returns the request URL
(`crates/flux-web/src/http.rs:118`) and `intents` raises `NetworkFetch` (`:126`). Calling `execute`
directly **bypasses `Executor::dispatch`**, so neither is ever consulted for the inner call. The
generated Tool must therefore declare the same subject and intent itself.

The connector manifest's `http_hosts` (C-10) is exactly the declared data for this. A test must
assert that every generated Tool's `permission_subjects` is non-empty and names the vendor host —
without it, installing a connector silently becomes a hole through the host's network policy. This is
the single most dangerous way this design can be implemented wrongly while appearing to work.

**2 · Register the secret with the redactor before the request is built**, not after, so a failure
between construction and dispatch cannot surface it in an error.

## The redaction guarantee, and the condition it turned out to have

**Decision (C-152): a credential the host's redactor will not hold is refused at resolve time and not
sent.** C-116 implemented must-do 2 and stated the resulting property unconditionally — "the credential
never reaches a surface". The property had a condition nobody had written down.

`Redactor::add_secret` **silently ignores a value under six characters once trimmed**
(`codewandler-flux-secret-1.0.1/src/lib.rs:195-201`, the version pinned in `Cargo.lock`):

```rust
pub fn add_secret(&self, value: impl Into<String>) {
    let v = value.into();
    let trimmed = v.trim();
    if trimmed.len() >= 6 {
        self.values.lock().unwrap().push(trimmed.to_string());
    }
}
```

That no-op is right for flux — over-redacting a common English word would corrupt every surface it
touches — and it means a caller reading `add_secret` as a guarantee gets one only above the threshold.
A five-character stored credential was registered *successfully* and travelled unredacted through all
four surfaces `Executor::dispatch` scrubs. **The code was correct about what it did; the prose was
wrong about what that meant, and the prose is what a reader relies on.**

Two ways out were on the table:

1. **State the threshold everywhere the guarantee is stated** and accept it.
2. **Refuse** a credential too short to be redactable, naming the address.

Refusing won, on three grounds:

- **It is the posture the rest of this port already takes.** No credential stored, a store that cannot
  answer, an inbound signing secret, a header the module already set — every one of them refuses and
  none degrades, precisely because an unauthenticated or unprotected send is the failure that looks
  like success. A credential the host cannot keep off a surface belongs in the same list.
- **Option 1 does not remove the leak, it documents it** — and it puts the burden on every future
  reader of four separate prose sites to notice a numeric threshold and reason about whether their
  value clears it. The value still travels in the clear.
- **A five-character API token is a misconfiguration long before it is a credential.** No vendor in
  this catalogue issues one; a store holding one is far more likely to hold a truncated value, a
  placeholder, or an empty string with a newline.

The refusal is `connector_pack::Error::UnredactableCredential`, raised where the value is registered
(`crates/connector-pack/src/credentials.rs`, `register`). It names the operation, the credential and
the address's tenant and authority — **and neither the value nor its length**, because a length is a
fingerprint, which is the care `connector_secrets::Secret`'s `Debug` already takes.

### It asks the redactor rather than mirroring the six

The check is not `value.trim().len() < 6`. The value is registered and the redactor is then asked
whether scrubbing it changes it; if it does not, nothing is protecting it. A mirrored threshold would
be a constant that goes stale on a flux upgrade **without any test noticing** — the same failure mode
`DEFAULT_SERVICE`'s mirror is guarded against, and there the guard was worth a test of its own. Asking
also covers the empty and all-whitespace cases without enumerating them.

### The named consequence

This is a behaviour change, not a tightening of an internal invariant. A deployment whose store holds
a credential under the threshold used to get a `200` from the vendor and a leak; it now gets a refusal
that names the address, and no request. That is the intended trade, and it is the one case where this
pack refuses a call the vendor would have answered.

### Also from that review

- **`auth::Assembled` no longer derives `Debug`.** It holds the assembled plaintext — `Bearer `'s
  token, or the base64 of a basic pair — and a derive there is a foot-gun waiting for the first `{:?}`
  added while debugging. Its hand-written `Debug` redacts the value and keeps the credential name and
  placement, matching `Secret`.
- **Registration moved above the fallible step.** The user half of a Basic join is a fallible
  environment read that used to run *between* the store returning a value and the redactor being told
  about it. Nothing in that window could surface the value, and C-116's own acceptance was
  "`add_secret` before the request is constructed" — so the window is closed rather than argued about.
- **The `view` surface is now asserted against a real `view`.** `flux_runtime::tool_fn` builds every
  result with `ToolResult::ok`, which leaves `view: None`, so the test's stand-in transport has to
  answer with `ToolResult::ok_view` for that assertion to scrub anything at all.

## What travels is not always what was resolved (C-159)

An independent review of C-152 found two more of the same class *while* C-152 was closing the
smaller version of it. Both are closed here.

### The registered string and the travelling string had diverged

**Decision: register every *form* of a credential that is not recoverable from a form the redactor
already holds, and let one function decide which forms those are.**

`register`'s own documentation said *"every value this pack puts on a request goes through here"*.
That was true of the door and false of the bytes. `auth::place` percent-encodes a
`Placement::Query` credential onto the URL, and `+`, `/` and `=` do not survive that — which is the
alphabet a base64 credential is made of, so the one case that diverged is the one that matters. For
a query-placed base64 token the string on the wire shared **no substring** with the string the
redactor had been told about, and all four surfaces `Executor::dispatch` scrubs rendered it in the
clear. It is the same class of overclaim C-152 exists to remove, introduced by the sentence that
closed it.

Three answers were open, and the story named all three: register the encoded form as well, refuse
query placement until something did, or restate the claim precisely. **Registering won**, on the
same three grounds C-152's own refusal won on, read the other way round:

- **It is what the rest of the port already does.** `base64(user:secret)` contains neither half, so
  C-116 already registered the assembled value on its own terms. A percent-encoded value is the same
  situation with a different transform, and answering it differently would be an inconsistency a
  reader has to hold in their head.
- **Refusing would refuse a placement the IR models and the loader accepts**, over a defect in this
  crate rather than in the connector. The catalogue happens to declare zero query placements today;
  that is a fact about which vendors have been described so far, not a decision to drop the axis.
- **Restating would document the leak rather than remove it** — C-152's own reason for rejecting
  option 1, and it applies unchanged.

The generalisation that covers all three transforms is C-184's rule, stated once and now enforced in
one place: **the redactor holds every form that is not recoverable from a form it already holds.**
Acquisition can transform (`base64`); placement can *surround* (a header prefix, which leaves the
credential verbatim inside the header and needs nothing extra — registering `SSWS ` would scrub a
public word out of unrelated prose and leave the bare token unheld) or *transform* (query encoding).
`auth::placed_form` is the single answer to which of the two a placement does; `auth::place` writes
what it returns and `credentials::resolve_mechanism` registers it, so the two cannot be derived
apart. Its match over `Placement` is exhaustive, so a placement added later has to state its answer
rather than inherit `no` by omission.

**Unreachable today, and that is why it is worth a test.** The committed catalogue is 18
`Placement::Header` and 2 `Placement::Inbound`. A fail-closed path with no shipped consumer gets no
accidental coverage, so `credentials.rs`'s test doctors the shipped slack provider's placement and
drives the real resolve path — the same technique, and the same justification, as the
authority-less-provider test one file over.

### `Request`'s derived `Debug` was the larger of the two plaintext exposures

**Decision: `Request` prints its shape and none of its values.**

C-152 hand-wrote a redacting `Debug` for `auth::Assembled` because a derive there was a foot-gun
waiting for the first `{:?}` added while debugging. The reviewer's observation is that `Assembled` is
built at one internal site and never escapes, while **`Request` is `pub`**, carries the assembled
credential in a header value and — for a query placement — in its URL, and is something a host can
hold and format. Nothing formats it today; that was equally true of `Assembled`.

What prints is the method, the host, the path, the header *names* and the query-parameter *names*.
What does not is every value, including the body — which prints as present or absent and never as
content or as a length, because a length is a fingerprint. There is deliberately **no allow-list of
safe header names**: a request cannot know which header holds the credential, and such a list rots
into a leak the first time a vendor puts a token somewhere new.

### Registration is idempotent, and it is verified rather than remembered

`Redactor::add_secret` pushes onto a `Vec` and dedupes nothing, while `redact` walks that set for
every scrub — so a long-lived host resolving one credential per call grew the set by an entry per
call. The reviewer measured the cost rather than guessing (1.6µs at one value, 23ms at 100k) and
judged it not to be C-152's problem; it is this story's.

The story framed it as a memo keyed on `(CredentialRef, value)` within a pack. **It is implemented
one step stronger and in the other direction: the redactor in hand is asked, every time, and told
only what it does not already hold.** A memo on this side would be a memory of some *earlier*
redactor — `ExecutionEnvironment::new` constructs one per environment, and whether that is per turn
or per process is decided by a binding in flux rather than here — so a remembered registration
against a redactor that never received the value is exactly a credential travelling unheld. Asking
also dedupes across two credentials that happen to hold one value, which a key on the address cannot.

The question has to be asked precisely, and `credentials::holds` is where that lives. `redact` runs
two passes — exact substrings from the registered set, then credential-*shaped* tokens (`sk-ant-…`,
`xoxb-…`) it was never told about — so `redact(value) != value` answers "yes" for a value nobody
registered, and a caller deciding whether to register from it would skip precisely the tokens that
look most like credentials. Gluing a non-boundary byte to the front leaves the shape pass
inapplicable and the substring pass untouched, so what remains is the question actually being asked.
`a_token_shape_the_redactor_scrubs_is_not_a_registration` pins it.

**And that answer has a condition, stated rather than assumed** — which is the discipline C-152
exists to enforce. flux-secret exposes no membership test, so what is observable is *coverage*:
`holds` is true when a registered value is a substring of the probed one, which is wider than "this
value is registered". The two differ only when a **proper** substring of the value is registered and
the rest of it is not, and there a skipped registration would leave the surrounding fragment
rendering in the clear. The three forms this port registers cannot stand in that relation — the
stored value `S`, `base64(user:S)` which does not contain `S`, and a percent-encoding which either
equals its input or escapes a character out of it — and the one containment that does occur in
practice, a trailing newline, is the case where skipping is *correct*, because `add_secret` stores
values trimmed and both spellings are one entry either way. What is left over is a store holding a
truncated copy of its own credential under a second address.

The registered set's size is not observable through `flux-secret` 1.0.1 — `values` is private and
there is no count — so `tests/credentials.rs` observes it through the one thing that does leak it:
`redact` replaces *each* copy in turn and its replacement text contains the word `redacted`, so a
duplicate nests the marker. The expectation is measured from a control redactor rather than written
out, and the test asserts that the probe can distinguish one registration from two before it asserts
anything else — if a future flux makes duplicates invisible, it says so rather than quietly holding.

## Ports the host binds

- **`CredentialStore`** — the adapter this repo already modelled and never wired to anything.
  `crates/connector-spec/src/credential.rs` holds `CredentialRef`, the `Layout` trait and
  `TenantLayout` (C-90). Managing expiring tokens was out of scope there and remains so.
- **`HttpRequestTool`** — injected rather than constructed, so a host can supply a pre-configured one.

## Channels

`flux-channels` has a `Channel` trait (`crates/flux-channels/src/channel.rs:16`) and adapters `a2a`,
`schedule`, `slack`, `webhook`. [C-82](../stories/C-82-channel-bindings-epic.md) already recorded that
flux's dispatch is a closed match with one arm per vendor, and that **its slack arm hand-builds a
`chat.postMessage` this repo already compiles**. A connector-backed `Channel` adapter is the second
surface; it lands after operations, depends on the same credential port, and has a far smaller
consumer.

## Configuration — a correction worth stating plainly

`context_budget(bytes)`, `max_iterations`, `max_tokens`, `max_calls` and
`max_inflight_per_principal` exist in `flux-config`. **There is no max-memory knob and no general
concurrency limit** — the only concurrency control is server-side per-principal. Those are flux-side
stories, not ours, and nothing in this design depends on them.

## The risk to name now

Two surfaces are generated from one IR — the `.flux` module and the Tool pack — and **they can drift
into disagreeing about the same operation**. `AGENTS.md` already warns about exactly this for the
C-12/C-95 shared lowering.

Both must be generated from the same IR in one build and covered by the existing fixed-point gate. A
**differential test** asserting that the pack's constructed request and the module's emitted request
agree is the honest guard. It belongs in C-117, not in a later postmortem.

## Out of scope

- **A runtime, a server, or a request path here.** flux constructs and runs; this crate is a pack.
- **Refreshing expiring tokens** — out of scope since C-90 and still is.
- **Composite emission going away.** `connectors/*.flux` keeps shipping. The pack is an additional
  surface, not a replacement, and the `.flux` artifact remains the human-readable contract.
