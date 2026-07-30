---
id: C-152
title: "The redaction guarantee has two holes and one vacuous assertion"
pillar: Bridge
status: in-progress
priority: 2
design: docs/designs/connector-tool-pack.md
epic: authentication-surface
areas: [bridge]
note: "found by C-116's review. flux's Redactor SILENTLY DROPS values under 6 trimmed characters, so a short credential travels unredacted through all four surfaces — and our docs state the guarantee unconditionally. Plus auth::Assembled derives Debug over the plaintext"
---

# The redaction guarantee has two holes and one vacuous assertion

## Goal

Make the redaction guarantee [C-116](C-116-credential-store-port.md) ships actually hold for every
credential, and make its test prove what it claims.

## The four findings, in the order they matter

### 1 · A short credential is never redacted at all

`Redactor::add_secret` **silently drops values under 6 trimmed characters**
(`codewandler-flux-secret-1.0.1/src/lib.rs:195-201`, the version in `Cargo.lock`). So a stored
credential of five characters or fewer travels **unredacted through all four surfaces** — the
`ToolResult` content, its `view`, an error, and a progress line.

C-116's documentation states the redaction property **unconditionally**. That is the defect: the
guarantee as written is not the guarantee that holds.

Options, and this needs a decision rather than a patch:

- **Refuse** a credential too short to be redactable, at resolve time, naming the `CredentialRef`. A
  5-character API token is almost certainly a misconfiguration anyway.
- **Or** state the threshold everywhere the guarantee is stated, and accept it.

Refusing is probably right — a credential the host cannot protect is one it should not send — but it is
a behaviour change and belongs to whoever owns this decision, not to a silent edit.

**Failing-first test:** a 5-character sentinel survives into a `ToolResult` today.

### 2 · `auth::Assembled` derives `Debug` over the plaintext

`crates/connector-pack/src/auth.rs:52-61` — `Assembled { value: String, … }` derives `Debug`, where
`value` is the **assembled plaintext credential** (`Bearer <token>`, or a base64 basic pair).

That is the opposite posture from `connector_secrets::Secret`, whose `Debug` deliberately redacts
(`crates/connector-secrets/src/secret.rs:82`). No call site formats it today — the only references are
construction and typing — so nothing leaks now. It is a foot-gun waiting for the first `{:?}` someone
adds while debugging.

Give it a hand-written `Debug` that redacts, matching `Secret`.

### 3 · One surface of the named test is asserted vacuously

`crates/connector-pack/tests/credentials.rs:157-163` checks the `view` surface — but
`flux_runtime::tool_fn` builds every result with `ToolResult::ok(...)`, which sets `view: None`
(`flux-runtime-0.39.0/src/fn_tool.rs:107`). So `view.as_deref().unwrap_or_default()` is `""` and the
assertion redacts an empty string.

The test as a whole is still genuinely red without registration — the reviewer proved that — so this is
an **overclaim of coverage, not a false pass**. Fix it by constructing a result that carries a `view`,
or drop the assertion and say the surface is unreachable from this call path.

### 4 · A window where the secret is in memory and the redactor has not been told

`credentials.rs`: the store returns the value at **:233**, `user_half` — which is *fallible* — runs at
**:249**, and `add_secret` is at **:254**.

Nothing in that window can surface the value (the error carries only operation, credential and env-key
names, and `Secret`'s `Debug` redacts) and `user_half` is a pure env read, so this is not live. But the
window is closable by registering before the fallible step, and C-116's own acceptance was
"`add_secret` **before** the request is constructed" — closing it makes the code match the rule.

## Acceptance

- [x] Item 1 decided, implemented, and the decision recorded in the design.
- [x] `Assembled`'s `Debug` redacts, asserted by a test.
- [x] The `view` assertion either exercises a real `view` or is honestly removed.
- [x] The registration window is closed.
- [x] `a_credential_never_reaches_a_surface` still passes and still fails when registration is removed.
- [x] The gate is green; the build stays a fixed point.

## Progress

**Item 1 was decided by refusing**, and the decision record is
[the design](../designs/connector-tool-pack.md)'s new section *"The redaction guarantee, and the
condition it turned out to have"* — the options, the three grounds, and the named behaviour change.
`connector_pack::Error::UnredactableCredential` is the refusal; it names the operation, the credential
and the address's tenant and authority, and neither the value nor its length.

The check **asks the redactor rather than mirroring the six**: `credentials::register` registers the
value and then asserts that scrubbing it changes it. A mirrored threshold would go stale on a flux
upgrade with no test noticing, which is the failure `DEFAULT_SERVICE`'s mirror guard exists for. Asking
also covers the empty and all-whitespace cases without naming them.

`register` is the single door every value goes through, which is what closes item 4 as well: it runs
directly after the store answers and *before* the fallible `user_half`, and the assembled Basic blob
goes through the same door on its own terms.

Items 2 and 3 are `auth::Assembled`'s hand-written redacting `Debug`
(`an_assembled_credential_redacts_its_value_the_way_a_secret_does`) and a stand-in egress that answers
with `ToolResult::ok_view`, so the `view` surface is asserted against a real `view` with the same
control the `content` surface always had.

Verified by hand, not by inference: commenting out the `register` call for the stored secret makes
`a_credential_never_reaches_a_surface` fail with *"the sentinel survived into the tool result"*. The
tripwire the reviewer used still trips.

## Notes

- **Item 1 is the one to start with**, and it is a good illustration of why a guarantee should be
  stated with its conditions: the code is correct about what it does, the prose is wrong about what
  that means, and only the prose is what a reader relies on.
- Related, out of lane, and worth its own check: `Error::CredentialStore` embeds `StoreError` as
  `#[source]`, and `Unreachable`/`Denied`/`Backend` carry a free-form `reason`. The pack relies on
  `connector-secrets`' documented contract that a `StoreError` is safe to log. If a future Vault
  transport ever put a response body into `reason`, that error is raised **before** `add_secret`. A
  test in `connector-secrets` should pin the contract.
