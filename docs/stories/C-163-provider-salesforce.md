---
id: C-163
title: Ship the Salesforce connector
pillar: Spec
status: done
design:
epic: provider-fleet-2
areas: [providers]
note: "PROBE: base URL is per-tenant (`https://{instance}.my.salesforce.com`) — the first provider whose HOST comes from configuration. AGENTS.md names Salesforce as belonging here"
---

# Ship the Salesforce connector

## Goal

Add Salesforce to the catalogue, and use it to answer the question it is chosen for.

## What this connector forces

**A configured host.** Every shipped provider has a constant base URL. Salesforce's is `https://{instance}.my.salesforce.com`, discovered at login, so the connector cannot name its own authority.

The epic's rule is that a connector earns its place by exercising something the model has not met —
[C-105](C-105-provider-fleet-2-epic.md), *"a connector that only adds a row is a row."* This one's
answer may be a **recorded refusal** rather than a shipped provider, and that is a successful outcome
if it names the constraint at `path:line`. C-107 spent its first attempt proving Notion could not ship
honestly before C-55, and that attempt was worth more than a connector that answered `400`.

**Auth:** OAuth2 bearer access token.

**Curated operation set (a starting point, not a mandate):** get an SObject record, create one, update one, run a SOQL query, describe an SObject

## Hazards specific to this one

Two things to check before writing operations: whether a `{variable}` in a base URL resolves from `[[config]]` (the configuration contract says `EndpointSpec::template` composes a URL — establish what it will and will not substitute), and what [C-92](C-92-declare-an-authority.md) expects, since a provider whose authority is per-tenant may not be able to declare one. If the host is not configurable, that is the finding.

## Acceptance

- [x] `providers/salesforce.toml`, hand-authored and **curated** — a small set of operations this pipeline
      can express honestly, not every endpoint the vendor documents. 5 operations: `salesforce-whoami`
      (verify), `salesforce-record-get`, `salesforce-record-create`, `salesforce-record-update`,
      `salesforce-sobject-describe`. The SOQL query operation the story led with is excluded — see
      `## Progress`.
- [x] Declared `risk`, `idempotency` and effects per operation, and a `description` on each written for
      a *model* to read rather than as UI copy. `effects` is not authored per operation — it is always
      `["network"]`, derived by the emitter (`crates/connector-flux/src/op.rs::metadata`) for every HTTP
      op — so there is nothing to author there beyond declaring the operation.
- [x] A `[[config]]` surface with `label` and `help` on every field, and `secret` agreeing with `binds`.
      Two fields: `instance` (`binds = "endpoint.instance"`, not secret) and `access_token`
      (`binds = "credential.salesforce.access_token"`, `secret = true`). Asserted in
      `salesforce_connector.rs::the_instance_template_is_bound_by_a_config_field`.
- [x] A `verify` operation that is a read and runs unattended. `salesforce-whoami`, `GET
      /services/oauth2/userinfo`, takes no parameters.
- [x] `crates/connector-flux/tests/salesforce_connector.rs` — a per-provider contract test asserting the
      thing *this* connector is about (see the archetype above), not that the file parses. The
      load-bearing test is `the_instance_template_is_bound_by_a_config_field`, which asserts the
      `{instance}` template variable is actually bound by a `[[config]]` field rather than merely
      present in `base_url`.
- [x] **Failing-first test:** the contract test must fail before `providers/salesforce.toml` exists. See
      `BASE_PROOF` in the implementation report.
- [x] The scoped gate is green: `build --provider salesforce`, `diff --provider salesforce` reporting no drift,
      `cargo build --workspace`, `cargo test --workspace --no-fail-fast`,
      `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- [x] **Exactly eight tests are red and reported, not silenced** — the whole-catalogue staleness checks
      `AGENTS.md` tabulates. They are red because you correctly did not write a coordinator-owned
      artifact. Report the eight; if the number differs, that is the finding. Measured: exactly the
      eight `AGENTS.md` names, no ninth (`response_schema_coverage.rs`'s three tests, including
      `the_recorded_floor_is_the_measured_figure`, all pass).

## Progress

- **The `{variable}`-in-`base_url` question, settled.** `crates/connector-spec/src/config.rs`
  documents `ConfigField::binds`'s `Binding::Endpoint { variable }` as reaching exactly a
  `{variable}` in the service's base URL (`config.rs:180-184`, parsed at `240-245`), and
  `template_variables` extracts every placeholder a base URL carries (`config.rs:348-363`). A
  configured host is precisely representable today: `[[config]] binds = "endpoint.instance"` at the
  bottom of `providers/salesforce.toml` binds `{instance}`, and
  `salesforce_connector.rs::the_instance_template_is_bound_by_a_config_field` asserts the binding
  exists rather than trusting the TOML's comment. C-169/C-170 (filed as C-187) already established
  that this binding reaches `base_url` and nothing else — no path segment, no query parameter — which
  is exactly the shape this connector's whole requirement is, so C-187's gap never bit here.
- **C-92 ("declare an authority"): no conflict, and none declared.** `authority`
  (`crates/connector-spec/src/ir.rs:764-771`) is the reverse-DNS namespace a provider publishes
  under, independent of `base_url` — the same struct's doc comment says `base_url` "may carry tenant
  templating", and `providers/babelforce.toml`'s header explains the same split for AWS's
  multi-service `s3`/`bedrock-runtime`. So a per-tenant host is not a reason an authority cannot be
  declared; `com.salesforce.api` would render exactly as `com.zendesk.api` does today, unaffected by
  `{instance}` being unresolved at compile time. It is not declared here because C-92 is its own
  open story whose point is deciding every awkward case together with one recorded reasoning; 23 of
  27 shipped providers (including google, which states this explicitly) are in the same undeclared
  state. This is worth restating for whoever picks up C-92: **a per-tenant host is not one of the
  awkward cases that story needs to solve** — it only affects providers with no obvious reverse-DNS
  form at all (openrouter, sentry, zoom, airtable, per C-92's own list), and salesforce is not one of
  them.
- **The SOQL query operation is excluded**, per the story's own instruction to check before promising
  it. `GET /services/data/vXX.X/query/?q=<SOQL>` needs a `q` query parameter carrying a full SOQL
  expression — spaces, quotes, commas, `=` — and the emitter percent-encodes no query value at all
  (`crates/connector-flux/src/op.rs`, C-28/C-30), the exact defect `zendesk-ticket-search` carries and
  `AGENTS.md`'s *Intentional gaps* names. It ships when C-30's structured-`query` handoff lands.
- **API version pinned as a literal, `v59.0`, repeated in each path** — the same non-DRY spelling
  zoom's and shopify's files already carry, because C-49's per-service `api_version` does not yet
  strip a version prefix out of `path` (their own header comments say so). Worth a bulk revisit
  alongside them if C-49 lands.
- **Unverified / not independently confirmed against a live org**, named per the story's instruction:
  the exact response shapes for `salesforce-whoami` (OIDC UserInfo), `salesforce-record-create`'s
  `{id, success, errors}` envelope, and `salesforce-sobject-describe`'s field-describe shape are
  transcribed from developer-facing documentation of Salesforce's REST and OAuth2 APIs recalled during
  authoring, not fetched or vendored (Salesforce publishes no single OpenAPI document for this
  surface, so there is nothing to vendor against, matching babelforce's and jira's provenance
  caveat). The path shapes (`/services/data/v59.0/sobjects/{type}/{id}`,
  `/services/data/v59.0/sobjects/{type}/describe`, `/services/oauth2/userinfo`) and the
  create/update/204 semantics are the well-documented, stable parts of the REST resource and are
  where confidence is highest; the describe response's full field list is the part most likely to be
  incomplete rather than wrong, and it is written as a curated subset for exactly that reason (the
  same posture `providers/jira.toml` takes toward Jira's `fields` map).

## Notes

- **Charter fit.** Salesforce is a paid HTTP SaaS service, which is what belongs here; `vision.md`'s
  non-goals exclude technology adapters and the inference path. If authoring reveals it is really a
  technology rather than a service, stop and say so rather than shipping it.
- **Provenance is hand-authored and drift is undetectable by machine**, the caveat zendesk, freshdesk,
  github and notion already carry. Record it in the TOML's header comment.
- **Do not invent an endpoint.** A confident set of four operations beats a guessed set of ten — the
  repository's rule is that a loud refusal beats plausible-but-incorrect output, and a hallucinated
  path ships as a `404` a contract test cannot catch. Where you are unsure of a path, shape or required
  field, leave the operation out and name it in `## Progress` as unverified.
- **No credential value, and no realistic-looking `example` on a secret field.** A placeholder shaped
  like a real token has already tripped GitHub's push protection and blocked a release.
- Whole-catalogue artifacts are coordinator-owned: `crates/catalog/src/generated.rs`,
  `web/public/catalog.json`, `web/public/v1/**`, `assets/readme-snippet-*.svg`. The per-provider
  `crates/catalog/src/generated/salesforce.rs` is **not** in that set and is yours to commit.

### Coordinator note at integration

**Merging this turned three green tests red, and the connector was not at fault.**
`crates/catalog/src/lib.rs`, `crates/connector-pack/src/lib.rs` and
`crates/connector-pack/tests/projection.rs` each used the literal `"salesforce"` as their
*definitely-not-a-real-provider* sentinel. Shipping Salesforce turned the unknown into a known.

Reverting a good connector because three tests had borrowed its name would have been the wrong repair,
so the sentinels were fixed at integration instead — they span two crates and no provider story could
own them, the same reasoning that makes `COVERED_FLOOR` coordinator-owned.

**The lesson is narrow and worth keeping: a negative sentinel must not be a plausible vendor name.**
`AGENTS.md` had named Salesforce as a provider that belongs here from the beginning, so this break was
scheduled from the moment the sentinel was chosen. Each use is now self-checking — the assertion *is*
that the catalogue does not carry the name — so it cannot rot into a vacuous pass, which is the failure
mode a freshly-picked hardcoded name would only defer.

Two questions this story settled for others:

- **A configured host works, and now it is verified rather than inferred.** `Binding::Endpoint {
  variable }` (`crates/connector-spec/src/config.rs:180-184,240-245`) reaches exactly a `{variable}` in
  `base_url`, so `{instance}` is bound by a config field. C-169 and C-170 had established the *negative*
  half of this (no path segment, no query parameter — [C-187](C-187-config-cannot-pin-a-request-component.md));
  this is the positive half.
- **[C-92](C-92-declare-an-authority.md) has no conflict here.** `authority` (`ir.rs:764-771`) is
  independent of `base_url`, which the same struct documents as possibly tenant-templated. No authority
  is declared, following `providers/google.toml`'s restraint pending C-92's own decision.

SOQL is excluded, correctly: a `q` query parameter is unencoded, which is the `zendesk-ticket-search`
defect exactly.
