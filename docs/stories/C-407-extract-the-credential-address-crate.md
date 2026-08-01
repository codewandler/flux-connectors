---
id: C-407
title: "Extract the credential address vocabulary so the compiler leaves the publish closure"
pillar: Build
status: ready
priority: 3
note: "connector-secrets re-exports CredentialRef from connector-spec, which drags the whole compiler IR crate into the publish closure. A 387-line address module is in a published API; a 4000-line IR is shipped to make it resolve"
---

# Extract the credential address vocabulary

## Goal

Stop publishing the compiler to make the host libraries resolve. `connector-spec` should leave the
crates.io closure.

## The leak, measured

- `CredentialRef` lives in `crates/connector-spec/src/credential.rs` — **387 lines**, self-contained.
- `connector-secrets/src/lib.rs:100` re-exports `validate_tenant`, `CredentialRef`, `Layout`,
  `TenantLayout`, `MAX_TENANT`, `TENANTS_ROOT` from it, putting them in a **published public API**.
- `connector-secrets` uses **nothing else** from `connector-spec`: every reference is
  `connector_spec::credential::…`.
- `connector-spec` is the connector IR, provider loading, validation and the lockfile. It is the
  **compiler**, and today it ships to crates.io purely so that one module resolves for consumers.

The roadmap already records the consequence as a fact of life — *"the closure is four crates, not
three"* — without noticing it is a dependency-direction problem rather than a packaging one.

## The shape (decided)

**Extract `credential` into its own small crate** — an address vocabulary that both `connector-spec`
and `connector-secrets` depend on. Suggested `connector-address`, published as
`codewandler-connector-address`.

The closure stays four crates, but *which* four changes, and that is the whole point: a 387-line
address vocabulary is a reasonable thing to have in a published API. A compiler is not. Afterwards
`connector-spec`, `connector-flux` and `connector-cli` are all unpublished, which is the direction
the family is already heading — this repository should ship **data and address vocabulary**, not the
machinery that produces them.

**Rejected:** moving `credential` into `connector-secrets` and having `connector-spec` depend on it.
That inverts the direction — the compiler would depend on a host library — and `connector-spec` genuinely
uses `CredentialRef` internally (`ir.rs:1275-1280`, deriving an address from a connector's declared
authority), so the edge cannot simply be deleted.

## Acceptance

- [ ] `credential` moves to its own crate; `connector-spec` and `connector-secrets` both depend on it.
- [ ] `connector-secrets` re-exports from the new crate, so `use connector_secrets::CredentialRef` is
      **unchanged for consumers**. This is a packaging change, not an API break — if a consumer has to
      edit an import, the extraction was done wrong.
- [ ] `connector-spec` is removed from the publish closure, and `scripts/publish-crates-io.sh` derives
      the new order from the manifests rather than a hand-written list.
- [ ] **Failing-first test** — or, where a test cannot express it, a check: the published closure
      contains no compiler crate. Assert the *property*, not the current membership.
- [ ] `cargo run -p connector-cli -- diff` clean; no artifact moves. This is a crate boundary change,
      not a semantic one.
- [ ] `docs/roadmap.md`'s "the closure is four crates, not three" paragraph is corrected — it records
      the leak as a reason, and after this it is no longer true.

## Progress
- (not started)

## Notes
- **Sequencing:** this adds a workspace member and edits manifests, so it cannot run beside C-403
  (the flux pin bump) or C-405 (the catalogue runtime field). Run it alone.
- Downstream: flux-exchange depends on `connector-secrets` and would pick up the smaller closure for
  free.
