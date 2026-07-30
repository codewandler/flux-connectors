---
id: C-79
title: Declare that a response field is a credential
pillar: Spec
status: ready
priority: 2
design:
epic: connectors-v1
areas: [connector-spec, connector-flux, flux-bridge]
note: Zoom's start_url carries a host-privileged token · the redactor cannot see it
---

# Declare that a response field is a credential

## Goal
Give a connector a way to say "this location in the response is a credential", so a host can redact
it before it reaches a model-visible symbol — the inverse of the `$auth` seam, which pushes secrets
*into* requests but has nothing that pulls them *out of* responses.

## Acceptance
- [ ] **The declaration is typed, not a comment.** A per-location marker with a JSON Pointer into the
      response — an operation-level `sensitive_response = ["/start_url", …]` or a validated schema
      keyword. It must be a real IR field: every struct is `deny_unknown_fields`, so a key placed
      inside `response_schema` (a free-form `serde_json::Value`) **deserializes silently and does
      nothing**. Inert is worse than absent, because it reads as protection.
- [ ] **A pointer that matches nothing is a loud error**, not a no-op — otherwise a vendor renaming a
      field silently unprotects it.
- [ ] **`connector-flux` refuses the contradiction it can see**: an operation marking a response
      location sensitive while its `risk`/`expose` say the payload is freely returnable. That gate
      lands here rather than waiting on flux.
- [ ] **The flux-side half is specified and filed**, paste-ready in the style of
      `docs/designs/auth-seam-flux-stories.md`: after a response arrives, the host either redacts the
      declared pointers before the value reaches a model-visible symbol, or extracts and registers
      them with the redactor. Note why registration alone is insufficient today —
      `Redactor::redact` matches by **exact substring** and `add_secret` is only ever called on a
      credential the *host* resolved, so a token the vendor minted inside a URL was never registered
      and is never masked in a transcript, a log or an error.
- [ ] **Zoom declares `start_url` on both operations that return it**, and its prose warning is
      retained rather than replaced — the declaration is the control, the prose is the disclosure.
- [ ] **The already-shipped set is swept**, since nothing has ever been able to declare this. Check at
      least: `shopify-order-get` (`order_status_url` carries a `key` capability token, plus
      `checkout_token`/`cart_token`), `freshdesk-ticket-get` and `zendesk-ticket-comment-list`
      (attachment URLs are pre-signed), `asana-task-get` (`attachments.download_url` is a signed,
      expiring URL). Each is either declared or recorded as checked and clear.

## Progress
- Not started. Filed 2026-07-30 from C-78, and confirmed leg by leg in the source by an independent
  review.

## Notes
- **`AGENTS.md`'s credential invariant is not violated and does not cover this.** It forbids a
  credential *value* in a provider TOML, generated Flux, a manifest or the lockfile — all artifacts,
  all true. What it never contemplated is a **response** placed in front of a model containing a
  host-privileged URL.
- The gap is structural and therefore applies to all 16 providers: `Operation` has eleven fields and
  none marks sensitivity, `Param` has five and none does either, and every emitted op ends
  `return $response` with declared type `Any`. There is no projection or filtering path in the
  emitter at all.
- Priority 2 because it is a live exposure on a shipped connector rather than a missing capability,
  and because every additional provider widens it.
