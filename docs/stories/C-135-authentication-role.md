---
id: C-135
title: "The authentication role and its grant members"
pillar: Spec
status: ready
priority: 3
design: docs/designs/authentication-surface.md
epic: authentication-surface
areas: [connector-spec, providers]
note: "reuses C-119's role mechanism rather than inventing a category beside it. OAuthGrant already exists with Password (babelforce's flow) and ClientCredentials — this gives OAuth2Spec its first real consumer"
---

# The authentication role and its grant members

## Goal

Declare `authentication` as a role a service can claim, with a required member per grant it supports,
so "which providers can mint a token, and how?" becomes a catalogue query.

## Acceptance

- [ ] `authentication` joins the closed `Role` set from
      [C-120](C-120-service-roles-declaration.md), with required members derived from the grants the
      service declares — not a fixed list, since a `client_credentials`-only vendor has no `password`
      member and must not be refused for lacking one.
- [ ] The grants come from the **existing** `OAuthGrant` in `crates/connector-spec/src/auth.rs`
      (`Password`, `ClientCredentials`, and whatever else it already carries). Do not define a second
      grant vocabulary beside it.
- [ ] A login operation declares which grant it runs, and the loader **refuses** a service claiming
      `authentication` whose declared operations do not cover the grants its `OAuth2Spec` names.
- [ ] The role reaches the manifest and `catalog.json`, so the query works from the published
      catalogue.
- [ ] **The default level is operator.** A login operation is `Level::Operator` unless it explicitly
      declares otherwise, per [connector-configuration.md](../designs/connector-configuration.md). A
      model-triggerable login must be a deliberate, visible exception.
- [ ] **Failing-first test:** `a_service_claiming_authentication_must_cover_its_declared_grants` —
      must fail before the check exists.
- [ ] The gate is green; the build stays a fixed point.

## Notes

- **Blocked on [C-144](C-144-request-body-encoding.md).** OAuth2 token endpoints are
  `application/x-www-form-urlencoded` **by specification**, and no connector can send a non-JSON body
  today — `op.rs` binds `application/json` unconditionally. `oauth2.login` cannot be emitted at all
  until that lands. Found while shipping Stripe.

- **Depends on [C-120](C-120-service-roles-declaration.md)** for the role mechanism, and on whatever
  matching rule that story settled on — it resolved "member name within the service" as a trailing
  segment match, which is worth re-reading before assuming an exact-name contract.
- **This story does not divert anything.** It declares the surface;
  [C-136](C-136-credential-diversion.md) makes the credential unreadable. Landing this alone would
  publish an operation that returns a bearer token as a plain value, so **do not ship it without
  C-136** — say so in Progress if they land separately.
- [C-88](C-88-prove-oauth2.md) already records that `OAuth2Spec` is a landed type **no shipped
  provider uses**, so half the configuration model is proven only by a fixture. Coordinate: this
  story and C-88 are two halves of closing that gap, and babelforce's resource-owner password grant
  is the shipped case.
- Keep the grant vocabulary honest: if a vendor's flow is not one of the declared grants, refuse it
  rather than mapping it onto the nearest one. A mislabelled grant is a security claim that is wrong.
