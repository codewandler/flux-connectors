---
id: C-159
title: "The bigger plaintext exposure is `Request`'s derived Debug, and a query credential does not travel as registered"
pillar: Bridge
status: ready
priority: 2
design: docs/designs/connector-tool-pack.md
epic: authentication-surface
areas: [bridge]
note: "found by C-152's review. C-152 hardened Assembled's Debug — but Request is pub, derives Debug over headers and url, and carries the plaintext AFTER auth::place. That is the larger of the two exposures"
---

# The bigger plaintext exposure is `Request`'s derived Debug, and a query credential does not travel as registered

## Goal

Close the two things an independent review of [C-152](C-152-redaction-guarantee-has-holes.md) found
*while* it was closing the smaller version of the same class.

## 1 · `Request` derives `Debug` over the plaintext credential

`crates/connector-pack/src/request.rs:70` — `Request` is **`pub`** and derives `Debug` over `headers`
and `url`, both of which carry the plaintext credential **after `auth::place` has run**.

C-152 hand-wrote a redacting `Debug` for `auth::Assembled` for exactly this reason. The reviewer's
observation is that **`Assembled`'s exposure is the smaller of the two**: `Assembled` is constructed at
one internal site and never escapes, while `Request` is public API a host can hold and format.

Nothing formats it today. That was also true of `Assembled` when C-152 hardened it.

- [ ] `Request`'s `Debug` redacts the credential-bearing parts, matching `Assembled` and
      `connector_secrets::Secret`.
- [ ] **Failing-first test:** `{:?}` on a `Request` carrying a sentinel prints the sentinel today.
- [ ] Whatever a `Request` still prints stays useful for debugging — method, host, path, header
      *names*. A `Debug` that prints nothing is its own kind of defect.

## 2 · A query-placed credential does not travel as the string that was registered

`crates/connector-pack/src/auth.rs:137-141` pushes `query_encode(&assembled.value)`, and `query_encode`
escapes `+ / =` (`:178-189`). So for `Placement::Query`, **the string that travels is not the string
registered with the redactor** — and a base64 credential is exactly the case that contains `+ / =`.

`credentials.rs:333` now says *"Every value this pack puts on a request goes through here."* That is
true of the door, and not true of the bytes. **It is the same class of overclaim C-152 exists to
remove**, introduced by the sentence that closed it.

**Unreachable today**: the committed catalogue is 18 `Placement::Header` and 2 `Placement::Inbound`,
**zero** `Placement::Query`. Pre-existing from C-116.

- [ ] Register the **encoded** form as well as the raw one for a query placement, or refuse query
      placement until it is, or restate the claim precisely. Decide, and record it in the design.
- [ ] **Failing-first test:** a query-placed base64 credential containing `+ / =` survives redaction
      today.
- [ ] The prose in `credentials.rs` matches whatever holds after the fix.

## 3 · Make pack registration idempotent

`add_secret` pushes a **duplicate on every resolve** (pre-existing in flux), and `redact` is linear in
the registered set. The reviewer measured it rather than guessing: **1.6µs at 1 value, 215µs at 1k,
2.3ms at 10k, 23ms at 100k** for a 35-character input.

Its judgement was that this does **not** need bounding for C-152 — `Executor::dispatch` already runs the
same O(n) walk over every tool result and progress line, on much larger strings, so the probe adds a
constant factor to a cost the host already pays per call. But a host running tens of thousands of
credentialed calls in one process grows the set by one or two entries per call.

- [ ] Registration is idempotent per `(CredentialRef, value)` within a pack, so a long-lived process
      does not accumulate duplicates.
- [ ] A test asserts the registered set does not grow across repeated resolves of the same credential.

## Notes

- **How long one `Redactor` lives is the open question** and it is not answerable from this repo.
  `ExecutionEnvironment::new` constructs `Redactor::new()` per environment
  (flux-runtime 0.39.0 `lib.rs:2436`) and the `Arc<Mutex<Vec<String>>>` is shared across clones
  (flux-secret `lib.rs:181`) — but whether that is per-turn or per-process is decided by a binding in
  the flux repo. A host-side count of registrations per session would settle whether item 3 is
  housekeeping or load-bearing.
- One curiosity the reviewer measured and it needs no action: `register` refuses a value the redactor
  *does* hold, if that value's trimmed form is literally `[redacted]`. Fail-closed, absurd input.
- The refusal message says "a value that short" without naming the threshold. Naming it leaks nothing
  the error's existence does not already imply — the threshold is flux's property, not the value's —
  and would tell an operator what to meet. Arguable; deferred rather than decided.
