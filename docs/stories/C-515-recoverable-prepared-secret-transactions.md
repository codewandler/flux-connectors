---
id: C-515
title: "Publish recoverable prepared secret transactions"
pillar: Bridge
status: ready
priority: 0
design: docs/designs/recoverable-prepared-secret-transactions.md
epic: all-integrations-connectors
areas: [connector-secrets, persistence, transactions, security, windows, release]
note: "Milestone 1 blocker — Exchange X-134 needs crash-recoverable atomic connection onboarding without moving credential bytes or crate-private SecretBatch mutations into Exchange"
---

# Publish recoverable prepared secret transactions

## Goal

Let an owning host coordinate one crash-recoverable transaction across connector credentials and
its own value-free metadata while `connector-secrets` remains the sole owner of credential values,
file format and batch interpretation. Exchange can prepare, query, commit or abort an opaque secret
transaction, but can never inspect or persist its values or mutation representation.

## Acceptance

- [ ] `connector-secrets` publishes validated opaque `SecretTransactionId` and
      `SecretProposalDigest` 256-bit newtypes, the closed state
      `Absent|Prepared|Committed`, and an object-safe `PreparedSecretStore: SecretStore` port usable
      as `Arc<dyn PreparedSecretStore>`. The port admits only `prepare(id, digest, &SecretBatch)`,
      `state(id)`, `commit(id)` and `abort(id)`; it exposes no staged path, address, count, secret,
      mutation iterator, backend handle or arbitrary transaction callback.
- [ ] `prepare` durably stages one checked `SecretBatch` inside the credential backend without
      changing any result from `get`, `references` or an ordinary `SecretStore` reader. Repeating a
      prepared id with the same proposal digest returns the existing prepared transaction and never
      replaces its staged values; a different digest refuses value-free. The digest is supplied by
      the coordinator and the store never computes or persists a secret-derived hash, length,
      presence fingerprint or serialized mutation list outside its credential sink.
- [ ] `commit` atomically exposes exactly the staged batch and records `Committed` as one recoverable
      backend decision. Repeating commit or querying after response loss returns `Committed` without
      a second mutation. `abort` atomically removes a prepared transaction and is idempotent for
      absent state; it can never undo or delete a committed transaction. A missing id, conflicting
      digest, unavailable backend, unsafe metadata and already-committed abort have closed
      value-free errors.
- [ ] `FileStore` implements the port on Unix and Windows. Staged values live only in owner-only
      credential-store files beneath its existing validated root; they are invisible to inventory
      and removed after abort or successful publication. Current committed credential bytes and
      `SecretStore::apply` semantics remain compatible. A clean old v1 store opens without migration
      ambiguity, and an interrupted new transaction is recovered without weakening owner, mode,
      DACL, link, reparse or bounded-file checks.
- [ ] The backend retains committed transaction state long enough for deterministic coordinator
      recovery and same-proposal replay under an explicit bounded retention policy. Reclamation
      never deletes a live credential and never turns an unresolved/committed transaction into
      `Absent`; an owner must supply an acknowledged safe floor before value-free transaction records
      can be compacted.
- [ ] `MemoryStore` implements identical state transitions for deterministic host tests. Vault and
      any backend that cannot prove durable prepare/query/commit/abort semantics return one explicit
      unsupported capability; a caller cannot emulate the port with point writes or treat
      unsupported as an empty/absent transaction.
- [ ] Failing-first crash injection covers every durable boundary: before/after stage-file write,
      stage fsync, prepare publication, commit decision, credential publication, directory fsync,
      committed-state publication and staged cleanup. Reopening a new store instance always reports
      `Absent`, `Prepared` with old credentials visible, or `Committed` with the complete new batch;
      it never exposes a partial batch, a truncated prior store or an ambiguous state.
- [ ] Concurrent native tests prove one transaction id cannot be prepared with two digests, two ids
      cannot race to mutate overlapping addresses, commit versus abort has one closed winner, and an
      ordinary `put`, `delete` or `apply` cannot interleave with a prepared commit to lose updates.
      Lock refusal remains value-free and never repairs unsafe state.
- [ ] Raw, escaped, percent-encoded and base64 sentinels are absent from transaction ids, proposal
      digests, state/error/debug/display output, traces, audit fixtures, paths, lock names and every
      persisted file outside the existing committed store plus its explicitly allowlisted staging
      sink. No public type containing `Secret` implements `Debug`, serialization or value-returning
      inspection.
- [ ] Native CI executes prepare/crash/reopen/commit/abort/concurrency and unsafe-root evidence on
      `aarch64-apple-darwin`, `x86_64-apple-darwin`, `aarch64-unknown-linux-gnu`,
      `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-msvc`. Cross-compilation or one architecture
      standing in for another is not acceptance evidence.
- [ ] The crate README/rustdoc, `docs/designs/recoverable-prepared-secret-transactions.md`, both
      changelogs and public documentation state the ownership boundary and exact crash model. The
      full workspace gate, publication-closure dry run and native jobs pass. Completion requires a
      verified crates.io release consumed by Exchange X-134; a path/git dependency or merged but
      unpublished commit does not satisfy the provider dependency.

## Progress

- 2026-08-04: Filed after the X-134 contract audit proved that an Exchange-owned SQLite credential
  schema would contradict Decision 0004 and that released `connector-secrets` 0.19.1 intentionally
  keeps `SecretBatch::operations` and `Mutation` crate-private. Existing atomic `apply` cannot by
  itself distinguish a crash before versus after a cross-store commit decision without persisting
  secret values in the coordinator.

## Notes

- Cross-repository authority:
  `../flux-roadmap/decisions/0004-flux-manages-a-verified-local-exchange.md` and
  `../flux-roadmap/decisions/0007-local-onboarding-uses-owner-bound-capabilities.md` at roadmap
  commit `ced7426`.
- Exchange owns its value-free transaction journal, metadata roll-forward, connection publication,
  audit outbox and receipt. This story owns only the credential-side prepared transaction and never
  learns a connector label, setting, grant, Service Account, Exchange receipt or lifecycle concept.
- C-509's existing portable owner-only store remains complete; C-515 extends its public host port
  without reopening platform ownership or permitting another credential schema.
