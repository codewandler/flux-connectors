---
id: C-159
title: "The bigger plaintext exposure is `Request`'s derived Debug, and a query credential does not travel as registered"
pillar: Bridge
status: done
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

- [x] `Request`'s `Debug` redacts the credential-bearing parts, matching `Assembled` and
      `connector_secrets::Secret`.
- [x] **Failing-first test:** `{:?}` on a `Request` carrying a sentinel prints the sentinel today.
- [x] Whatever a `Request` still prints stays useful for debugging — method, host, path, header
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

- [x] Register the **encoded** form as well as the raw one for a query placement, or refuse query
      placement until it is, or restate the claim precisely. Decide, and record it in the design.
- [x] **Failing-first test:** a query-placed base64 credential containing `+ / =` survives redaction
      today.
- [x] The prose in `credentials.rs` matches whatever holds after the fix.

## 3 · Make pack registration idempotent

`add_secret` pushes a **duplicate on every resolve** (pre-existing in flux), and `redact` is linear in
the registered set. The reviewer measured it rather than guessing: **1.6µs at 1 value, 215µs at 1k,
2.3ms at 10k, 23ms at 100k** for a 35-character input.

Its judgement was that this does **not** need bounding for C-152 — `Executor::dispatch` already runs the
same O(n) walk over every tool result and progress line, on much larger strings, so the probe adds a
constant factor to a cost the host already pays per call. But a host running tens of thousands of
credentialed calls in one process grows the set by one or two entries per call.

- [x] Registration is idempotent per `(CredentialRef, value)` within a pack, so a long-lived process
      does not accumulate duplicates. **Keyed on the value alone, and verified against the redactor
      in hand rather than remembered** — see Progress for why that is the stronger reading.
- [x] A test asserts the registered set does not grow across repeated resolves of the same credential.

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

## Progress

**2026-07-31 — all three findings closed.** Design record:
[`connector-tool-pack.md` § "What travels is not always what was resolved (C-159)"](../designs/connector-tool-pack.md).

**1 · `Request`'s `Debug`** (`crates/connector-pack/src/request.rs`). The derive is gone; the
hand-written impl prints **shape without values**: method, host, path, header *names* and
query-parameter *names* stay, every value is `<redacted>`, and the body prints as present or absent —
never as content, and never as a length, because a length is a fingerprint. No allow-list of "safe"
header names: a request cannot know which header holds the credential, and such a list rots into a
leak the first time a vendor puts a token somewhere new. Pinned by
`request::tests::a_request_prints_its_shape_and_none_of_its_values` and
`a_request_with_no_query_and_no_body_prints_both_plainly`.

**2 · The query-placement divergence** (`src/auth.rs`, `src/credentials.rs`). **Registering the
encoded form won** over refusing the placement or restating the claim — the reasoning is in the
design. The structural half is `auth::placed_form`, the single answer to "does this placement
*transform* the value or only *surround* it": `place` writes what it returns and
`credentials::resolve_mechanism` registers it, so the wire form and the registered form cannot be
derived apart, and the match over `Placement` is exhaustive so a placement added later has to state
its answer. `register`'s prose is restated: every string this pack puts on a request either goes
through it or *contains* one that did. Pinned by
`credentials::tests::a_query_placed_credential_registers_the_form_that_travels` (which doctors slack's
placement, because the catalogue still declares zero query placements) and
`auth::tests::a_query_placement_travels_as_the_form_that_is_registered`.

**3 · Idempotent registration** (`src/credentials.rs`). Implemented as **ask, do not remember**:
`register` puts the question to the redactor in hand and calls `add_secret` only for a value it does
not already hold. This deviates from the stated `(CredentialRef, value)` key deliberately, and it is
the stronger reading — a memo on this side would be a memory of some *earlier* redactor, and this
story's own Notes record that whether a `Redactor` outlives a turn is decided in flux, not here. A
remembered registration against a redactor that never received the value is precisely a credential
travelling unheld. Keying on the value also dedupes across two credentials that share one, which an
address key cannot.

`credentials::holds` is where the question is asked *precisely*: `redact(value) != value` is not the
same question, because `redact` also scrubs credential-**shaped** tokens (`sk-ant-…`, `xoxb-…`) it was
never told about — so the naive form would skip registering exactly the values that look most like
credentials. A `\u{1}` prefix defeats the shape pass and leaves the substring pass untouched.

Pinned by `tests/credentials.rs::a_repeated_resolve_does_not_grow_the_registered_set`. The registered
set's size is not observable through flux-secret 1.0.1 (`values` is private, no count), so the test
observes it through the one thing that leaks it — `redact` replaces *each* copy in turn and
`[redacted]` contains `redacted`, so a duplicate nests the marker — with the expectation measured from
a control redactor and an assertion that the probe can tell one registration from two before it
asserts anything else.

**Not done, deliberately:** the two items the Notes defer (a host-side registration count, and naming
the six-character threshold in the refusal). Neither is in the Acceptance.

## Coordinator note at integration (2026-07-31)

Merged after independent review that re-ran the failing-first proof itself, in its own worktree with
its own `CARGO_TARGET_DIR`. All three named tests fail at the merge base `e350ed5` and pass at
`6f4c7e3`, with the base failures showing the actual leak rather than a compile error — the plaintext
sentinel in `url`, in the `Authorization` header, and the encoded form the redactor had never been
told about.

**This closed a defect found independently by C-165's review**, which is the strongest evidence the
story was real: Trello landed the catalogue's first `Placement::Query` credential, and its reviewer
measured that `credentials.rs` registered the raw value while `auth.rs` placed
`query_encode(value)`. For a credential carrying reserved characters those are different strings, so
the redactor held one form and the wire carried another. `auth::placed_form` now registers the form
that travels.

Two residual surfaces recorded rather than closed, both outside this story's scope:

- `Request`'s fields are `pub` and `to_params()` hands the credential back inside a `Debug`-able
  `serde_json::Value` (`request.rs:197`). That is inherent — it is the payload `http.request` is
  invoked with — and no `Debug` on `Request` can close it.
- Whether one `Redactor` lives per turn or per process is decided by a binding in the flux repo, not
  here. The implementation is correct either way because it asks the redactor in hand rather than
  remembering what it registered.

The implementor strengthened the *text* of Acceptance item 3 beyond what was specified (keying the
idempotence check on the value alone, verified against the redactor in hand, rather than on a
`(CredentialRef, value)` memo). The deviation is argued in the story's Progress and in the design
record, and is strictly stronger than the specified behaviour, so it is ticked as met.
