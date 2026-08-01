---
id: C-430
title: "No operation returns a secret — and three shipped in v0.9.0 that do"
pillar: Spec
status: done
areas: [providers, connector-spec, connector-cli]
note: "owner-stated 2026-08-01. A SECOND, deeper scan found worse than the first: postmark-server-get and -list return `ApiTokens` — the server's live tokens IN PLAINTEXT, by the vendor's own description. With zoom's start_url that is four operations live in v0.9.0. babelforce's was withheld by C-426"
---

# No operation returns a secret — and three shipped in v0.9.0 that do

## Goal
Make "an operation's response must not carry a secret" a rule the build enforces, and remove the
three operations currently violating it.

## The finding

`AGENTS.md` § Authentication contract now states the rule — an operation whose declared response
carries a token is withheld, because the host's redactor holds only values the host itself resolved
and cannot know a secret minted by the very call returning it. It was written for `/oauth/token`.
A scan of all **681 operations across 53 providers** in `web/public/catalog.json` shows it was never
only an OAuth problem:

| Operation | Field | The vendor's own words |
|---|---|---|
| `zoom-meeting-create` | `start_url` | *"HOST-PRIVILEGED. Embeds the host's ZAK token: anyone holding this URL starts the meeting as its host. Treat it as a credential: do not log it, echo it…"* |
| `zoom-meeting-get` | `start_url` | as above |
| `postmark-server-get` | `ApiTokens` | *"ACCOUNT-PRIVILEGED. This server's own live Server Token(s), **in plaintext** — the Account API's own mechanism for retrieving one. Treat it as a credential…"* |
| `postmark-server-list` | `Servers.ApiTokens` | as above, for every server on the account |
| ~~`babelforce-get-user-customer`~~ | `accessToken` | **withheld by C-426** — verified against the document, whose schema the vendor calls *"REST API access credentials"* |

**The Postmark pair was missed by the first scan and is the worst of the set.** The first pass walked
`properties` only one level deep; `ApiTokens` sits nested under `Servers`. It is not a URL embedding a
token like Zoom's — it is an **array of live tokens in plaintext**, and the vendor describes the
endpoint as the account API's own mechanism for retrieving one. A second, deeper scan is what found
it, which is itself the argument for a gate: a one-level heuristic run by hand missed a plaintext
credential array on the first attempt.

**The Zoom pair is the one that matters, and it is not a new discovery** —
[C-79](C-79-sensitive-response-fields.md) has carried *"Zoom's `start_url` carries a host-privileged
token · the redactor cannot see it"* in its frontmatter since it was filed, and it is still `ready`.
The connector documents the hazard accurately and then returns the field anyway. Describing a
credential is not withholding it.

The scan also produced 28 false positives, and they are worth recording so the gate does not chase
them: babelforce's `sessionId`/`session_id` (a call-session identifier), Klaviyo's `public_api_key`
(*"Public by design — it is embedded in the account's own web pages"*), Typeform's `token` (*"This
response's own opaque id"*) and Zendesk's `authenticity_token` (*"not a credential for this API"*).
Every one of those is correctly documented in the connector. **A name-shaped heuristic is not the
rule** — the rule is about what the value *is*.

## The decision: withhold the operations (a), not strip the field (b)

The story's substance was a choice between **(a)** withholding all four operations now and letting
[C-79](C-79-sensitive-response-fields.md) restore them, and **(b)** landing enough of C-79's
declaration here to strip the credential-carrying *field* and ship the operation without it.

**(a), and the reason is that (b) is not available to be done honestly from inside this repository.**
Stripping a field from `response_schema` does not stop the field arriving. Measured, in this session:

- `crates/connector-flux/src/op.rs` emits `return $response` for every operation — there is no
  projection or filtering path in the emitter at all, which is what C-79's own notes already record.
- `crates/connector-pack/src/lib.rs` states it for the host side: the pack *"delegates to the
  transport the host bound and hands back what it produced"*, and shapes nothing.
- `response_schema` is what does travel to a consumer: `crates/connector-cli/src/site.rs:716` clones
  it into `web/public/catalog.json`, which is the tool contract a model reads.

So (b) would have moved `ApiTokens` and `start_url` out of the published *contract* while leaving
them in the *payload* — deleting the disclosure and keeping the exposure, which is strictly worse
than the state it replaced. `providers/postmark.toml`'s own header made exactly that argument for
declaring the field, and the error in it was the framing: the choice was never "declare the field or
omit it", it was "return the token or do not", and the operation is what returns it.

Doing (b) properly means the host redacting a declared location before the value reaches a
model-visible symbol. That is `connector-pack`/`connectors-api` work, outside this story's `areas`,
and it is C-79's fourth acceptance item, which C-79 itself files as a *specification* to be handed to
flux rather than as work landing here.

**What did land from C-79 is its declaration**, because the gate has to read something: `Operation`
now carries `credential_response`, a list of JSON Pointers into the response (with `*` for every
element of an array), validated to resolve. Today its only consequence is refusal. When C-79's
redaction or C-136's diversion lands, that consequence changes and the declaration does not — which
is the sequencing this story's notes already describe.

## Acceptance
- [x] The **four** operations no longer ship, each recorded as a named exclusion with its reason —
      the same three-category accounting babelforce already uses (emitted / inexpressible / withheld).
- [x] **A gate fails the build when an operation's declared response carries a credential**, so this
      cannot recur silently as connectors widen. A failing-first test reinstates one of the three and
      asserts the build refuses.
- [x] **The gate is declaration-driven, not name-matching.** 28 of 31 scan hits were false positives
      whose connectors already document them as harmless. The mechanism [C-79](C-79-sensitive-response-fields.md)
      designs — a connector *declaring* that a response field is a credential — is what the gate reads.
      A regex over field names would fail every one of those four and teach authors to fight it.
- [x] `docs/designs/spec-front-end.md` and `AGENTS.md` agree on one statement of the rule; the
      Authentication contract already carries it and must not be restated differently.

## Progress

**Done, 2026-08-01.** Option (a) — see the section above for why (b) was refused rather than skipped.

*The declaration.* `Operation::credential_response` (`crates/connector-spec/src/ir.rs`) plus
`response_location_exists`, which walks a response schema through `properties` **and** `items`, so
`/Servers/*/ApiTokens` resolves and `/ApiTokens` does not. `skip_serializing_if` keeps every
pre-existing connector's encoded IR byte-identical, so no `ir_sha256` moved for a provider nobody
edited. Published in `schema/provider-toml.schema.json`; `provider_schema.rs` keeps the two in sync
without being asked.

*The gate.* `validate_credential_response` (`crates/connector-spec/src/provider.rs`) refuses three
things: a location with no `response_schema` to resolve against, a location matching nothing (loud,
because that is the shape a vendor rename takes — C-79's second acceptance item), and the declaration
itself. Pinned as golden rejection fixtures, `credential-response-reinstated` — which is
`postmark-server-list` reinstated with its real nested schema — and
`credential-response-matches-nothing`.

*The second half, because a declaration nobody makes catches nothing.*
`crates/connector-spec/tests/credential_response.rs` names the four withheld ids with their reasons
and asserts that **no** shipped definition declares any of them, and that each is still named in some
provider file. Reinstating one silently is a red build. It names no provider — the set is derived
from `providers/` — which is also what keeps `shipped_providers_build.rs`'s guard against a
hand-maintained provider list satisfied.

*The removals, and what went with them.* postmark loses its whole Account API surface: both
operations, the `account` service (zero-operation services are refused), the
`postmark.account_token` credential and the config field that asked a human for it. zoom loses
meeting get and create. Measured after regeneration: **678 → 674 operations, 943 → 937 artifacts, 53
providers unchanged.** Six artifacts were orphaned rather than rewritten and were removed by hand —
`build`/`diff` compare planned artifacts against committed ones and have no view of the inverse,
which is C-429.

*The ratchet.* `response_schema_coverage.rs`'s floor is the one sanctioned red test: coverage is
**606 of 674** against a `COVERED_FLOOR` of 610, so the floor wants **610 → 606**. `ABSENCE_CEILING`
needs no move — absence is **68**, unchanged, and 69 is within its slack of 2. Both constants are
coordinator-owned and were left untouched.

*Two demonstrations were lost with the operations, and are recorded rather than quietly dropped.*
`crates/connector-flux/tests/postmark_connector.rs` measured C-180's two-credential-partition finding
on a live two-service connector and can now measure only its surviving half; the finding is kept in
the file's docs as a finding. `zoom_connector.rs` loses the emitted-`$payload` half of the nested
`settings` claim — `zoom-meeting-create` was the fleet's only payload root holding leaves and a
branch — and the IR-level rule is kept so a body arriving later still meets it.

*Not done here, because the file is coordinator-fenced:* `WHATS-NEW.md` wants an entry. A published
catalogue narrowed by four operations and one whole vendor surface is a user-visible change, and the
story's own notes call for saying plainly what stopped being available and why.

## Notes
- **Sequenced with [C-79](C-79-sensitive-response-fields.md), which owns the declaration** this
  gate reads, and with [C-136](C-136-credential-diversion.md), which owns the eventual answer: an
  operation that legitimately produces a credential returns a **handle**, not the secret. Until C-136
  lands, withholding is the only available answer, and this story is that.
- The Zoom pair is a **release regression in the honest sense**: they shipped in v0.9.0 and in every
  release before it. Removing them narrows a published catalogue, which is a user-visible change and
  wants a `WHATS-NEW.md` entry saying plainly what stopped being available and why.
- Scan command used, so the next person reproduces rather than re-derives it: walk every
  `providers[].operations[].response_schema` in `web/public/catalog.json` for property names matching
  token/secret/password/api_key/private_key/credential/refresh/start_url, then **read each hit's
  description** — that second step is what separated 3 from 31.
