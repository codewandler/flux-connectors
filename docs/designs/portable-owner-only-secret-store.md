# Design: one portable owner-only `FileStore`

**Status:** accepted for C-509 · **Pillar:** Bridge · **Story:**
[C-509](../stories/C-509-portable-owner-only-secret-store.md)

## Boundary

`connector-secrets` owns one durable store behind the existing public `FileStore` and
`SecretStore` API. The logical file bytes, address parsing, bounded whole-file load, in-memory
candidate state and `SecretBatch` transaction stay platform-independent. Only the filesystem
protection and replacement primitives vary by operating system.

This is deliberately not a second store and not an Exchange adapter. A host opens `FileStore`,
passes it as `dyn SecretStore`, and never receives a credential value merely to persist or migrate
it. Windows support therefore cannot be a disabled type, an in-memory fallback or a facade that
accepts a path without enforcing the native protection contract.

## Protection contract

The backend is for one local operator. It is not encryption and it does not protect against
administrator/root access or a backup copied outside these controls.

| platform | newly created state | accepted existing state |
|---|---|---|
| Unix | containing directory `0700`; file and sibling replacement file `0600` | objects owned by the effective process identity, of the expected kind, not symlinks, and with no group/other permission bits |
| Windows | directory, file and sibling replacement file owned by the process token's user SID with a protected DACL | expected non-reparse object kind; owner is that SID; DACL inheritance is disabled; every allowed access entry names only that SID |

Metadata is checked before file contents are read and again on the opened object where the native
API permits it, so a path check is not treated as proof about a later handle. An unsafe or
uninspectable object is refused without chmod, ownership repair, DACL repair, replacement or value
read. Diagnostics name the affected filesystem path, never a credential address or value.

A store directly under a shared directory remains unsafe because its containing directory is not
owner-only. Its refusal tells the operator to create an owner-only child or choose the platform's
conventional per-user state root. It never recommends narrowing the shared ancestor.

## Atomicity and failure

Every mutation renders the complete v1 candidate into a fresh sibling object created with the same
platform-native owner-only protection, flushes its bytes, and uses the operating system's atomic
replacement primitive. The sibling location keeps replacement on one filesystem. The destination
is never opened with truncation.

The in-memory map changes only after replacement succeeds. A failed point write or batch therefore
leaves both the old process view and the old durable file visible; cleanup of an unfinished sibling
is best effort and never turns a failed commit into success. Reopening a new `FileStore` instance is
the proof boundary for restart and first-to-second connection migration behavior.

## Evidence

Native Unix and Windows tests create safe state, widen or replace one protection property at a time,
and assert three things together: open/write refuses before a value is exposed, the diagnostic names
only the filesystem path, and the planted metadata remains equivalent after refusal. Native tests
also drive multi-value restart, an atomic address migration and injected replacement failure.

CI runs the Windows fixtures on a Windows runner. Separate target checks compile the complete public
crate surface for both Apple targets, both Linux targets and `x86_64-pc-windows-msvc`; a cross-check
does not substitute for the native Windows execution.
