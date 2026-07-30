---
id: C-76
title: Ship the OpenRouter connector
pillar: Spec
status: ready
priority: 7
design: docs/designs/provider-operation-inventory.md
epic: provider-fleet
areas: [providers, connector-spec]
note: bearer · OpenAI-compatible · charter-named
---

# Ship the OpenRouter connector

## Goal
Ship the third model provider `AGENTS.md` names in its charter, and the cheapest connector in
the fleet: OpenRouter speaks the OpenAI request shape, so C-51's operations transfer almost unchanged.

## Acceptance
- [ ] `providers/openrouter.toml` is hand-authored, following the `providers/zendesk.toml` precedent: the
      header comment names the upstream document, says why it is not a `[spec]` pointer yet (C-4's
      ingest is unimplemented), and lists the operation set as the selection to reproduce.
- [ ] `base_url = "https://openrouter.ai"`, `vendor = "OpenRouter"`, and a `[[auth]]` entry with `scheme = "bearer"` over `OPENROUTER_API_KEY`, named by `default_auth`.
- [ ] A curated set of roughly three over `/api/v1`: chat completion, models list, generation get
      — the OpenAI-compatible subset.
- [ ] **No operation declares a string-ish or `Any`-typed query parameter**, tested in the strong form
      (zero query parameters of any type, on the IR and on the emitted URL). C-30 is unimplemented, so
      the emitter still emits such a value unencoded — the defect behind `zendesk-ticket-search`. Any
      operation needing one is excluded and named in Notes.
- [ ] **No optional request-body field is declared** until C-56 lands: an omitted optional body field
      travels as an explicit `null`, which several vendors reject. Declare required fields, and name in
      Notes what was left out for this reason.
- [ ] `cargo run -p connector-cli -- build` emits `connectors/openrouter.flux` and
      `connectors/openrouter.connector.toml`, both committed, and a second build is byte-identical.
- [ ] The generated module passes the same parse-and-analyze and formatter fixed-point gates as every
      shipped provider, and the provider is picked up by the shared per-provider tests.
- [ ] `http_hosts` derives from the base URL and is never widened to `*`; no credential value appears
      in any generated artifact, the manifest, or the public catalogue. A test asserts it.
- [ ] Every write operation's `risk` reflects what it changes and who sees it; no write is marked
      idempotent unless the vendor documents it as such.
- [ ] **`max_tokens` is required, as C-51 made `max_completion_tokens` required**: an operation an LLM
      can call that spends money must not be unbounded, and required also sidesteps the optional-body
      `null` gap (C-56).
- [ ] The optional `HTTP-Referer` and `X-Title` attribution headers are **not** declared as caller
      parameters; they are constant headers, which is C-55's subject. Record the omission.
- [ ] The story records how much of `providers/openai.toml` transfers and what differs, so the next
      OpenAI-compatible vendor is a copy rather than a rediscovery.

## Progress
- Not started. Filed 2026-07-30 in the ten-provider fleet push.

## Notes
- Cost-bearing operations carry a `risk` above `low`, following C-51.
- Deliberately excluded pending C-30: the models list's filter parameters.
- **Still cannot make a live call**, for the same reason as every shipped connector: flux's
  `http.request` takes `{"$secret": …}` as a whole-value replacement, so no scheme prefix is produced
  (`docs/designs/auth-seam.md`). This connector is the catalogue's proof, not a working client.
