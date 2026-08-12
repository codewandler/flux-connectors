---
id: C-555
title: "Anthropic declares its OAuth2 acquisition"
pillar: Connector
status: done
priority: 1
epic: catalog-artifact
areas: [providers, connector-spec]
note: "The hard leg of the cross-repo login goal: Anthropic's authorize host (claude.ai) differs from its token host (console.anthropic.com), which one endpoint service base cannot express — the token-endpoint quirk surface or a deliberate model extension is the likely path. PKCE S256 is mandatory on this flow, matching what Exchange already enforces"
---

# Anthropic declares its OAuth2 acquisition

## Goal

`providers/anthropic.toml` declares the OAuth2 authorization-code acquisition (the Claude
sign-in flow: authorize on `https://claude.ai/oauth/authorize`, token on
`https://console.anthropic.com/v1/oauth/token`, PKCE S256 required), so a host can compose
Anthropic's authorize URL from the artifact and redeem the code at the declared token endpoint.
Exchange's composition (X-154) and autodev's Anthropic login are the consumers.

## Acceptance

- [x] The two-host shape is expressed honestly within the model — an auth-host service for the
      authorize leg plus the token endpoint carried where the model puts a cross-host token URL
      (the `[[auth.quirks.token_endpoint]]` surface babelforce established is the first candidate;
      read its loader semantics before inventing anything). If the model genuinely cannot express
      it, STOP at that finding: report precisely what is missing, and the model extension becomes
      its own loader/spec story rather than an ad-hoc widening inside this one.
- [x] Grants `authorization_code` + `refresh_token` (this flow issues refresh tokens); PKCE's
      mandatory-S256 property is recorded in the declaration's comments with its source; scopes
      are the minimal set the connector's operations need (`user:inference` for the models
      surface; each scope's reason a comment).
- [x] No registration value in the artifact; the client id is deployment configuration (the flow
      has a well-known public client id — that FACT may be recorded in a comment, the VALUE must
      not be a declaration field the loader would refuse anyway).
- [x] The credential relationship is deliberate: how the OAuth2-acquired token relates to the
      existing API-key credential slot(s) is decided and recorded (a sibling credential the
      operations can authenticate with, or a distinct acquisition on the existing slot) — not
      left implicit.
- [x] Scoped provider gate green with only the documented expected staleness reds;
      `catalog/anthropic.catalog.json`'s auth block quoted in the report.
- [x] Composability sanity: every base URL the composition needs is non-templated or carries a
      declared default (X-154's `NoDeclaredDefault` rule is the consumer contract).

## Progress

- 2026-08-12: Filed by the cross-repo coordinator for the exchange/autodev OAuth login goal.
- 2026-08-12: **Stopped at Acceptance item 1's own escape clause — the model cannot express the
  two-host shape, and two of this story's premises did not survive re-verification.** No
  `[auth.oauth2]` block was written; `providers/anthropic.toml` is unchanged. The four findings
  below are what a follow-up story needs. Nothing here is implementable until the owner rules on
  finding 4, which is upstream of the loader question.

- 2026-08-12 (round 2): **Both flows shipped.** The operator ruled on finding 4 — ship both
  Anthropic OAuth2 flows, accepting the subscription flow's attested ToS restriction explicitly.
  C-556 landed the two spec extensions the findings named (`token_endpoint` two-host reference and
  `public_client` PKCE discriminator). Delivered on `impl/C-555` (`97e4760a` Console flow,
  `36eae6ef` subscription flow, merged): the **Console flow** (single-host, `public_client`,
  `console_oauth` + `console_oauth_admin` with `org:admin`) and the **subscription flow**
  (`subscription_oauth`, authorize on `claude.ai` via a `subscription-authorize` service, token on
  `platform.claude.com` via `token_endpoint: login`, PKCE S256, refresh). Endpoints web-verified
  with sources this session; `console.anthropic.com` corrected to `platform.claude.com` per
  finding 2; `org:admin`/`user:*` scopes per findings 3. The subscription credential authorizes no
  operation deliberately (a URL-composition token for the host). `auth_archetypes` green — the
  public clients are exempt from C-22's operator-secret requirement. Anthropic now ships 4 services
  / 22 per-provider artifacts; the catalogue is 70 services / 1173 artifacts. The findings below are
  kept as the record of why the model needed extending first.

## Findings (C-555, measured 2026-08-12)

### 1. The model cannot express a cross-host OAuth2 flow, and the quirk surface is not the seam

Read from the loader source in this worktree, not from recollection:

- `crates/connector-spec/src/auth.rs:240-264` — `OAuth2Spec` carries **one** host slot,
  `endpoint: String`, documented as *"The declared endpoint name whose base URL the paths below
  resolve against"*. `authorize_path` and `token_path` are both joined onto that **single** base
  URL; `token_path` is documented as *"The token endpoint path (every grant and refresh POSTs
  here)"*. The struct is `#[serde(deny_unknown_fields)]` and its seven fields are `endpoint`,
  `authorize_path`, `token_path`, `client_id`, `scopes`, `grants`, `redirect`.
- `crates/connector-spec/src/auth.rs:225-230` — `OAuthRedirect` is `{ port: u16, path: String }`:
  a loopback callback, carrying no host.
- `crates/connector-spec/src/auth.rs:382-401` — `TokenEndpointQuirk` has exactly four fields, all
  `String`: `grant`, `behaviour`, `attribution`, `measured`, under `deny_unknown_fields`. It is
  **prose provenance, not a URL carrier**, and its own doc comment records the owner's ruling
  against becoming one: *"Prose rather than a field, because a field is a promise every other
  connector is then assumed to keep"*, and *"Kept deliberately narrow. Owner-decided 2026-08-02: if
  it is not in the specification, it does not become a general thing."*
- `crates/connector-cli/src/catalog.rs:867-872` lowers exactly those fields
  (`endpoint`/`authorize_path`/`token_path`/…), so no second host survives anywhere downstream
  either.

**So the `[[auth.quirks.token_endpoint]]` surface is not a candidate for a cross-host token URL and
adding one would be the ad-hoc widening this story forbids.** The gap, precisely: *`OAuth2Spec` binds
the authorize leg and the token leg to one declared service, and therefore to one origin.* Expressing
a two-host flow needs a deliberate spec/loader change — a second endpoint reference (e.g. an optional
`token_endpoint` naming a second declared service, keeping the URL out of the field and the host
inside the `http_hosts` allow-list that `endpoint` exists to preserve). That is its own story.

### 2. The story's token host is stale — `console.anthropic.com` is dead

`https://console.anthropic.com/v1/oauth/token` **404s**; the live endpoint is
`https://platform.claude.com/v1/oauth/token` (console.anthropic.com was renamed to
platform.claude.com). The authorize leg is still reported at `https://claude.ai/oauth/authorize`,
and the registered callback stays on the legacy host
(`https://platform.claude.com/oauth/code/callback`). PKCE `S256` is confirmed as mandatory.
**Every one of these facts comes from community reverse-engineering, not from Anthropic**: Anthropic
publishes no authorize/token URL for this flow on any page fetched during this story.

### 3. `user:inference` is the wrong scope for *this* connector's operations

The story specifies `user:inference` "for the models surface". The scope set on that flow is
`user:profile user:inference user:sessions:claude_code user:mcp_servers`. But
`providers/anthropic.toml`'s own charter boundary is **"NO INFERENCE OPERATION IN THIS FILE"** — what
it ships is the model catalogue plus a curated Admin API slice. `user:inference` authorizes the thing
this connector deliberately does not do, and nothing in that scope set covers the Admin API at all.
Anthropic's own CLI flow uses a different vocabulary (`org:admin`) for exactly that surface. A scope
list is not derivable until finding 4 is settled.

### 4. Upstream of all of it: this may be a flow the connector must not declare

There are **two** distinct Anthropic OAuth systems, and this story conflates them:

- **The Claude subscription login** (claude.ai authorize + platform.claude.com token, fixed public
  client id, `user:*` scopes). Anthropic does not permit third-party client registration on it, and
  since Jan–Feb 2026 enforces its restriction to Claude Code and Claude.ai **server-side**, with
  consumer-plan OAuth tokens rejected elsewhere. *(Widely attested across secondary sources; I could
  not confirm the exact policy paragraph on an Anthropic-hosted page I fetched directly — the owner
  should confirm before this is treated as settled.)*
- **The Console/CLI login** (`ant auth login`), which is what actually authorizes the Models and
  Admin APIs this connector calls. Anthropic's own docs describe it as *"a browser-based OAuth flow
  against the Claude Console"* — i.e. **single-host**, which the current model already expresses —
  but publish no authorize/token URLs for it, and direct non-interactive and third-party use to API
  keys or Workload Identity Federation instead. `platform.claude.com/docs/en/manage-claude/authentication`
  lists exactly three supported methods for the Claude API: **API key, Workload Identity Federation,
  App Attest.** Authorization-code OAuth is not among them.

**The decision this needs from the owner:** whether flux-connectors declares an acquisition that
Anthropic restricts to its own first-party clients (finding 4a), or declares the Console flow whose
endpoints Anthropic does not publish (finding 4b, and unverifiable endpoints cannot be declared under
"Before you assert anything"), or declares neither and records the absence — with WIF's
`POST /v1/oauth/token` as the sanctioned machine-credential path this connector could model instead.

### The credential relationship, decided and recorded

Had the flow been declarable, the OAuth2-acquired token would be a **third, sibling credential**
(`anthropic.oauth_token`), never a second acquisition bolted onto `anthropic.api_key` or
`anthropic.admin_key` — following the gitlab pattern, where `gitlab.token` (static, operator-provisioned)
and `gitlab.oauth_token` (delegated, per-user) coexist and `default_auth` admits either. The reason is
`subject`: both existing anthropic keys are console-minted, organization-provisioned secrets, whereas
an OAuth token is `subject = "user"` and bounded by the signing-in person's own permissions. Merging
them would put two different principals behind one credential name and let a per-user token silently
satisfy an Admin operation that an organization-scoped key was provisioned for.

## Notes

- Write set: `providers/anthropic.toml` + per-provider artifacts; possibly a finding against
  `connector-spec`'s loader (reported, not implemented here).
- Verify the endpoint facts against current public documentation of the flow before declaring
  them; the story's URLs are the coordinator's knowledge-cutoff recollection and must not be
  trusted un-reverified (this repository's "Before you assert anything" rule).
