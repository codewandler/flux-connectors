---
id: C-207
title: "The host forgets every credential on restart — `MemoryStore` is not a deployment"
pillar: Bridge
status: in-progress
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

- [x] **Failing-first test:** store a credential through the host's HTTP surface, drop and rebuild the
      `App` against the same location, and assert the credential is still resolvable — a test that
      fails against `MemoryStore` today. Name it.
      → `a_credential_survives_the_host_being_rebuilt`
      (`crates/connectors-api/tests/persistence.rs:84`). At the merge base it fails
      `left: (false, …) right: (true, "tenants/google-…/com.anthropic.api/api_key")`.
- [x] The store is selected by configuration, with a default that is safe for a local single-operator
      deployment. The selection is a startup decision that fails loudly on a bad value, never a silent
      fallback to memory — a host that quietly forgets is the failure this story exists to end.
      → `crates/connectors-api/src/secrets.rs`, `StoreChoice::{from_env,parse,open}`;
      `App::deployed` is what `main.rs` calls and defaults to the file store. All four legs are
      pinned in `tests/credential_store.rs`, each against a mutant that was previously green:
      `the_deployed_default_is_a_file_under_the_data_home` (default → `Memory`),
      `a_store_that_will_not_open_refuses_and_does_not_become_memory` (open failure → `MemoryStore`),
      `a_store_it_does_not_have_stops_the_host` and
      `a_bad_value_in_the_environment_refuses_to_build_a_host` (an unparseable value).
      `an_ambient_store_variable_does_not_reach_a_host_built_with_new` pins the other half of the
      rule — **exactly one constructor reads the environment** — because when `App::new` read it too,
      an operator with the variable exported could not run `cargo test` without the suite writing
      their real store and sending a live request to `api.anthropic.com`.
- [x] The persisted location is **outside the repository checkout** and outside any directory served
      by the host. Asserted, not assumed.
      → `secrets::refuse_inside` (`src/secrets.rs:213`), called from `App::build` *before* the file
      is created; `a_store_inside_the_hosts_own_root_is_refused_before_it_is_created`.
- [x] File permissions are `0600` and the containing directory `0700`, set at creation rather than
      fixed up afterwards, and a test asserts the mode on a freshly created store.
      → `OpenOptions::mode` / `DirBuilder::mode` in `crates/connector-secrets/src/file.rs`;
      `a_fresh_store_is_0600_inside_a_0700_directory` there, and
      `the_store_the_host_creates_is_0600_in_a_0700_directory` through the host's own HTTP surface.
      A widened existing store is **refused, not tightened** — file and directory separately
      (`a_world_readable_store_is_refused`, `a_world_readable_directory_is_refused`; the second
      exists because deleting the directory leg left every file assertion green).
- [x] A write is atomic — no path through the code leaves a truncated or partially-written store on a
      crash mid-write.
      → `FileStore::write_temporary`: `create_new` sibling at `0600` → `write_all` → `sync_all` →
      `rename(2)` → directory `fsync`. A write that fails rolls the in-memory map back, so nothing is
      resolvable that is not on disk. Pinned two ways, because the first way was not enough:
      `a_concurrent_reader_never_sees_a_half_written_store` reads *while* a writer rewrites, which is
      what `a_write_leaves_no_temporary_and_no_truncated_file` only appeared to do — it opened the
      store after the writes finished, so a truncate-then-write implementation left it green. The
      three legs no single-threaded test can observe (`create_new`, the temporary's `sync_all`, the
      directory `fsync`) are held by `the_write_path_keeps_all_three_legs_of_its_atomicity`, a source
      audit in the style of `Secret::every_exit_from_the_wrapper_is_named_expose_secret`.
      **A crash between the write and the rename leaves a full copy of every credential**, so
      `FileStore::open` reaps stale temporaries (`a_temporary_left_by_a_crash_is_reaped_on_the_next_open`)
      and the documented revoke is `rm -r` on the directory, not `rm` on the file.
- [x] **The redaction guarantee is unchanged and re-proved.** `crates/connectors-api/tests/host.rs`
      already asserts no credential value reaches any served surface; extend it so that a credential
      loaded *from the persisted store* is held to the same property, including on error paths.
      → `a_credential_loaded_from_disk_reaches_no_surface` (`tests/persistence.rs:131`) runs the same
      sweep against a host whose only knowledge of the value is the file it read at startup, and
      asserts the reload happened first so it cannot pass vacuously. `tests/host.rs` is untouched.
- [x] No credential value reaches a log line, a `Debug` rendering, or a startup diagnostic naming the
      store. [C-159](C-159-request-debug-and-query-encoding.md) is the precedent for how a derived
      `Debug` becomes the leak.
      → `FileStore` has a hand-written `Debug` (path + count, not even an address);
      `debug_carries_neither_a_value_nor_an_address`. Parse failures name a line number and a
      length, never content (`a_parse_failure_names_the_line_and_never_the_value`). Write failures
      carry only an errno and a path (`a_write_failure_names_no_value`, and
      `a_persistence_failure_refuses_without_quoting_the_value` over HTTP). The banner is built from
      a `StoreChoice`, which holds a path and nothing else.
- [x] `crates/connectors-api/README.md` states where the store lives, how to point it elsewhere, and
      how an operator destroys it — a credential store with no documented delete is one an operator
      cannot revoke.
      → §"Where the credentials live", plus §"The restart, performed and labelled" with the
      hand-driven transcript. `rm <path>` and `DELETE /v1/credentials/…` are both given, and the
      `DELETE`-survives-a-restart case is asserted in `a_credential_loaded_from_disk_reaches_no_surface`.

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

## Progress

**2026-07-31 — implemented on `impl/C-207`.** Gate green: `cargo build --workspace`,
`cargo test --workspace --no-fail-fast` (**1273 passed, 0 failed** across 135 result lines),
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.

What landed, and where a resuming agent should look first:

- **`crates/connector-secrets/src/file.rs` — `FileStore`.** The store itself, in the crate that
  already owns `SecretStore`/`MemoryStore`/`VaultStore` rather than in a second seam. No new
  dependency: `std` only, so the hex codec and the atomic write are hand-written. Unix-only and
  re-exported `#[cfg(unix)]` from `connector-pack`, because a file mode is the whole of its security
  argument and a platform without one would get a store that implied safety it did not have.
- **`crates/connectors-api/src/secrets.rs` — `StoreChoice`.** The *choice*, which is the deployment
  decision the port never captured. `CONNECTORS_CREDENTIAL_STORE` is `file` | `file:<absolute>` |
  `memory`; anything else stops the host.
- **Exactly one constructor reads the environment, and it is `App::deployed`** — the one `main.rs`
  calls. Everything else takes its store as an argument or does not persist.

### Rework round 1 — what the review found, and what changed

Three findings, each now pinned by a test that was measured red against the mutant that produced it.

1. **An ambient `CONNECTORS_CREDENTIAL_STORE` reached every test host.** `App::new` honoured the
   variable as well, and `App::new` is what the suite builds hosts with. With it exported — which
   this crate's README instructs — `tests/host.rs`'s
   `an_operation_without_its_credential_refuses_by_address` answered **200 instead of 400**, meaning
   the suite *dispatched* and sent a live request to `api.anthropic.com` using a value an earlier run
   had left in the operator's real file. Fixed by removing the ambient read entirely rather than by
   teaching the harness to clear the variable: the test files that would have needed the clearing are
   fenced, and a rule the harness has to remember is the weaker fix. Pinned by
   `an_ambient_store_variable_does_not_reach_a_host_built_with_new`.
2. **"There is no silent fallback to memory" was unfalsifiable on the open path.** Only `parse`
   failures were pinned; making `StoreChoice::open` answer a failed `FileStore::open` with a
   `MemoryStore` left the whole suite green. Pinned by
   `a_store_that_will_not_open_refuses_and_does_not_become_memory`, at all three layers that could
   swallow it.
3. **`App::deployed`'s default was exercised by nothing.** Its only test reached it inside the `Err`
   branch of `default_path()`, unreachable on a machine with `HOME` set — while this story's own
   acceptance cited that line as its evidence. Pinned by
   `the_deployed_default_is_a_file_under_the_data_home`, which drives `App::deployed` against a
   scratch `XDG_DATA_HOME` and asserts on the file that appears. `tests/persistence.rs` now builds
   every host the same way, so the shipped default is what all four of those tests exercise.

**The general lesson, which is the one worth keeping.** All three were tests that named a subject
they did not touch, and the third had been promoted into the acceptance checklist as proof. The
pattern is specific: each asserted something *near* the behaviour — a parser rather than the
constructor that calls it, a constant rather than the file it governs, an equality between two
functions rather than the one under test. The instrument that catches it is not more tests but the
mutation: change the line the test claims to defend, and require the test to go red. Every item above
was verified that way, and three more were added for the same reason (`create_new`/`fsync`/directory
`fsync` are held by a source audit; the truncation window by a concurrent reader, because
`a_write_leaves_no_temporary_and_no_truncated_file` opened the store *after* the writes finished and
stayed green under a truncate-then-write implementation).

Minors also addressed: `FileStore::open` reaps a crashed write's temporary (which held a full copy of
every credential and made `rm <file>` a revoke that did not revoke — the documented delete is now
`rm -r` on the directory); the `v1` header is checked rather than skipped; the directory mode check
has its own test; write errors name the store rather than the internal temporary path.

Open, and deliberately not done here:

- **Encryption at rest.** There is none, and every operator-facing surface says so in those words.
  A deployment that is not one operator's laptop wants `VaultStore`; widening the bind (see
  `docs/designs/connectors-api.md` §"The bind") should not be argued on the strength of this store.
- **Cross-process writers.** One process, stated in the module docs. Two hosts on one file will
  overwrite each other's last write, and the temporary-reaping above assumes it.
- **`api.rs` puts the store's filesystem path in a `502` served to any authenticated tenant.** No
  value, so redaction holds; it is a layout disclosure. Left as is — it is the diagnostic that makes
  a store failure actionable on a loopback-only single-operator host — but `api.rs` is C-212's and
  the call is worth revisiting when the bind widens.
- **Sessions still die with the process**, which is correct and is not changed here.
