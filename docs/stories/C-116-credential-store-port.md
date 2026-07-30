---
id: C-116
title: "The CredentialStore port, in-Rust auth assembly, and redaction"
pillar: Bridge
status: blocked
priority: 3
design: docs/designs/connector-tool-pack.md
epic: tool-pack
areas: [bridge, connector-spec]
note: "finally wires C-90's Layout/CredentialRef to a consumer — and removes the $auth seam from milestone 1's critical path, because a Tool builds `Bearer <token>` itself"
---

# The CredentialStore port, in-Rust auth assembly, and redaction

## Goal

Give the pack a bound credential adapter, and assemble the vendor's auth **in Rust** — the `Bearer`
prefix, the basic-auth base64, the query-parameter placement — so flux's whole-value `{"$secret"}`
marker never needs to grow those capabilities.

This is what takes the `$auth` seam off milestone 1's critical path.

## Acceptance

- [ ] A `CredentialStore` port exists and is bound when the pack is constructed, not looked up
      globally. Its addressing is C-90's existing `CredentialRef` + `Layout` /
      `TenantLayout` (`crates/connector-spec/src/credential.rs`), which currently has no consumer.
- [ ] The three axes of [unified-auth.md](../designs/unified-auth.md) — source × acquisition ×
      placement — are honoured: header with prefix, basic base64, and query placement each reach the
      request correctly.
- [ ] **`ctx.redactor.add_secret(...)` is called before the request is constructed**, not after, so a
      failure between construction and dispatch cannot surface the value. `crates/flux-web/src/http.rs:248`
      is the precedent.
- [ ] **Failing-first test:** `a_credential_never_reaches_a_surface` — drive an operation with a known
      sentinel secret and assert the sentinel appears in neither the `ToolResult` content, nor the
      `view`, nor an error string, nor a progress line. It must fail against an implementation that
      builds the header without registering the redactor.
- [ ] A missing credential is a clear, actionable error naming the `CredentialRef` that was not found
      — never a request sent without auth.
- [ ] The gate is green.

## Notes

- **Out of scope, as it has been since C-90:** managing or refreshing expiring tokens. The store
  hands back a value; rotation is the host's problem.
- Do not put credential *values* anywhere near the catalogue, the manifest, the lockfile or a
  generated artifact — the standing rule in `AGENTS.md`. This story handles values only in memory,
  and only behind the redactor.
- The redactor is `pub redactor: Redactor` on `ToolContext` (`crates/flux-runtime/src/lib.rs:1226`).
- Once this lands, update [C-26](C-26-file-seam-stories-on-flux.md) and
  [auth-seam.md](../designs/auth-seam.md) to narrow the seam to the composite-only case, and record
  that the Tool pack does not need it. Leaving 11 drafts described as "critical path" when they are
  no longer on it is its own kind of drift.

## Progress

- **Parked after a session-limit failure, not a defect.** The implementor was killed by
  `You've hit your session limit` mid-run — infrastructure, so it was not retried; a spawn that fails
  on a limit fails identically until the limit lifts.

  **Its work is preserved on `impl/C-116` at `ebdaed8`**, committed by the coordinator because the
  worktree held 33 uncommitted files and no commit of its own. That commit is explicitly **not
  reviewed, not gated, and not ready to merge**. It stopped while adding a `credentials()` helper to
  the integration test files.

  Landed there: `crates/connector-pack/src/{auth,credentials}.rs` and `tests/credentials.rs` — the
  port over C-91's `SecretStore`, plus in-Rust auth assembly.

  **Three things a resuming implementor must settle first:**

  1. **18 files under `crates/catalog/src/generated/` are modified.** Those are coordinator-owned
     whole-catalogue artifacts and were fenced from this story. Establish whether the change is
     deliberate and out of lane, or incidental regeneration, before anything is merged. Note C-125
     recorded that carrying a composed schema into the Rust catalogue would rewrite exactly these 18
     files — so this may be that edge, arrived at from the other direction.
  2. `Cargo.toml` / `Cargo.lock` are modified. Expected: this story was the wave's manifest owner and
     was allowed the `connector-secrets` dependency.
  3. **The named failing-first test has not been demonstrated red at the merge base.** Its whole point
     is that a sentinel secret reaches no surface, so a proof taken after the fact proves nothing —
     re-take it.

  Everything C-116 depends on has landed: C-115's request path and C-91's `SecretStore`. The story is
  unblocked on substance; it is blocked only on capacity.
