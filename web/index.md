---
layout: home

hero:
  name: flux-connectors
  text: Vendor API specs, compiled into Flux-Lang
  tagline: Describe a provider once in TOML. The build emits committed, reviewable Flux modules and a capability manifest — so you stop hand-writing the part a machine can derive.
  actions:
    - theme: brand
      text: What works, and what does not
      link: '#status'
    - theme: alt
      text: View on GitHub
      link: https://github.com/codewandler/flux-connectors

features:
  - title: One IR, several backends
    details: A provider is described once. The same IR emits the .flux module, the connector manifest, and the embedded Rust catalogue — no second source of truth to drift.
  - title: Flux, not a template DSL
    details: Modules are built as real flux-lang AST nodes and formatted by flux-lang's own formatter, never by string templates. Unparseable output is structurally impossible.
  - title: Credentials stay out of artifacts
    details: No credential ever enters a provider TOML, a generated .flux file, or the lockfile. The generated call carries an auth reference the host resolves and redacts.
---

## What this is

flux-connectors compiles **vendor API specs into Flux-Lang**.

Integrating a SaaS product into [flux](https://github.com/codewandler/flux) normally means writing a
stdio plugin — a large hand-written Rust artifact for a handful of operations. But almost everything
such a plugin encodes (base URL, auth kind, endpoints, parameters, response shapes) is already
published by the vendor. A **connector** is what remains once you stop hand-writing that part:
**auth + operations + quirks**.

Describe a provider once in `providers/<name>.toml`, and the build emits committed, reviewable
artifacts — `connectors/<name>.flux` (typed `op` declarations) and `connectors/<name>.connector.toml`
(the capability manifest). flux loads the module and every `op` becomes a first-class operation and
an LLM tool.

```flux
op zendesk-ticket-comment-add(ticket_id: Number, updated_stamp: String, body: String, public: Bool) -> Any
  description "Add a comment to a ticket; the comment is an internal note unless public is explicitly true"
  risk "medium"
  idempotency "conditional"
  effects ["network"]
  expose true

  $base = "https://{subdomain}.zendesk.com"
  $url = fmt("{base}/api/v2/tickets/{ticket_id}.json")
  $content_type = "application/json"
  $safe_update = true
  $payload = { ticket: { comment: { body: $body, public: $public }, safe_update: $safe_update, updated_stamp: $updated_stamp } }
  $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "PUT", url: $url })
  return $response
```

## Status — v0.0.1 {#status}

Early. The pipeline works end to end and three providers compile — **zendesk** (7 operations),
**freshdesk** (9) and **babelforce** (9), 25 operations curated from 186 available.

**Nothing here can make a live API call yet.** Read the limits below before evaluating this.

## What does not work yet

Stated plainly, because a connector that looks like it works and doesn't is worse than one that says
it doesn't:

- **No provider can make a live call yet.** All three need credentials, and flux's `http.request`
  cannot express any of their auth schemes — its `{"$secret": "ENV"}` marker is a whole-value
  replacement, so it produces neither a `Bearer ` prefix nor a base64-joined Basic pair. The fix is
  designed in
  [docs/designs/auth-seam.md](https://github.com/codewandler/flux-connectors/blob/main/docs/designs/auth-seam.md)
  and must land in flux.
- **Freshdesk ships with no credential at all**, deliberately. Its `base64(<api_key>:X)` puts the
  secret in the *username* position, which the IR cannot yet mark as secret — so it would escape
  secret gating and redaction. Fail-closed 401s beat a leaked key.
- **`zendesk-ticket-search` is non-functional.** Query values are not percent-encoded and flux has no
  op that does it. Note that `url::Url::parse` already rescues *spaces*, so a casual test looks fine
  while `&`, `#` and `+` corrupt the request — and a value like `x&per_page=1` injects parameters.
  See
  [docs/designs/query-encoding.md](https://github.com/codewandler/flux-connectors/blob/main/docs/designs/query-encoding.md).
- **Base URLs carry unbound template variables** (`https://{subdomain}.zendesk.com`) with no env
  binding yet.
- **OpenAPI ingest is not wired.** All three providers are hand-authored; the loader refuses a
  `[spec]`-backed provider rather than emitting an empty module.
- **This site does not browse the catalogue yet.** The
  [provider & operation explorer](/explorer) is not implemented — see that page for what is missing.

## Where to read more

| If you want | Read |
|---|---|
| Why this exists, and the principles | [docs/vision.md](https://github.com/codewandler/flux-connectors/blob/main/docs/vision.md) |
| What ships next, and the epics | [docs/roadmap.md](https://github.com/codewandler/flux-connectors/blob/main/docs/roadmap.md) |
| How a provider becomes a `.flux` module | [docs/designs/connector-pipeline.md](https://github.com/codewandler/flux-connectors/blob/main/docs/designs/connector-pipeline.md) |
| One credential model for every provider | [docs/designs/unified-auth.md](https://github.com/codewandler/flux-connectors/blob/main/docs/designs/unified-auth.md) |
| The operating contract, if you are an agent | [AGENTS.md](https://github.com/codewandler/flux-connectors/blob/main/AGENTS.md) |
