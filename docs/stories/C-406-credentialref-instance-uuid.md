---
id: C-406
title: "CredentialRef gains an optional instance uuid, required when a tenant holds more than one"
pillar: Bridge
status: done
note: "owner-directed 2026-08-01. Two Zendesk instances for one tenant render ONE address today, so the second connection silently overwrites the first and calls go to whichever credential survived — a 200 from the wrong instance, not a refusal. flux-exchange X-14 is the same fact from the host side"
---

# CredentialRef gains an optional instance uuid, required when a tenant holds more than one

## Goal

`CredentialRef` can name **which instance** of a connector a credential belongs to, so one tenant can
hold two connections to the same vendor — two Zendesk subdomains, a sandbox and a production Jira —
and address them apart.

## The bug this closes

`CredentialRef` (`crates/connector-spec/src/credential.rs:66`) is four components: `tenant`,
`authority`, `service`, `credential`. `Connector::credential_ref_for` (`ir.rs:1268`) composes it from
the tenant, the connector's declared `authority`, the elided `default` service and the credential
leaf. **Nothing in it varies per connection.**

So a tenant that connects `acme.zendesk.com` and then `acme-eu.zendesk.com` renders one address for
both. The second write overwrites the first, and every later call resolves whichever credential
survived — returning a `200` from the wrong instance rather than refusing. There is no compile-time
signal and no runtime error; the only symptom is data from the wrong account.

This is the inverse of [C-226](C-226-one-credential-cannot-be-shared-by-two-connectors.md): that one
is a credential two connectors cannot share, this one is a connector that cannot hold two
credentials.

## The shape, as directed

- `CredentialRef` gains an **optional** instance component, a **uuid**.
- It is **required when the tenant holds more than one integration of the same kind**, and absent
  when there is exactly one — so every existing single-instance address renders unchanged.
- The **ambiguous case refuses**. A tenant with two instances and a reference that names none is an
  error naming what would have worked, never a guess at which one was meant. "Refuse; never repair"
  is the family's stated posture and this is exactly the case it is for.

A uuid rather than an operator's label is deliberate: it is stable under rename, cannot collide, and
cannot be spelled to traverse. **Worth deciding explicitly in this story, not left implicit:** a uuid
is opaque to the operator reading it, so the human-facing "production vs sandbox" naming has to live
somewhere — most likely as a label on the connection that *resolves to* the uuid, with the uuid
being what reaches the address. Say which layer owns that mapping, even if the mapping itself is a
host concern rather than this crate's.

## Acceptance

- [x] `CredentialRef` carries an optional instance uuid, validated at construction like every other
      component — `new` re-checks components precisely because a reference can be built from outside
      a loaded `Connector`, and this one is no different.
- [x] **Failing-first test** — two instances of one connector, for one tenant, render **different**
      addresses. This is the bug; assert it directly.
- [x] **Failing-first test** — a single-instance address renders **byte-identical** to what it
      renders today. The instance component is additive, and an address that shifted under existing
      deployments would strand every credential already stored.
- [x] The ambiguous case — more than one instance, no uuid supplied — is a **refusal naming what
      would have worked**, not a default and not the first match.
- [x] A uuid that is not a uuid is refused at construction, and the refusal names the component.
- [x] `Connector::credential_ref_for` and every call site are updated coherently; no second spelling
      of an address appears anywhere in the workspace.
- [ ] The address grammar is documented wherever the four-component form is currently written down,
      including `crate::address` and the integration guide. **All but the integration guide** —
      `docs/integrating-with-flux.md` is owned by C-403 in this wave and was left untouched
      deliberately; the paragraph under "The unit of addressing is a `CredentialRef`" is the one
      place still stating the four-component form.

## Progress
- **Landed on `impl/C-406`.** The address is
  `tenants/<tenant>/<authority>[/@instances/<uuid>][/<service>]/<credential>`.
- `InstanceId` is a validated newtype (canonical lowercase hyphenated uuid only — one value, one
  address; the nil uuid is refused because "no instance" is already spelled by omitting the level).
- `TenantInstances` carries the fact this crate cannot derive: how many connections a tenant holds
  and which one is named. It states the whole rule — elide at one, the named one at several, refuse
  when several and none is named, refuse a uuid the tenant does not hold.
- The marker is `@instances`, and `@` is unspellable in every component grammar, so the level cannot
  be forged and no service or credential name is reserved away. A bare uuid segment would have been
  ambiguous with a service, since a uuid is a well-formed service name.
- `Connector::credential_ref_for` takes `TenantInstances`; every call site passes
  `TenantInstances::sole()` except the new tests, so no shipped address moves —
  `cargo run -p connector-cli -- diff` reports 557 artifacts up to date.
- **The label→uuid mapping is the host's** (recorded in `docs/designs/credential-addressing.md` and
  in `AGENTS.md`): the label is tenant-scoped, mutable and renameable, and a compiled artifact must
  hold none of those. `connector-pack` still composes the sole-connection form; threading a
  connection through is the host's, i.e. flux-exchange X-14.

## Notes
- **Consumer waiting on this:** flux-exchange
  [X-14](https://github.com/codewandler/flux-exchange/blob/main/docs/stories/X-14-two-instances-of-one-connector.md)
  and X-10. That host must not fork the address scheme locally to route around the gap — two
  spellings of an address is how two components stop agreeing where a credential lives — so the
  dimension belongs here.
- The constraint that shapes the host side, from flux-exchange's `docs/designs/invoke.md`: **the
  caller cannot name the authority, the host or the credential.** An instance selector is a value a
  caller *does* supply, so the host resolves a tenant-scoped label to the uuid; the caller names
  *which of my connections*, never *what it points at*. This story only has to make the address able
  to carry the distinction.
- Ordering: this is independent of [C-403](C-403-flux-0-45-bump.md) — it touches the address model,
  not the engine line — but both land before a host can execute an operation against the right
  account.
- **Sequenced behind [C-407](C-407-extract-the-credential-address-crate.md), noted 2026-08-01.** That
  story extracts the credential address vocabulary into its own crate, i.e. it *moves* the very types
  this story adds a component to. Implementing both at once means two agents editing
  `crates/connector-spec/src/credential.rs` and `address` concurrently, and the merge would be
  resolved by whoever went second guessing at the other's intent. Land C-407 first, then add the
  instance component in its new home. There is a live `impl/C-406` branch and worktree from that
  effort; it holds the story-filing commits, not an implementation of this story.
