---
id: C-515
title: "Publish recoverable prepared secret transactions"
pillar: Bridge
status: done
design: docs/designs/recoverable-prepared-secret-transactions.md
epic: all-integrations-connectors
areas: [connector-secrets, persistence, transactions, security, windows, release]
note: "Released in v0.20.0; five native hosts and the immutable Exchange registry-adoption checkpoint are verified"
---

# Publish recoverable prepared secret transactions

## Goal

Let an owning host coordinate one crash-recoverable transaction across connector credentials and
its own value-free metadata while `connector-secrets` exclusively owns durable credential
persistence, prepared representation, file format and batch interpretation. Exchange transiently
receives local input and constructs a batch, but can never inspect its private mutations or persist,
journal, log or derive identity from a credential value.

## Acceptance

- [x] `connector-secrets` publishes non-zero `SecretTransactionGeneration(u64)`, opaque
      `SecretTransactionId([u8; 32])` and `SecretProposalDigest([u8; 32])` newtypes. An id is exactly
      the generation's big-endian eight bytes plus a unique 192-bit nonce, but exposes no ordering,
      `Display`, serialization derive or user-facing numeric transaction API. The provider API alone
      constructs and extracts its protocol bytes; generation zero and wrap refuse.
- [x] The object-safe `PreparedSecretStore: SecretStore` port works as
      `Arc<dyn PreparedSecretStore>` and exposes only `prepare(id, digest, &SecretBatch)`, `state(id)`,
      `commit(id)`, `abort(id)` and `reclaim(through_generation)`. Its separate closed
      payload-free `PreparedSecretError` leaves the existing exhaustive `StoreError` unchanged and
      returns only `Unsupported`, `Busy`, `DigestMismatch`, `TransactionIdReused`, `NotPrepared`,
      `AlreadyCommitted`, `Retired`, `Capacity`, `InvalidBatch` or `Backend`. No result contains an id,
      digest, scope, address, mutation count, stage path or secret-derived fact.
- [x] Public transaction state is exactly `Absent|Prepared|Committed`. A never-seen id above the
      retired generation reports `Absent`; prepare enters `Prepared`; commit enters `Committed`;
      abort records an internal terminal tombstone but reports `Absent`. Abort records that tombstone
      even before prepare, so a delayed prepare cannot resurrect it. A reclaimed id returns
      `Retired`, never `Absent`. The design publishes the exhaustive prepare/state/commit/abort table,
      including same/different digest replay and every commit-versus-abort winner. At full terminal
      capacity, abort of an unseen id returns payload-free `Capacity` without mutation; an `Absent`
      success means the tombstone is durable.
- [x] One store admits at most one prepared transaction. Successful prepare validates scope,
      duplicate addresses, move source/destination state, complete touched-address reservation,
      entry/value/file/ledger bounds and both prospective encodings, then durably stages the complete
      next credential image. While prepared, another prepare and ordinary `put`, `delete`, `apply` or
      reclaim refuse; prepared-port operations return payload-free `Busy`, while the unchanged
      ordinary mutation methods return existing `StoreError::Conflict` with only the store path and a
      fixed value-free prepared-slot reason. FileStore uses its live store path and MemoryStore uses
      exactly `<memory-store>`. Reads see the old committed image. Commit never
      reinterprets the batch or discovers deterministic conflict after the coordinator's decision.
- [x] While one id is prepared, `abort` of any different unseen or terminal id that would rewrite
      the live ledger returns `Busy` without mutation; it cannot publish a tombstone that the
      prepared id's immutable staged image would later erase. State queries remain available, and
      same-id abort retains the exhaustive transition above. Concurrent evidence pins this cross-id
      rule and proves that every acknowledged abort tombstone survives a later commit.
- [x] Same-id/same-digest prepare returns the existing `Prepared` or `Committed` outcome without
      inspecting or replacing the supplied batch. A different digest refuses. Commit is idempotent;
      abort can never undo committed credentials. If commit wins, abort returns `AlreadyCommitted`;
      if abort wins, commit returns `TransactionIdReused`. I/O failure is outcome-uncertain until a
      later state query resolves it.
- [x] `reclaim(G)` is an explicit owner acknowledgement that no transaction in generations `<= G`
      can be queried or replayed. It refuses while any transaction is prepared, advances one durable
      inclusive generation fence and removes only terminal committed/aborted records. Every later
      operation in a retired generation returns `Retired`; reclamation never deletes a credential.
      At most 4096 terminal records and the existing 1 MiB store bound are admitted; capacity refuses
      rather than evicting or growing without bound.
- [x] `FileStore::open` acquires one non-blocking exclusive writer/recovery lease before reading,
      recovery or cleanup and holds it for the store lifetime. Unix validates an owner-UID, one-link,
      regular `0600` lease and uses a kernel file lock. Windows validates current-`TokenUser`
      ownership, a protected owner-only DACL with exactly one allow ACE for the current process SID,
      regular non-reparse metadata and `LockFileEx`. Another
      0.20 opener/process refuses while held; abrupt process exit releases the lease; lease files are
      never repaired, replaced or reaped. Because 0.19.1 does not participate in the lease, upgrade
      instructions require every legacy writer to be stopped before the first 0.20 open and do not
      support mixed-version concurrent writers.
- [x] FileStore v2 couples committed credentials, the retired-through fence and the bounded terminal
      ledger in one owner-only atomic live file. A fixed-name owner-only stage holds the complete
      prepared next image and contains no id/digest in its path. V2 fixture-pins the inclusive fence,
      zero or more terminal records sorted by their raw 32-byte ids, at most one prepared record and
      the existing canonical v1 credential-entry encoding. Prepared/committed records contain a
      digest; abort-before-prepare records use a separate digestless grammar. A clean v1 file opens
      without eager migration after legacy-writer quiescence; first transaction use writes v2; a
      newly opened released 0.19.1 store then refuses v2 rather than erasing metadata. Interrupted
      prepare/commit/abort/stage cleanup has the exact recovery cases in the linked design and never
      yields a partial credential image or ambiguous public state.
- [x] `MemoryStore` implements identical transitions and the one-prepared reservation. Vault and any
      backend that cannot prove durable semantics return `Unsupported`; callers cannot emulate the
      port with point writes or treat unsupported as absent.
- [x] `Secret` retains its existing redacted `Debug`; value-bearing containers/stores and the new
      generation/id/digest types use opaque manual `Debug` and no `Display`. The closed payload-free
      state enum's variant `Debug` exposes only the already-returned public
      `Absent|Prepared|Committed` result. The closed payload-free error may expose variant `Debug` and
      fixed non-contextual `Display`/`Error`. No rendering exposes a concrete id, digest or generation
      value, or a contextual scope, address, mutation kind/count, path, credential value, value length
      or secret-derived fact beyond that explicit state result. Raw, escaped, percent-encoded and
      base64 sentinels are absent from errors, traces, fixtures, paths, locks and every persisted file
      outside the committed credential store and fixed staging sink. None of the new types implements
      serde.
- [x] Failing-first tests spawn and abruptly terminate a real child process at every applicable
      prepare, commit, abort and reclaim stage/live-file write, file flush, atomic replacement,
      Unix directory sync and cleanup boundary; Windows instead pins `FlushFileBuffers`,
      `ReplaceFileW`/write-through `MoveFileExW`, post-replacement handle validation and cleanup. A
      fresh process acquires the released lease and recovers only `Absent` with old credentials,
      `Prepared` with old visible
      credentials plus a complete invisible candidate, or `Committed` with the complete new image.
      Reclaim recovery yields either the complete old fence/ledger or the complete new fence/ledger,
      never a partially retired generation. Two-child tests prove lease refusal/release; concurrent
      tests prove digest conflict,
      abort-before-prepare fencing including cross-id abort-versus-commit, one prepared slot and
      mutation exclusion. A native upgrade fixture proves an already-open 0.19.1 writer is unsafe
      and must be quiesced, while a fresh 0.19.1 open refuses the migrated v2 file.
- [x] Native CI asserts the runtime host triple and executes the complete child-crash, lease,
      concurrency, owner/mode-or-DACL, link/reparse, wrong-kind, bound and unsafe-root suite on
      `aarch64-apple-darwin`, `x86_64-apple-darwin`, `aarch64-unknown-linux-gnu`,
      `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-msvc`. Cross-compilation or another
      architecture standing in is supplementary only.
- [x] The change is released as `connector-secrets` 0.20.0: existing `StoreError` and method
      signatures remain source-compatible, manual opaque Debug retains trait availability, and the
      new FileStore lease/v2 format plus the unsupported mixed-0.19/0.20-writer transition are
      documented as the pre-1.0 minor boundary. README/rustdoc, the linked design, both changelogs
      and public docs agree. The full workspace gate, publication closure and native jobs pass;
      completion records the verified crates.io release and Exchange
      X-134 resolving that registry version/checksum. A path/git dependency or unpublished commit
      does not satisfy C-515.

## Progress

- 2026-08-04: Closure evidence is immutable and complete. Exact-host release CI run
  [`30925896962`](https://github.com/codewandler/flux-connectors/actions/runs/30925896962) is
  terminal green at canonical commit
  `c764f5c3b8e745cc65e90a298b04851647b76778` for the complete native
  `aarch64-apple-darwin`, `x86_64-apple-darwin`, `aarch64-unknown-linux-gnu`,
  `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-msvc` jobs; each asserted its runtime host and
  ran the durable backend suite. The same run's workspace, publication-closure dry run and both
  consumer gates are green.
- 2026-08-04: Tag and release
  [`v0.20.0`](https://github.com/codewandler/flux-connectors/releases/tag/v0.20.0) point to that
  canonical commit; tag-triggered crates.io run
  [`30927493484`](https://github.com/codewandler/flux-connectors/actions/runs/30927493484) is terminal
  green. Fresh registry downloads measured SHA-256
  `bdee7fb0d488de4ed97dbd3b8414e04138c122ee36b6f9c97a174bb317913d8c` (address),
  `9a7737659b74876b09ff6e09b253402c5bdfcafcbde89373cb76f689bd8ffed2` (catalog),
  `edf98bece86f6364aba3e7dd48c3b7e161146942e9e8450d5dc286143b627717` (secrets) and
  `8e858a844dab8324d42bb83c98c4ffb6823681eb1157ddb96a79d5d7a42cff48` (pack). Immutable Exchange
  adoption checkpoint `bd040b9ae5c53454c8df21fd720f8272398cd7c6` has exactly one registry
  `codewandler-connector-secrets 0.20.0` lock record with the released secrets checksum and composes
  one retained store through `Arc<dyn PreparedSecretStore>`, recovery before readiness and the
  closed five-method port without a copied credential schema or point-write emulation. This is
  C-515 downstream-consumption evidence only; it does not claim Exchange X-134 complete or merged.
- 2026-08-04: Failing-first `cargo test -p codewandler-connector-secrets --test
  prepared_transactions` failed to compile because the prepared transaction types and methods did
  not exist. After implementation, `cargo test -p codewandler-connector-secrets --no-fail-fast`
  measured 43 passed/2 deliberately ignored unit tests, 25 passed integration tests and 5 passed
  doctests. The suite includes the child-crash matrix, two-process lease proof, mixed-version
  fixture, concurrent FileStore cross-id refusals/tombstone survival, encoded-sentinel rendering
  audit and retired-fence parser/encoder adversarial cases.
- 2026-08-04: Implementation and local evidence are complete. The native-CI and publication
  acceptance rows remain deliberately open: the implementation PR must first produce all five host
  triples green; the roadmap coordinator then cuts from clean canonical main with
  `scripts/cut-release.sh minor`, pushes the tag, verifies crates.io bytes/checksum and records
  Exchange X-134 resolving the registry release before closing this story.
- 2026-08-04: Filed after the X-134 contract audit proved that an Exchange-owned SQLite credential
  schema would contradict Decision 0004 and that released `connector-secrets` 0.19.1 intentionally
  keeps `SecretBatch::operations` and `Mutation` crate-private. Existing atomic `apply` cannot by
  itself distinguish a crash before versus after a cross-store commit decision without persisting
  secret values in the coordinator.
- 2026-08-04: Final contract audit found that four methods could not express bounded reclamation,
  prepare did not guarantee post-decision commit convergence, FileStore did not enforce its
  single-writer assumption and abort could resurrect a delayed prepare. Roadmap authority `daf80d5`
  closes those gaps with generation fences, terminal tombstones, one prepared reservation and a
  lifetime native lease.

## Notes

- Cross-repository authority:
  `../flux-roadmap/decisions/0004-flux-manages-a-verified-local-exchange.md` and
  `../flux-roadmap/decisions/0007-local-onboarding-uses-owner-bound-capabilities.md` at roadmap
  commit `daf80d5`.
- Exchange owns its value-free transaction journal, metadata roll-forward, connection publication,
  audit outbox and receipt. This story owns only the credential-side prepared transaction and never
  learns a connector label, setting, grant, Service Account, Exchange receipt or lifecycle concept.
- C-509's existing portable owner-only store remains complete; C-515 extends its public host port
  without reopening platform ownership or permitting another credential schema.
