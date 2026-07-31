---
id: C-193
title: "A templated base URL reaches the wire verbatim, so six connectors cannot resolve a host"
pillar: Bridge
status: ready
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

- [ ] **Failing-first test:** an operation under a templated provider builds a request whose URL
      still contains `{`, and/or declares a permission subject that does. Name it.
- [ ] A tenant's endpoint values reach the pack through a **bound port**, in the shape
      `Credentials` already uses — `Arc<dyn …>` handed in at construction, never a global, never an
      ambient environment read. `crates/connector-pack/src/credentials.rs:6-14` states that posture
      and is the model to follow.
- [ ] Substitution is **total or refused**, never partial. A `base_url` with an unfilled placeholder
      must produce a named error, not a request to a host containing a brace. This is the same rule
      `request.rs` already holds for its evaluator: *"a partly-evaluated request is a different call,
      and the vendor answers it"*.
- [ ] `permission_subjects` declares the **substituted** host, and a test asserts the subject the
      gate sees is the host the request reaches.
- [ ] The env-read for Basic auth's user half (`credentials.rs:307-329`) is reconsidered in the same
      pass — it is the same category of mistake (process environment standing in for tenant config),
      and fixing hosts while leaving it is a half-migration. Either move it to the same port or
      record why it stays.
- [ ] No credential value or tenant value enters a committed artifact. The substituted URL exists
      only at request-build time.
- [ ] The scoped gate is green and the build stays a fixed point.

## Progress

- (not started)

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
