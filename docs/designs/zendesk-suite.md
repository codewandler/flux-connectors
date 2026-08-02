# Design: Zendesk suite — stable Support addresses, spec-backed expansion

**Status:** proposed · **Pillar:** Spec (+ Surfaces) · **Epic:** `zendesk-suite` ·
**Depends on:** [spec-front-end.md](spec-front-end.md), [provider-services.md](provider-services.md)

## Goal

Keep the shipped Zendesk Support connector and its published addresses stable while replacing
hand-transcription with pinned first-party OpenAPI documents and adding deliberately curated
Support, Help Center, Messaging, and webhook surfaces.

This is one Zendesk connector, not an import of every endpoint the vendor publishes. Selection stays
opt-in, every write carries an explicit risk and idempotency statement, and operations whose request
or response cannot be represented honestly remain withheld.

## Measured starting point

The measurements below were re-run on 2026-08-02. These commands, rather than this prose, are the
source of the figures:

```text
rg -c '^\[\[operations\]\]' providers/zendesk.toml
7

cargo run -q -p connector-cli -- diff --provider zendesk
10 artifacts up to date (1 provider checked)

curl -fsSL https://developer.zendesk.com/zendesk/oas.yaml | yq -o=json | \
  jq '[.openapi,.info.version,(.paths|length),([.paths[]|to_entries[]|select(.key|IN("get","put","post","delete","options","head","patch","trace"))]|length)]'
["3.0.3","2.0.0",434,625]

curl -fsSL https://developer.zendesk.com/help_center/oas.yaml | yq -o=json | \
  jq '[.openapi,.info.version,(.paths|length),([.paths[]|to_entries[]|select(.key|IN("get","put","post","delete","options","head","patch","trace"))]|length)]'
["3.0.2","2.0.0",119,181]
```

The seven operations are all in the reserved `default` service and therefore publish addresses in
the `com.zendesk.api:v2#…` namespace and the installable unit `zendesk.flux`. Those are already a
contract and do not move.

## Source policy

Use Zendesk's downloadable first-party documents as evidence, not as an exposure list:

- Ticketing: `https://developer.zendesk.com/zendesk/oas.yaml`
- Help Center: `https://developer.zendesk.com/help_center/oas.yaml`
- Messaging: Zendesk's `sunshine-conversations-api-spec` repository
- Webhooks: the official [API reference](https://developer.zendesk.com/api-reference/webhooks/webhooks-api/webhooks/),
  [setup lifecycle](https://developer.zendesk.com/documentation/webhooks/creating-and-monitoring-webhooks/),
  [request anatomy](https://developer.zendesk.com/documentation/webhooks/anatomy-of-a-webhook-request/),
  [event types](https://developer.zendesk.com/api-reference/webhooks/event-types/webhook-event-types/),
  and [verification](https://developer.zendesk.com/documentation/webhooks/verifying/) prose

The fetched bytes are date-pinned under `specs/zendesk/`, scrubbed by a reproducible script, and
carry public source URL, upstream version, fetch time, upstream hash, and vendored hash. Compilation
remains offline. Prose-only Webhooks API facts are hand-curated and identified as such.

Examples and first-party status do not make vendor bytes safe to publish unchanged. Credential-shaped
values, personal email addresses, and telephone numbers are scrubbed while declarations — especially
`securitySchemes` and response fields — remain intact.

## Address-preserving service growth

Today's service rule makes a default-only provider and a named-service provider mutually exclusive.
That is sound for a new connector and insufficient for a published one that grows: renaming Support
to `support` would change all seven operation addresses, while adding `help-center` beside `default`
is refused.

The prerequisite is an explicit **legacy default** in a mixed connector. It is not implicit fallback:

- every legacy operation explicitly names `default` once a named sibling exists;
- omission in a mixed connector remains a loud error;
- `default` keeps the elided address, credential path, and unsuffixed artifact names;
- named siblings render ordinary service segments and suffixed artifacts;
- roles, tags, configuration, verification, and member ownership remain service-scoped.

This capability is for preserving an address already published, not for letting new providers mint
an ambiguous default beside named services.

## Curated surfaces

1. **Support foundations:** ticket audit history and the bounded user, organization, group, field,
   form, view, and custom-status operations the inventory approves.
2. **Support sync and custom data:** only cursor/pagination and custom-object operations whose paths
   and query inputs can be encoded without injection or ambiguity.
3. **Help Center:** articles, sections, categories, and translations as a named service.
4. **Messaging:** conversations, messages, users, and participants as a named service. Messaging
   webhook administration is withheld because four response schemas return the live signing secret;
   publishing the delete call alone would leave an orphaned destructive lifecycle operation.
5. **Webhooks:** no service ships in this epic. The exact HMAC row is representable, but the ordinary
   lifecycle's generic response may carry `signing_secret`; response-schema narrowing does not
   redact the raw result. C-479 owns lossless event discriminator values and C-480 owns complete
   subscription and generated-secret provisioning before outbound administration or verified
   inbound declarations can land together.

Talk, AI Agents, Sell, and legacy Chat are out of this epic. They lack the same confirmed downloadable
spec route, cross a distinct product/auth boundary, or are deprecated in favour of Messaging; the
inventory records them as later candidates rather than silently absorbing them.

## Safety boundaries

- Query strings remain unsafe until the encoding gap recorded by C-28 is fixed. The first new
  operation is query-free; optional query parameters are explicitly omitted.
- Caller-supplied path strings must remain inside one segment. C-478 derives their placement from
  emitted Flux and refuses the same delimiters already rejected for configuration path pins; this is
  required before Messaging's unconstrained conversation and user ids ship.
- Multipart-only operations wait for C-426.
- Authentication, session, password, OAuth-secret, and signing-secret responses are withheld under
  the credential-response rule enforced by C-430.
- A response-safe update or delete does not ship alone when every operation that discovers or creates
  its resource is withheld. In particular, Zendesk's webhook delete is destructive/non-idempotent:
  the vendor documents 204 and 404 outcomes but no repeat guarantee, and an absent final state is not
  evidence that an automatic retry reproduces the call.
- A spec field absent or wrong is patched only when the overlay can express the correction. Ticket
  creation, for example, does not land merely because the document has a `CreateTicket` operation.
- Existing operation ids, methods, paths, response shapes, OIPs, credential address, and emitted
  files are regression-pinned before conversion.

## Delivery order

The epic begins with three disjoint stories: the mixed-service prerequisite, vendoring, and the
curated inventory. Existing C-6 then measures whether the full Ticketing document can reproduce the
seven shipped Support operations. That preflight disproved conversion through the current overlay:
the pinned paths omit the published `.json` suffix, its response envelopes do not require the
published members, three public writes select the same vendor operation, and the overlay cannot yet
override path/response/repeatability or add wired parameters. The seven therefore remain inline;
preserving their contract takes precedence over forcing a spec migration. Provider stories follow
serially because each writes `providers/zendesk.toml` and the same per-provider artifacts.
Whole-catalogue artifacts remain a coordinator integration step.
