---
id: C-144
title: "No connector can send a non-JSON request body"
pillar: Spec
status: done
areas: [connector-spec, connector-flux]
note: "found shipping Stripe: op.rs binds application/json unconditionally and the IR has no content_type key anywhere. Blocks every form-encoded vendor — and OAuth2 token endpoints are form-encoded BY SPEC, so C-135's oauth2.login needs this too"
---

# No connector can send a non-JSON request body

## Goal

Let an operation declare how its body is encoded, so a connector can address a vendor that does not
accept JSON.

## What was measured

`crates/connector-flux/src/op.rs:144` and `:553` bind one media type unconditionally:

```rust
const JSON_MEDIA_TYPE: &str = "application/json";
```

and there is **no `content_type` / `encoding` / `body_format` key** on the provider, the service, the
operation or the `ParamSet`. So an operation declaring a body field always emits
`headers: { "content-type": "application/json" }` with a JSON document.

Stripe parses **only** `application/x-www-form-urlencoded`. So any Stripe operation with a body field
would send a document Stripe does not parse and get `400 Missing required param`.

## Why this is bigger than Stripe

C-106 worked around it with a selection rule — *an operation ships only if it addresses everything it
needs in the path* — which is why its refund is the legacy charge-nested
`POST /v1/charges/{charge}/refunds` rather than the canonical `POST /v1/refunds`, and why capture and
refund are full-amount only. That is a real fidelity loss, taken deliberately, on one connector.

It will recur on every form-encoded vendor: **Twilio, Mailgun, PayPal classic**, and others in the
fleet stories.

**And it blocks the authentication epic.** OAuth2 token endpoints are `application/x-www-form-urlencoded`
**by specification** (RFC 6749 §4.3.2 and friends). So
[C-135](C-135-authentication-role.md)'s `oauth2.login(grant: password, …)` cannot be emitted at all
until this lands. That dependency is not recorded in C-135 today and should be.

## Acceptance

- [x] An operation can declare its request-body encoding. The set is **closed** — at minimum `json`
      and `form` — because an open string is a media type nobody validates, and a typo would ship a
      body the vendor silently ignores.
      → `connector_spec::BodyEncoding` (`crates/connector-spec/src/ir.rs`), authored as
      `params.body_encoding`; `tests/ir_roundtrip.rs::body_encoding_is_closed_and_its_default_is_invisible`
      and `tests/body_encoding.rs::an_unknown_encoding_does_not_load`.
- [x] `json` stays the default, so no shipped provider's emitted module changes. **A test asserts
      every existing module is byte-identical** across this story.
      → `crates/connector-flux/tests/shipped_modules.rs::every_shipped_operation_is_byte_identical_to_its_committed_rendering`,
      plus `flux-connectors build` reporting `19 providers, 256 artifacts up to date; nothing written`.
- [x] The emitter encodes a `form` body as `application/x-www-form-urlencoded`, with the
      `content-type` header to match, and nesting refused rather than flattened — form encoding has
      no agreed nesting convention, and picking one silently is how a vendor receives a field it does
      not recognise.
      → `op::form_payload` and `op::check_body_encoding`; `Error::UnencodableFormField`. **Values are
      interpolated verbatim** — flux has no form encoder, see the Progress note.
- [x] **Failing-first test:** `a_form_encoded_operation_emits_a_form_body_not_json` — must fail
      today, where the media type is a constant.
      → `crates/connector-flux/tests/body_encoding.rs`.
- [x] Generated Flux still parses, analyzes and is a fixed point of flux's own formatter.
      → `tests/body_encoding.rs::a_form_body_parses_analyzes_and_is_canonical`, and the whole shipped
      set through `shipped_modules.rs`.
- [ ] `AGENTS.md`'s *Intentional gaps* list drops the entry this story closes.
      → **There is no such entry to drop.** The form-body gap was recorded in
      `providers/stripe.toml`, in C-106 and in this story, but never in `AGENTS.md`. Rather than leave
      that list untrue in the other direction, the entry it *does* own — unencoded query values — now
      also names the residual form-body gap, and the refusals list names the three new refusals.

## Notes

- **Coordinate with [C-135](C-135-authentication-role.md)**, which needs this. Whichever is scheduled
  first, C-135 should not start believing it can emit a token grant.
- Consider whether the encoding belongs on the operation or the service. A vendor is usually
  consistent, but Stripe's API is form-encoded while its *webhook* payloads are JSON — so the axis
  is per-request, not per-vendor, and the declaration should sit where that is expressible.
- `http.request`'s body argument is read with `Value::as_str`
  (`../flux/crates/flux-web/src/http.rs`), so the encoded form must reach it as **text**, the same
  way `parse($body, as: "json")` already canonicalises a record. Check what the equivalent is for a
  form body before assuming one exists — if flux has no form encoder, that is a flux-side story and
  the finding belongs on their board.
- Do not fix this by letting a provider write a raw `content-type` header. That would let a connector
  claim an encoding the emitter does not actually produce, which is worse than the current honest
  limitation.

## Progress

**Landed on `impl/C-144`.** The axis is `ParamSet::body_encoding`, a closed
`BodyEncoding { Json, Form }` whose default is skipped in every serialization — which is why the
lockfile hash domain, every manifest, the published catalogue and all 256 artifacts are unchanged.

It sits on `ParamSet` rather than on `Operation` for two reasons, one principled and one mechanical:
it describes exactly what `params.body` and `params.body_schema` declare, and `Operation` is
constructed with exhaustive struct literals in `connector-cli` and `connector-pack`, which this story
may not edit — `ParamSet` is always built with `..ParamSet::default()`.

**flux has no form encoder, and this was measured rather than assumed.** Under flux-lang 0.39 the only
node that turns a record into text is `parse(x, as: "json")`, and the analyzer restricts `as_type` to
`f64`/`i64`/`bool`/`json`/`string` (`../flux/crates/flux-lang/src/analyze.rs:1809-1815`) — so
`as: "form"` does not merely fail at runtime, it fails analysis. There is no `encode`/`stringify`/
`serialize` node, no `expr` function that escapes anything, no core-catalogue op that percent-encodes,
and `http.request` does not serialize a record for you: it reads `body` with `Value::as_str` and
forwards the bytes verbatim. So the emitter assembles the pairs with `fmt`, exactly as the query string
is assembled — required and `const`-pinned fields in the opening template, optional ones appended under
`when` guards so an unsupplied field cannot travel as the literal text `note=null`.

**The residual gap, and it is real: form values are interpolated verbatim.** A value carrying `&` or
`=` corrupts the body and can inject a field. This is the same class as the recorded query-encoding
gap, now in a second request position; it is recorded in `AGENTS.md` and in `op::form_payload`, and the
fix is a **flux-side** encoder that belongs on flux's board next to the structured-`query` handoff in
[../designs/query-encoding-flux-stories.md](../designs/query-encoding-flux-stories.md). That story is
not written yet — a resuming agent should write it before any provider is switched to `form`.

**Three shapes are refused rather than emitted**, each because the alternative is a request a vendor
answers `200` to: a nested field under `form` (a dotted `wire`, a braced wire name, or a declared
`object`/`array` value), a free-form `body_schema` under `form`, and a `body_encoding` on an operation
that sends no body at all.

**Not done here, deliberately:** no `providers/*.toml` was touched, so no shipped connector uses `form`
yet. Stripe's full-amount-only capture and refund, and its charge-nested refund path, stay as C-106
selected them — switching them over is a provider story, and it should wait for the flux-side encoder
above. [C-135](C-135-authentication-role.md)'s recorded dependency on this story is satisfied on the
declaration side: a token grant can now be declared as `form`, with the same value-encoding caveat.

### Coordinator notes at integration

- **The one unchecked box was my error, not a shortfall.** I wrote an acceptance item saying
  `AGENTS.md`'s *Intentional gaps* list should drop the entry this closes. **There was no such entry** —
  the form-body gap lived in `providers/stripe.toml`, in C-106 and in this story, never in `AGENTS.md`.
  The implementor did the right thing: rather than leave that list untrue in the other direction, it made
  the entry `AGENTS.md` *does* own name the residual gap.

- **The flux-side encoder is committed** at `c5c69fed` in `../flux` — `parse($record, as: "form")`, with
  its own story `L-101`, four tested wire decisions, docs and changelog. I verified flux's gate myself
  before committing to another repository's `main` rather than taking the report's word for it.

  **It does not help this repository yet.** `codewandler-flux-lang` is pinned to a crates.io release and
  must stay one (C-1's reasoning), so the encoder arrives only after flux publishes. Until then form
  values are interpolated verbatim.

- **Follow-up owed before any provider is switched to `form`:** the residual gap means a value carrying
  `&` or `=` corrupts the body and can inject a field. That is the same class as the pinned
  query-encoding gap, now in a second request position. Nothing ships as `form` today, so nothing is
  exposed — but the provider story that switches Stripe to the canonical `POST /v1/refunds` must wait for
  a published flux-lang, not merely for this axis.
