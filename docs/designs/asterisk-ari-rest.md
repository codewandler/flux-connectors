# Asterisk ARI is a generated REST connector

## Decision

Asterisk ARI belongs in `flux-connectors`. Its outbound API is an HTTP REST description and follows
the same path as every other spec-backed connector: pinned vendor bytes, a deterministic normalized
OpenAPI document, explicit selection metadata, generated Flux, catalogue entries, and execution
through `connector-pack`.

The earlier choice to put Asterisk in Flux was wrong. It followed the broad label “technology
adapter” instead of the actual interface requested by the owner, crossed a repository boundary the
task did not require, and added AMI and event-runtime scope that was not requested.

## Measured source boundary

Re-measured on 2026-08-02 from Asterisk tag `22.10.1` at commit
`f0e408a7b0d829c85bf15fa4b487870a50cb3000`:

```text
python3 <inventory over rest-api/api-docs/*.json>
documents 11
paths 76
operations 109
websocket 1
rest 108
methods {'GET': 32, 'POST': 48, 'DELETE': 20, 'PUT': 8}
```

The eleven vendor documents use legacy Swagger 1.1 (the events document uses 1.2). They are still
the source contract. A deterministic repository script converts them into one OpenAPI 3 document
because the existing connector front-end accepts OpenAPI 3, not Swagger. The generated document is
reviewable derived input, never an independently authored API description; provenance retains the
upstream tag, commit, URL, every raw-file hash, and the normalized-file hash.

## Release surface

> **C-30 update — 2026-08-03.** The source and normalized census remains 108 REST operations. The
> published connector now contains 96: twelve operations declare array-valued query parameters but
> do not declare whether ARI expects repeated keys, delimiters, brackets or JSON, so exact
> reason-bearing deferrals withhold them. The remaining scalar query values use Flux 0.54's
> structured RFC 3986 encoding. The paragraphs below describe the v0.17.0 release surface before
> that fail-closed narrowing.

This release includes all 108 non-WebSocket operations declared by the source. That census includes
`events.userEvent`, which is an ordinary outbound POST. It excludes only
`events.eventWebsocket`, whose `upgrade = "websocket"` is not REST.

The excluded WebSocket is deliberately not represented as an operation, event, or synthetic
lifecycle control. Event delivery will become channel work after the channel concept is mature.
No WebSocket, AMI, blob-store, plugin-protocol, or Flux host capability is part of this design.

`recordings.getStoredFile` remains in the REST census. Its response is binary, so its source schema
is retained and the operation is catalogued; no special cross-repository download runtime is added.
The generic HTTP result is the only execution contract this release claims.

All 108 operations are catalogued and addressable. Broad selectors conservatively classify POST
and PUT as non-replayable high-risk writes, DELETE as destructive and non-replayable, and GET as
low-risk reads. A bounded useful set
may be exposed to model tool registries; `expose = false` is not an execution refusal because the
host's named-operation resolution path remains available.

## Endpoint and authentication

ARI uses HTTP Basic authentication and a deployment-specific host. The connector asks for one
username, one password, and one host and renders the TLS-enabled ARI form
`https://{host}:8089/ari`. This preserves the catalogue's rule that credentials are never sent over
plain HTTP; existing connector-pack URL and private-network guards remain authoritative, and this
provider gets no bypass. Supporting a caller-selectable scheme or non-default port is separate
endpoint-configuration work and is not disguised as an Asterisk runtime requirement.

## Ownership and write sets

- C-484 owns raw vendoring, normalization, provenance, and tests over the exact 109-to-108 census.
- C-485 owns `providers/asterisk.toml`, provider-specific tests, and per-provider generated files.
- C-486 owns whole-catalogue artifacts, documentation/changelogs, closure, and the immediate release.
- The Flux repository independently deletes its complete Asterisk plugin and references. No source
  file is moved from Flux as authoritative code; only first-party vendor bytes may reproduce the
  same upstream hashes.

## Proof

The provider story must fail first on the absent provider, then prove:

- every normalized operation is traceable to exactly one raw Swagger operation;
- exactly 108 non-WebSocket operations are selected and `eventWebsocket` is not;
- the provider is a scoped build fixed point and every operation composes an absolute, brace-free
  request from declared endpoint/auth configuration;
- the full integration gate is green after coordinator-owned regeneration;
- a new `flux-connectors` release is cut immediately after the provider lands.
