---
id: C-508
title: "A GitLab connection supports an operator-approved self-managed HTTPS origin"
pillar: Connector
status: done
design: docs/designs/connector-configuration.md
epic: connector-config
areas: [providers, connector-spec, connector-pack, catalog, tests]
note: "Milestone 1 critical: GitLab must support gitlab.com by default and an operator-pinned self-managed origin before the collaboration migration can replace the native plugin"
---

# Support an operator-approved self-managed GitLab origin

## Goal

Let one labelled GitLab connection address either `gitlab.com` or a company's self-managed GitLab
installation without giving a Service Account, model invocation, or ordinary tenant caller the
ability to choose an egress destination.

The connector owns the GitLab REST API path. A person supplies or selects only the HTTPS origin
(`https://gitlab.company.example`); `/api/v4` remains connector-declared and cannot be replaced by
connection input.

## Acceptance

- [x] `gitlab.com` remains the zero-configuration default and produces the current
      `https://gitlab.com/api/v4` request URLs and permission subjects byte-for-byte.
- [x] GitLab declares its self-managed origin as non-secret configuration with complete form
      metadata. The manifest and public catalogue publish the same field, its level, validation,
      default and operator-approval policy so Exchange's console and the Flux onboarding CLI can
      render one contract without parsing provider TOML. This consumes C-87's config and `verify`
      projection rather than adding a GitLab-only surface.
- [x] A custom value is an absolute HTTPS origin only. The loader/runtime refuses `http`, userinfo,
      query, fragment, credentials or a path outside the explicitly accepted origin grammar;
      it also refuses attempts to smuggle or replace `/api/v4` through an origin value. The
      connector, not input, appends exactly `/api/v4`.
- [x] A custom origin becomes usable only after deployment/operator policy approves and pins it for
      the connection. An ordinary signed-in connection owner may propose the value if the host flow
      permits that, but cannot activate or silently widen it; a Service Account, operation input and
      model-visible tool contract have no field that can set or override it.
- [x] Request composition and capability/permission subjects derive from one resolved endpoint and
      carry the exact same scheme, authority and effective port. A test fails if authorization is
      checked against `gitlab.com` while transport reaches the custom origin, or vice versa.
- [x] A live fixture serves GitLab-shaped responses at a custom HTTPS origin and proves both
      `verify` and one ordinary operation use that origin plus `/api/v4`; the default SaaS fixture
      remains green.
- [x] The generated connector manifest, embedded catalogue, public catalogue and form declaration
      are regenerated and agree on the default, custom-origin shape and approval requirement.
- [x] No configured origin value, connection label, credential value or authorization header is
      copied into emitted Flux, model-visible descriptions, logs, refusal details, conformance
      evidence or generated public artifacts. Credential references remain addresses, never values.
- [x] C-402's fail-closed whole-authority rule is enforced for this connector: a self-managed origin
      is the explicit operator-pinned case, not an unbounded tenant-controlled host template.
- [x] Approval enforcement reads the embedded closed typed policy. An unapproved proposal is refused
      and absent from permission subjects, intents and evidence; exact approval, replacement, revoke,
      named instances and the no-proposal case are covered.
- [x] Loader and runtime share parity cases for the accepted origin grammar, and tests prove configured
      origins, connection labels and authorization values do not leak. GitLab's declared `gitlab.com`
      default is reported as zero-configuration rather than `unbound-base-url-template`.

## Progress

- 2026-08-04: Independent pre-PR review found that derived `Debug` output on `ConfigValue` and
  `MemoryConfig` exposed a configured origin. A failing-first sentinel now pins both public debug
  surfaces to hand-written output that retains only the approval decision and conceals store
  addresses and values.
- 2026-08-04: Integration review closed. Approval is matched as a closed type; proposals,
  replacements, revocations and named instances fail closed without entering subjects, intents or
  executor evidence. Loader/runtime grammar parity, three-sentinel non-leak evidence and GitLab's
  zero-configuration public status are pinned by targeted tests.
- 2026-08-04: Reopened after integration review found proposal fallback and declaration-string policy
  checks at the runtime boundary, missing grammar/non-leak evidence, and a false public status issue.
- 2026-08-04: Initial implementation. GitLab.com remains the zero-configuration default; an operator-approved
  connection may instead pin a strict HTTPS origin, with one resolved endpoint shared by request
  composition and permission subjects and exercised against a live TLS fixture.

## Notes

- **Sequence:** C-87 first publishes complete config and `verify` declarations. This story then
  makes GitLab the self-managed reference connector. C-499 may claim GitLab migration parity only
  after both are delivered and exercised through Exchange.
- The human-facing connection name (`--name company`) belongs to Exchange's labelled connection
  lifecycle, not provider TOML. This story requires every generated/provider artifact to remain
  independent of that label.
- This refines C-402 without weakening it: a SaaS connector can declare a closed host set or suffix;
  a self-managed product cannot know an installation's host at compile time, so it declares an
  operator-pinned origin policy instead. Both forms refuse an unbounded caller-selected authority.
