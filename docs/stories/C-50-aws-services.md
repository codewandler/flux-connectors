---
id: C-50
title: Offer AWS as a provider with s3 and bedrock as services
pillar: Spec
status: backlog
priority:
design:
epic: connectors-v1
areas: [connector-spec, providers, flux-bridge]
note: first multi-service provider · SigV4 signs the request, not a header
---

# Offer AWS as a provider with s3 and bedrock as services

## Goal
Prove [C-49](C-49-provider-services.md)'s service level on a real multi-service vendor, and settle
honestly which parts of AWS a generated connector can reach — because AWS breaks three assumptions
this pipeline currently makes, and each break is worth naming before an operation is emitted that
cannot work.

## Acceptance
- [ ] **`providers/aws.toml` is hand-authored**, pid `com.amazonaws`, with `bedrock-runtime` and `s3`
      as declared services and a curated handful of operations each. AWS publishes Smithy models and
      botocore JSON, **not OpenAPI 3**, so C-4's ingest does not apply; hand-authoring follows the
      zendesk precedent (C-17) rather than blocking on a second ingest format.
- [ ] **Per-service endpoints as operator config.** `bedrock-runtime.{region}.amazonaws.com` and
      `s3.{region}.amazonaws.com` are per-service `base_url` overrides; the region resolves from
      operator config, never baked in. Each service's manifest carries its own `http_hosts`, never
      widened to `*`.
- [ ] **SigV4 is recorded as a request *signer*, not a credential value.** The signature is
      HMAC-SHA256 over a canonical form of the whole request — method, path, sorted query, signed
      headers, payload hash, region and service name — so its value depends on the request rather than
      on the credential alone. `source × acquisition × placement` (C-19) cannot express that: every
      axis value there yields a value derivable before the request exists. The design states which it
      is — a fourth notion, or a `sign` acquisition the host applies to the assembled request — and
      amends `docs/designs/unified-auth.md`. C-19's schema-accepted `hmac` placeholder is
      insufficient, and the story says so in as many words.
- [ ] **No unsigned request is ever emitted.** Until the flux-side signer lands, SigV4 operations are
      **refused at emit** with an error naming this story and the operation, following C-8's and
      C-30's refusal pattern. A loud refusal beats a connector that 403s on every call.
- [ ] **Paste-ready flux story drafts for the signer seam**, in the style of
      [auth-seam-flux-stories.md](../designs/auth-seam-flux-stories.md), each naming its
      failing-first test. Credential assembly stays host-side: a signing key in a `.flux` file would
      land raw key material in a model-visible symbol.
- [ ] **`bedrock-runtime` is the tractable first AWS service** and it builds: `InvokeModel` and
      `Converse` are JSON in and JSON out, and the generated module passes the C-11 parse-and-analyze
      gate and the formatter fixed-point test.
- [ ] **S3's reachable surface is written down, gap by gap**, with each operation either in scope now
      or blocked on a named gap:
      XML response bodies (`ErrorEnvelope.message_pointer` and `response_schema` are JSON-pointer and
      JSON-schema shaped) · opaque byte bodies for object GET/PUT (the body model is JSON-schema
      based) · virtual-hosted bucket-in-hostname templating (`{bucket}.s3.{region}.amazonaws.com`) ·
      the required payload-hash header. The likely answer is a small control-plane subset and a
      recorded refusal for the object data path.
- [ ] **The charter boundary is checked in writing.** AWS services are HTTP + auth + quirks and are
      paid SaaS, so they sit inside `AGENTS.md`'s boundary; the contrast with the technology adapters
      that stay hand-written flux plugins is stated, because "S3 is storage" is the obvious objection
      and it deserves an answer rather than a silence.

## Progress
- Not started. Filed alongside C-49 on 2026-07-30, split out from it because the service level is a
  spec change that must land clean, while AWS additionally needs a signing seam, a non-OpenAPI source
  format and an XML/bytes answer — one story covering both could not be reviewed.

## Notes
- **Unblocks after C-49.** Nothing here is coherent without the service level: a single-service AWS
  connector would need one `base_url`, one `api_version` and one flat operation list, which is exactly
  the shape AWS does not have.
- **The signer is the real cost, not the operations.** Both an authored `aws.toml` and the emitter
  work are ordinary; the seam is a change in flux and belongs on flux's board like the `$auth` marker
  (C-16, C-26). Expect this story to file stories elsewhere and then wait.
- **Bedrock is worth having on its own merit** — it is the first connector that makes flux call a
  model provider through generated Flux, and it needs no XML and no byte bodies.
- The `s3` XML finding likely generalises: any SOAP-era or XML-first vendor hits the same two fields.
  If a second one appears, that is a story about the IR's response model rather than about AWS.
