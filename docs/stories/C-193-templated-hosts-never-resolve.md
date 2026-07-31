---
id: C-193
title: "A templated base URL reaches the wire verbatim, so six connectors cannot resolve a host"
pillar: Bridge
status: in-progress
priority: 2
design: docs/designs/connector-configuration.md
epic: tool-pack
areas: [bridge, connector-pack]
note: "the gap with no owner: C-68 declares the variable, C-86 landed the IR, C-87 publishes it — and NOTHING substitutes a tenant's value into the URL. `crates/connector-pack/src/lib.rs:106-111` names the gap without a story id. Blocks 38 operations across zendesk, shopify, jira, freshdesk, salesforce, docusign — and poisons `permission_subjects` too"
---

# A templated base URL reaches the wire verbatim, so six connectors cannot resolve a host

## Goal

Let the pack substitute a tenant's configuration values into a templated `base_url`, so that a
connector whose host carries a `{subdomain}` can build a request that reaches a real server.

## What was measured

The execution path is otherwise complete. C-114, C-115 and C-116 landed, and for **7 providers /
38 operations** every in-tree prerequisite for a live call is already satisfied. This story is one of
the two things standing between the rest of the catalogue and that same state (the other is
[C-92](C-92-authorities-for-every-provider.md)).

**Six providers declare a templated host**, and every operation under them builds a URL containing a
literal brace:

| provider | `base_url` |
|---|---|
| `zendesk` | `https://{subdomain}.zendesk.com/api/v2` |
| `shopify` | `https://{shop}.myshopify.com/admin/api/...` |
| `jira` | `https://{site}.atlassian.net` |
| `freshdesk` | `https://{domain}.freshdesk.com/api/v2` |
| `salesforce` | `https://{instance}.my.salesforce.com` |
| `docusign` | `https://{account_host}/restapi/v2.1/accounts/{account_id}` |

Re-measure the list and the operation count before starting; the catalogue moves.

**Every layer to the left of the pack already exists.** [C-68](C-68-endpoint-binding.md) declares the
variable, [C-86](C-86-connector-configuration-epic.md) landed `ConfigField` and `Binding::Endpoint`
in the IR, and [C-87](C-87-configuration-codegen.md) will publish it. `parse_binding`
(`crates/connector-spec/src/config.rs`) already resolves `endpoint.<name>` to the placeholder it
fills. **What is missing is the consumer**: nothing hands the pack a tenant's values, and nothing
substitutes them.

`crates/connector-pack/src/lib.rs:106-111` names this gap in prose and cites no story id. This is
that story.

## The second half, which is easy to miss

`Operation::permission_subjects` (`crates/connector-pack/src/tool.rs:260-277`) is the **mirrored
network gate** — the pack calls `http.request`'s `execute` directly, bypassing `Executor::dispatch`,
so this is the only place flux's egress allow-list is consulted for the inner call. Today an
un-substituted host is declared as the subject, which an allow-list **cannot match**. C-115's
Progress records this as "Left for the next story".

So the fix is not only "build a working URL". It is "declare the subject the request will actually
reach", and getting the first without the second produces a request that is either refused by the
gate or — worse — admitted against a subject nobody can audit.

## Acceptance

- [x] **Failing-first test:** an operation under a templated provider builds a request whose URL
      still contains `{`, and/or declares a permission subject that does. Named
      `a_templated_host_is_substituted_into_the_request_url` and
      `the_permission_subject_is_the_host_the_request_reaches`, in
      `crates/connector-pack/tests/endpoint_configuration.rs`.
- [x] A tenant's endpoint values reach the pack through a **bound port**, in the shape
      `Credentials` already uses — `Arc<dyn …>` handed in at construction, never a global, never an
      ambient environment read. `crates/connector-pack/src/config.rs`: `ConfigStore` behind
      `Configuration`, a required argument to `pack` and `Operation::project`. The crate now reads
      no environment variable at all.
- [x] Substitution is **total or refused**, never partial. `Error::MissingConfig`, raised before the
      body is evaluated, naming tenant/connector/field. `Error::UnresolvedEndpoint` is the second
      lock for the one residual path (a caller parameter whose text spells a variable).
- [x] `permission_subjects` declares the **substituted** host, on both the built path and the
      malformed-call fallback. `tool.rs`'s `subjects`/`substituted_host`; asserted by
      `the_permission_subject_is_the_host_the_request_reaches`,
      `the_fallback_subject_is_substituted_too`, and catalogue-wide by
      `a_templated_host_is_never_declared_as_a_permission_subject`.
- [x] The env-read for Basic auth's user half is **moved to the same port** (`Field::Username`),
      not merely reconsidered. A server environment holds one value and this is a per-tenant one, so
      a pack serving a second tenant would have signed its requests as the first.
- [x] No credential value or tenant value enters a committed artifact. `diff` reports
      `470 artifacts up to date (43 providers checked)` and no artifact moved.
- [x] The scoped gate is green and the build stays a fixed point.

## Progress

- **Landed.** The port is `Configuration` + `ConfigStore` + `MemoryConfig` in
  `crates/connector-pack/src/config.rs`, bound at construction beside `Credentials`.
- **The port is synchronous, and that is forced rather than chosen.**
  `Tool::permission_subjects` cannot fail and cannot await, and it is the only place flux's egress
  allow-list is consulted for the pack's inner call. An `async` port would be unusable there, so the
  subject would have fallen back to the template — the exact failure this story exists to remove. A
  host that keeps settings in a database resolves eagerly and binds a snapshot.
- **The variables are read off each operation's own emitted Flux**, not from the IR, so this lands
  against the catalogue as it stands and does not widen into C-87. The derivation is exact rather
  than heuristic: flux interpolates `fmt` and never `lit`, so a brace surviving in a string literal
  is by construction a name no evaluation fills — and across all 242 emitted operations the only
  such literals are the nine templated base URLs.
- **Substitution is over literals, never over the finished URL.** The one-line-shorter alternative
  would let a caller's parameter value be filled with a tenant's configuration on its way to the
  vendor. `a_parameter_that_spells_a_variable_is_not_substituted` pins it.
- **Re-measured, and the story's figures were low.** Not six providers/38 operations but **nine
  providers / 53 operations**: seven with a templated *host* (`zendesk`, `shopify`, `jira`,
  `freshdesk`, `salesforce`, `docusign`, `okta` — 43 ops) and two with a templated *path* on a host
  that does resolve (`contentful`, `statuspage` — 10 ops). The story missed `contentful`; `okta` and
  `statuspage` landed on `main` mid-story. The path-templated pair is the quieter half:
  `https://api.statuspage.io/v1/pages/{page_id}` reaches a real server and gets a `404` that reads
  as a missing record rather than as a connector nobody configured.
- **`freshdesk` and `okta` both name their variable `domain`.** The port is keyed by connector as
  well as by variable, so these are two values rather than a collision.
- **New refusal not in the story:** `Error::TenantMismatch`. Both ports carry a tenant, and nothing
  in the types stopped a host from pairing tenant A's credentials with tenant B's settings — whose
  outcome is one tenant's token sent to another tenant's host. Refused at `project`.
- Left for C-87: publishing the configuration *surface* (labels, help text, `binds`) into the
  manifest and catalogue, so a product can render "connect your Zendesk" rather than a host having
  to know the variable names. `Operation::endpoint_variables()` is the interim answer.

## Notes

- **Read `docs/designs/connector-configuration.md` first.** It draws the operator/connection level
  split, and this story lives entirely on the *connection* level: a subdomain is a tenant's value,
  not the product's.
- **Do not widen this into C-87.** Publishing the configuration surface into the manifest and the
  catalogue is C-87's job and it is a separate, breaking change. This story is the runtime consumer
  and can land against the IR as it stands today.
- **Do not invent a config file format.** The port takes values from the host; how a host obtains
  them is the host's business, and `crates/connectors-app` is where that gets exercised.
- Worth checking while here: whether `docusign`'s two-placeholder host (`{account_host}` **and**
  `{account_id}`) works through the same path, since it is the only provider needing more than one
  substitution and it also puts one in the *path* rather than the authority.
