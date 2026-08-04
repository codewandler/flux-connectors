# Recoverable prepared secret transactions

This is the provider-owned contract for C-515. It extends `connector-secrets` without exposing a
credential value, private `Mutation`, staged path or second credential schema to Exchange. Roadmap
Decisions 0004 and 0007 at `daf80d5` own the cross-repository recovery boundary.

## Public API

The additive object-safe API is equivalent to:

```rust
pub struct SecretTransactionGeneration(NonZeroU64);
pub struct SecretTransactionId([u8; 32]);
pub struct SecretProposalDigest([u8; 32]);

pub enum SecretTransactionState {
    Absent,
    Prepared,
    Committed,
}

#[async_trait]
pub trait PreparedSecretStore: SecretStore {
    async fn prepare(
        &self,
        id: SecretTransactionId,
        digest: SecretProposalDigest,
        batch: &SecretBatch,
    ) -> Result<SecretTransactionState, PreparedSecretError>;
    async fn state(
        &self,
        id: SecretTransactionId,
    ) -> Result<SecretTransactionState, PreparedSecretError>;
    async fn commit(
        &self,
        id: SecretTransactionId,
    ) -> Result<SecretTransactionState, PreparedSecretError>;
    async fn abort(
        &self,
        id: SecretTransactionId,
    ) -> Result<SecretTransactionState, PreparedSecretError>;
    async fn reclaim(
        &self,
        through: SecretTransactionGeneration,
    ) -> Result<(), PreparedSecretError>;
}
```

There are no generic methods, callbacks, associated return types, borrowed results or backend
handles, so `Arc<dyn PreparedSecretStore>` is valid.

The first eight id bytes are one non-zero big-endian reclamation generation and the remaining 24
bytes are a coordinator-unique opaque nonce. The provider API constructs an id from those two typed
inputs and exposes the complete 32 bytes only for protocol encoding. The complete id has no public
ordering, `Display`, serde derive or numeric transaction API. Generation wrap refuses.

`SecretProposalDigest` is exactly 32 caller-supplied bytes. `connector-secrets` neither computes it
nor interprets its domain. Exchange owns the Decision 0007 SHA-256 proposal digest.

The new error type preserves the existing exhaustively matchable `StoreError`:

```rust
pub enum PreparedSecretError {
    Unsupported,
    Busy,
    DigestMismatch,
    TransactionIdReused,
    NotPrepared,
    AlreadyCommitted,
    Retired,
    Capacity,
    InvalidBatch,
    Backend,
}
```

No variant carries an id, generation, digest, scope, address, mutation count, stage path or
secret-derived fact. Provider implementations sanitize `StoreError` internally to the unit
`Backend` variant; the existing `SecretStore` methods and exhaustively matchable `StoreError` remain
unchanged. A transaction-method I/O failure is outcome-uncertain until `state` resolves it.

## State machine

The store persists four internal conditions but exposes only three states. An aborted tombstone is
reported as `Absent`; a reclaimed generation returns `Retired` rather than a state.

| Durable condition | `state` | `prepare(id, D, batch)` | `commit` | `abort` |
|---|---|---|---|---|
| unseen, generation above fence | `Absent` | validate and enter `Prepared` | `NotPrepared` | record aborted tombstone, return `Absent` |
| prepared with digest D | `Prepared` | same D: `Prepared`; other: `DigestMismatch` | publish once, return `Committed` | tombstone and remove stage, return `Absent` |
| committed with digest D | `Committed` | same D: `Committed`; other: `DigestMismatch` | `Committed` | `AlreadyCommitted` |
| aborted tombstone | `Absent` | `TransactionIdReused` | `TransactionIdReused` | `Absent` |
| id generation at/below fence | `Retired` | `Retired` | `Retired` | `Retired` |

Same-digest prepare replay never inspects or compares the new batch. Commit and abort serialize. If
commit wins, abort observes committed; if abort wins, commit observes the tombstone. Abort of an
unseen id deliberately writes a tombstone: a delayed prepare can therefore never resurrect work the
coordinator already abandoned.

The table assumes no different transaction occupies the prepared slot. While id T1 is prepared,
`abort(T2)` for any T2 whose terminal condition would require a live-ledger rewrite returns `Busy`
without mutation. Otherwise T1's older immutable staged image could later replace the live file and
erase T2's newly acknowledged tombstone. `state(T2)` remains available; `commit(T2)` remains a
non-mutating lookup/error; same-id `abort(T1)` remains the transition above.

## Validation and reservation

One store has one global prepared slot. Before returning `Prepared`, the backend applies the private
mutation sequence to a cloned committed snapshot and validates:

- scope and duplicate-address rules;
- every move source and destination precondition;
- entry, value, file and terminal-ledger capacity;
- the current/live prepared encoding and complete committed-next-image encoding;
- safe owner, kind, link/reparse and root metadata for every touched object.

It then stages the complete already-validated next store image. While prepared, another prepare,
`put`, `delete`, `apply` and `reclaim` refuse. Prepared-port methods return payload-free `Busy`.
Because the inherited ordinary mutation methods retain their existing `StoreError` return type,
`put`, `delete` and `apply` return `StoreError::Conflict` carrying only the already-supported store
path and a fixed value-free reason that the prepared slot owns mutation; they do not return a new
`StoreError` variant. `get`, `references` and `state` remain available and see the old committed
image. Commit performs no mutation interpretation or merge and therefore cannot discover a
deterministic conflict after Exchange records its decision. A transient I/O or unsafe-metadata
refusal remains retryable; it does not authorize abort after the coordinator decision.

The single slot deliberately rejects disjoint concurrent prepared batches. Address reservations
would create a larger durable model without helping the one Exchange coordinator.

## Bounded terminal retirement

An opaque random id is not an ordered floor. `reclaim(G)` instead acknowledges that Exchange will
never query or replay any id in generations through `G`:

- reclaim refuses while any transaction is prepared;
- `G` at or below the current fence is idempotent success;
- one atomic live-file rewrite advances the inclusive fence and removes committed and aborted
  tombstones in those generations;
- every future operation for those generations returns `Retired`, preventing id resurrection;
- credential entries are never selected or deleted by reclamation.

Exchange persists generation allocation in its value-free coordinator root, rotates only after all
ids in the old generation are terminal and never wraps. The provider retains at most 4096 terminal
records and remains within the existing 1 MiB file bound. At capacity, prepare returns `Capacity`;
it never evicts. Failure to acknowledge retirement therefore loses onboarding availability rather
than recovery evidence or unbounded disk.

## FileStore v2 and lifetime lease

`FileStore::open` acquires a fixed sibling lease before reading, recovering or cleaning anything and
holds the handle for its lifetime. Unix accepts only an owner-UID, one-link regular `0600` lease and
uses an exclusive kernel file lock. Windows accepts only current-`TokenUser` ownership, a protected
current-user/System DACL, regular non-reparse metadata and an exclusive `LockFileEx` range. Another
0.20 opener/process returns a value-free conflict. Abrupt exit releases the kernel lease. The lease
is never repaired, replaced or reaped.

Released 0.19.1 predates this lease and therefore cannot be made to honor it. Operators must stop
every 0.19.1 writer before the first 0.20 open, and concurrent mixed-version writers are explicitly
unsupported. Once a transaction has written v2, a newly opened 0.19.1 store refuses the version;
that refusal does not retroactively make a legacy process that opened v1 safe to keep running.

The live v2 file atomically contains:

```text
# codewandler-connector-secrets file store, v2
# retired-through <16 lowercase hex generation>
# transaction <64 lowercase hex id> prepared <64 lowercase hex digest>
# transaction <64 lowercase hex id> committed <64 lowercase hex digest>
# transaction <64 lowercase hex id> aborted
<credential-address> <hex secret>
```

There is exactly one record form per durable state; abort-before-prepare is digestless because
`abort(id)` receives no digest. There are zero or more transaction records, each unique by id and
sorted lexicographically by its raw 32 id bytes, with at most one prepared record. Record ordering is
an internal canonical encoding and does not add public `Ord` to the opaque id. Credential entries
follow all transaction records and retain the exact canonical v1 address-plus-lowercase-hex encoding
and ordering. The concrete implementation may use an equivalent byte layout only if these
cardinalities, exact comparator, ordering, bounds, state coupling and old-reader refusal are
fixture-pinned.
The fixed staging filename contains no id/digest in its name and stores one complete owner-only v2
next image whose selected transaction is committed and whose other terminal records and fence match
the live prepared image.

A clean v1 file opens as an empty ledger without eager migration after the required legacy-writer
quiescence. Existing reads and ordinary apply remain compatible. First transaction use atomically
writes v2; a fresh 0.19.1 open then hits its existing version check and refuses rather than erasing
transaction metadata.

### Prepare

1. Validate current and prospective images completely.
2. Write, flush and atomically publish the fixed committed-next-image stage; sync its directory.
3. Rewrite the live file with unchanged credentials plus `Prepared`; sync its directory.
4. Return only after the live prepared record is durable.

### Commit

1. Revalidate the live/stage handles and their exact id/digest relationship.
2. Copy the immutable stage through a fresh protected temporary and flush it.
3. Atomically replace the live file. This single replacement publishes the complete credential image
   and `Committed` record together.
4. Sync the directory, remove the original stage and sync again. A matching leftover stage beside
   committed live state is cleanup-only.

### Abort

For prepared or unseen ids above the fence, atomically rewrite unchanged live credentials with an
aborted tombstone and sync. Then remove a matching stage, if any, and sync again. A crash before live
replacement leaves the prior public condition; after replacement it is durably absent and fenced.

### Open recovery

- live `Prepared` requires a complete safe matching stage or open refuses;
- live matching `Committed`/aborted plus stage means cleanup was interrupted and deletes the stage;
- no live record plus a valid stage means prepare crashed before publication and deletes the stage;
- unsafe, oversized, mismatched, unparseable or undeletable state refuses without repair.

Recovery exposes only absent with old credentials, prepared with old credentials and a complete
invisible candidate, or committed with the complete new image.

## Debug and value ownership

Exchange transiently receives vendor bytes and constructs `SecretBatch`; connector-secrets owns the
only durable credential and prepared representation after successful prepare. The API never claims
that the provider owned bytes before the call.

Existing `Secret` keeps `Secret(<redacted>)`. Manual public renderings are exactly opaque in kind:

```text
SecretBatch(<opaque>)
MemoryStore(<opaque>)
FileStore(<opaque>)
SecretTransactionGeneration(<opaque>)
SecretTransactionId(<opaque>)
SecretProposalDigest(<opaque>)
```

They reveal no scope, address, operation, count, generation, id, digest, path, value or value length.
The new types implement neither `Display` nor serde. Internally computing bounded encoded byte sizes
is permitted; returning, logging, identifying by or persisting secret-derived metadata outside the
credential sink is not.

## Native crash evidence

Every native target job asserts that `rustc -vV` host equals its matrix target, spawns the test
executable in a child mode, terminates it without unwinding at each durable failpoint and recovers in
a fresh process. Prepare, commit, abort and reclaim failpoints cover stage/live create, complete
write, file flush, atomic replacement and cleanup. Unix additionally covers every parent-directory
sync. Windows instead covers `FlushFileBuffers`, `ReplaceFileW` or write-through `MoveFileExW`,
post-replacement handle validation and cleanup; it does not invent a directory-fsync primitive the
platform lacks. Reclaim recovery observes either the complete old fence/ledger or the complete new
fence/ledger, never a partially retired generation. Two-child tests prove lease contention and release
after abrupt exit. A cross-id race test proves that an abort which returns success cannot be erased
by another transaction's staged commit. An upgrade fixture demonstrates why an already-open 0.19.1
writer must be quiesced and separately proves that a fresh 0.19.1 open refuses v2. Platform-native
suites retain owner/mode or owner/DACL, link/reparse, wrong-kind, bound and unsafe-root cases.

The exact native set is `aarch64-apple-darwin`, `x86_64-apple-darwin`,
`aarch64-unknown-linux-gnu`, `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-msvc`.
Cross-compilation is supplementary.

## Compatibility and publication

The trait/types are additive and `StoreError` plus existing methods remain unchanged. Manual opaque
Debug retains trait availability. The lifetime lease and v2 on-disk format are a pre-1.0 minor
boundary, so the current 0.19.1 line advances to 0.20.0. Upgrade documentation requires quiescing all
0.19.1 writers before 0.20 first opens the store and explicitly rejects concurrent mixed-version
writers. Completion requires the repository's CI-only publication path, verified crates.io bytes and
Exchange X-134 resolving the registry version and checksum. A path, git or sibling checkout is never
provider evidence.
