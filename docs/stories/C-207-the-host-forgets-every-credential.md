---
id: C-207
title: "The host forgets every credential on restart — `MemoryStore` is not a deployment"
pillar: Bridge
status: ready
priority: 2
design:
epic: connectors-api
areas: [bridge, host]
note: "owner-directed 2026-07-31: the goal is an app you sign into and then wire connectors inside. Wiring that does not survive `cargo run` twice is a demo, not a deployment — and the store that forgets is the same one holding the first plaintext credential this repository has ever held at runtime"
---

# The host forgets every credential on restart — `MemoryStore` is not a deployment

## Goal

Give `connectors-api` a credential store that survives the process, without widening what a
credential can reach on its way there.

## What was measured

`crates/connectors-api/src/state.rs:32` binds the host's credential port to `Arc<MemoryStore>`, and
the doc-comment above it is honest about the choice:

> In memory, deliberately, for now: the process exiting is the cleanup, and this is the first
> component in the repository that holds a plaintext credential at runtime. A file-backed `0600`
> store and the existing `VaultStore` are both drop-in — the port is `Arc<dyn SecretStore>` precisely
> so that swapping it is a one-line change at this call site.

That was the right call for the vertical slice, which had to prove one live vendor call and nothing
more. It stops being the right call the moment the service has accounts: [C-204](C-204-google-signin-accounts.md)
makes a tenant a durable thing keyed by an OIDC `sub`, and a durable principal whose credentials
evaporate on restart is a worse shape than no principal at all — the operator re-pastes every token
every time, which is exactly the habit that gets a token pasted somewhere it should not be.

## Why it is not simply "call `VaultStore` instead"

The port is one line; the decision behind it is not.

- **A file-backed store puts plaintext credentials on disk** for the first time in this repository.
  `0600` is the floor, not the answer: the file's directory, its umask at creation, what happens on a
  partial write, and whether the path is inside the repo checkout (it must not be — `App::new` is
  currently handed `CARGO_MANIFEST_DIR`) are all part of the decision.
- **`VaultStore` moves the problem to an operator dependency.** For "locally deployable" that is a
  heavy prerequisite, and [C-149](C-149-vault-live-leg-reports-ok-when-it-skips.md) is the standing
  warning about how its live leg reports success when it skips.
- The two are not exclusive. The port exists so a deployment can choose; what is missing is the
  choice being expressible and its default being safe.

## Acceptance

- [ ] **Failing-first test:** store a credential through the host's HTTP surface, drop and rebuild the
      `App` against the same location, and assert the credential is still resolvable — a test that
      fails against `MemoryStore` today. Name it.
- [ ] The store is selected by configuration, with a default that is safe for a local single-operator
      deployment. The selection is a startup decision that fails loudly on a bad value, never a silent
      fallback to memory — a host that quietly forgets is the failure this story exists to end.
- [ ] The persisted location is **outside the repository checkout** and outside any directory served
      by the host. Asserted, not assumed.
- [ ] File permissions are `0600` and the containing directory `0700`, set at creation rather than
      fixed up afterwards, and a test asserts the mode on a freshly created store.
- [ ] A write is atomic — no path through the code leaves a truncated or partially-written store on a
      crash mid-write.
- [ ] **The redaction guarantee is unchanged and re-proved.** `crates/connectors-api/tests/host.rs`
      already asserts no credential value reaches any served surface; extend it so that a credential
      loaded *from the persisted store* is held to the same property, including on error paths.
- [ ] No credential value reaches a log line, a `Debug` rendering, or a startup diagnostic naming the
      store. [C-159](C-159-request-debug-and-query-encoding.md) is the precedent for how a derived
      `Debug` becomes the leak.
- [ ] `crates/connectors-api/README.md` states where the store lives, how to point it elsewhere, and
      how an operator destroys it — a credential store with no documented delete is one an operator
      cannot revoke.

## Notes

- Sequencing: this and [C-204](C-204-google-signin-accounts.md) both land in `crates/connectors-api`
  and will collide. C-204 first — the tenant a credential is stored *for* has to be real before
  persisting it means anything.
- The session store from C-204 has the same restart question and a different answer: sessions
  *should* be invalidated by a restart until there is a reason otherwise, and conflating the two
  stores would silently make stolen cookies outlive the process.
- `SecretStore` is the port; `Credentials::new(store, tenant)` is the pairing. Whatever backs the
  store must keep tenant isolation structural rather than by key-prefix convention alone — the
  `TenantMismatch` reasoning in `lib.rs` is the standard to hold.
