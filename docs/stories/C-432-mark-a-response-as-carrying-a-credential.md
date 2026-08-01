---
id: C-432
title: "A token endpoint is a connector function that marks its response — not an operation we refuse"
pillar: Spec
status: in-progress
priority: 1
design: docs/designs/connector-security-posture.md
epic: connector-security-posture
areas: [connector-spec, connector-flux, connector-pack]
note: "OWNER RULING 2026-08-01, superseding the one recorded that morning: a token endpoint SHOULD be a connector function, marked as returning sensitive information. flux 0.47.1's credential_boundary REFUSES such a response outright when it is unmarked, so an unmarked one does not merely leak — the exchange fails"
---

# A token endpoint is a connector function that marks its response — not an operation we refuse

## Goal
Let an operation declare that its response carries sensitive material, so a token exchange can be a
connector function that works — instead of being withheld, or emitted and then failed by flux.

## The ruling, and what it supersedes

Owner-stated 2026-08-01, in these words: *"it should be a connector function! but it needs to be
marked somehow as returning sensitive information — recent changes in flux core will block such
requests entirely as it will detect the response as an auth token and completely fail the exchange."*

That reverses the direction three artifacts currently take, all written the same day and all now
wrong in the same way:

| Artifact | What it says today |
|---|---|
| `AGENTS.md` § Authentication contract | *"An authentication endpoint is never a connector operation"* |
| [C-426](C-426-multipart-body-encoding.md) | withheld babelforce's `/oauth/token` under that rule |
| [C-136](C-136-credential-diversion.md) | made a `produces_credential` operation **refuse to build** |

The rule was right about `authorize` (a browser redirect with no result to return to a program) and
about `revoke` (which takes a `client_secret` as a plain argument). It over-reached on `token`, which
is a real request/response call a program makes and reads.

**flux already implements the other half**, and this is what makes marking necessary rather than
merely tidy. `codewandler-flux-plugin-0.47.1/src/host/credential_boundary.rs` (flux C-312): an
operation opts in by declaring `PlatformSourcing`, and then *"its responses are **refused**, not
merely redacted, when they carry credential-shaped material. Redaction hides a leak from the model;
refusal says the boundary was crossed."* So an unmarked token exchange does not leak — **it fails**.

## Acceptance
- [ ] An operation can declare that its response carries sensitive material, and a token exchange
      **builds and emits** rather than being refused. C-136's `check_credential_diversion` refusal is
      replaced by this, not merely relaxed — a failing-first test emits one and asserts the module.
      → **Not done, and deliberately not attempted.** The acceptance rests on the premise that an
      unmarked exchange *fails* at flux, making the emitter refusal redundant. The finding below
      shows it does not. `check_credential_diversion`'s own justification is independent of flux and
      still holds — an emitted `op` ends `return response` and flux has no credential-store port, so
      removing the refusal would bind a raw token to a model-visible symbol, regressing an invariant
      `AGENTS.md` declares. Removing it on a false premise was the one thing this story must not do.
- [x] **The declaration reaches flux in the vocabulary flux reads.** Establish what
      `credential_boundary.rs` actually keys on — `PlatformSourcing`, the manifest shape, the field
      names in `credential_material` — **before** designing the declaration, and report the finding.
      A marking flux does not recognise is worse than none: it reads as safety and the exchange still
      fails.
      → Established from the vendored source and recorded in `AGENTS.md` § *What flux actually keys
      on*. **The declaration cannot exist**: `PlatformSourcing` is an opt-in to refusal, not a permit,
      and the boundary is on a seam this repository's artifacts never reach.
- [x] The three artifacts above are corrected in the same change, so the repository does not carry
      two rules. `AGENTS.md` keeps the `authorize`/`revoke` half — those are still never operations —
      and narrows to say that a token exchange is a function whose response is marked.
      → `AGENTS.md` § Authentication contract, `C-426` and `C-136` all corrected. The narrowing is
      recorded as *intent the mechanism does not yet support*, rather than as an accomplished rule.
- [x] **`unstated` stays distinguishable from `stated`.** An operation that says nothing about its
      response is not the same as one that says the response is safe. This trap has been hit
      independently by C-235, C-408 and C-430; do not hit it a fourth time.
      → Not hit: no new declaration was introduced, and nothing added lets an operation assert its
      response is *safe*. Silence still means only silence. (A pre-existing collapse in
      `credential_response` — `skip_serializing_if = "Vec::is_empty"` makes "said nothing" and "said
      empty" one encoding — is noted as adjacent, not introduced here.)
- [x] Interaction with [C-430](C-430-no-operation-returns-a-secret.md)'s `credential_response` is
      settled, not left to a reader. That declaration exists and *withholds*; this one *marks and
      ships*. Two declarations about the same fact with opposite consequences need one story to say
      which applies when — or to become one declaration with two dispositions.
      → Settled as **one fact, two dispositions, selected by purpose rather than shape**, and
      enforced: `validate_one_credential_disposition` refuses an operation declaring both and the
      refusal carries the discriminator.
- [x] Say whether this restores any of the five operations withheld across v0.9.0 and v0.9.1, and if
      it restores none, say why in the provider files that name it.
      → **It restores none.** Said in `providers/postmark.toml` and `providers/zoom.toml`. Note the
      count is not five: see *The count is four plus three, not five* below.

## Progress

**Status: partial.** The finding landed and the C-430/C-136 conflict is settled and enforced. The
marking itself was not built, because the mechanism the ruling assumes does not exist.

### The finding: what flux actually keys on

Read from `codewandler-flux-plugin-0.47.1/src/host/credential_boundary.rs` and
`codewandler-flux-plugin-protocol-1.2.0/src/lib.rs:306-319`. Two independent reasons the marking the
ruling asks for cannot be written today:

1. **`PlatformSourcing` is an opt-in to refusal, not a permit.** Three states — `None` (the
   `#[default]`), `Operation`, `Activation`. `refuse_response` returns `None` immediately when
   `platform.is_none()`, and its own test `an_ordinary_op_is_not_subject_to_the_boundary` pins that.
   `Operation` and `Activation` are what *turn refusal on*. There is no fourth state meaning "this
   response carries a credential, allow it". The module's single exemption is `secret.read`, a
   flux-internal host op reached through `EndpointBroker::resolve_credential_for`, which no manifest
   can declare. **Marking a token exchange in flux's own vocabulary would cause the refusal the
   owner wants to avoid.**
2. **The boundary is on the plugin seam, and this repository is not on it.** `refuse_response` is
   applied to a plugin `OperationSpec` response arriving over the NDJSON plugin protocol. What this
   repository emits is a `.flux` module and a `<connector>.connector.toml` whose serialized struct
   (`crates/connector-cli/src/seam.rs`) has no `platform`, `secret_purposes`, `redact_fields` or
   `reaches` field. `credential_boundary.rs`'s own header says the check is *"a no-op"* on every
   plugin in this repository.

So the premise in the `note:` frontmatter above — *"flux 0.47.1's credential_boundary REFUSES such a
response outright when it is unmarked"* — **is not true of this repository's operations.** An
unmarked token exchange would not fail at flux. `AGENTS.md` § *What flux actually keys on* carries
this so the next reader hits it before designing a marking.

### What was built

- `validate_one_credential_disposition` (`crates/connector-spec/src/provider.rs`) refuses an
  operation declaring both `credential_response` and `produces_credential`, and the refusal carries
  the discriminator: **purpose, not shape**. Both per-field validators are skipped when it fires, so
  exactly one disposition is stated — before this, the loader rendered C-430's *"Withhold the
  operation"* beside a declaration whose whole meaning is that the operation ships.
- Both fields' doc comments in `ir.rs` state the exclusion and the rule for choosing.
- `AGENTS.md`, `C-426` and `C-136` reconciled to one rule.

### What was not built, and why

`check_credential_diversion` **stands**. Its justification was never flux's boundary — it is that an
emitted `op` ends `return response`, flux holds no handle on the credential store, and so a module
carrying a login binds the raw token to a model-visible symbol. Nothing in the finding touches that.
Removing it would have regressed a declared safety invariant on the strength of a premise that does
not hold, which is precisely the *"marking that reads as safety while changing nothing"* the
acceptance warns against, in its most damaging form.

**What a follow-up owes.** A token exchange becomes shippable only with a mechanism nobody has
built: either a credential-store port on the flux side, or an operation that lives on the
`connector-pack` path without being emitted into a module. C-136 rejected the second because
`emit_operation` produces **one** rendering fed to the module, the per-operation `.flux` and
`web/public/catalog.json` alike — that argument is the one a follow-up must reopen first, since it
is the shape the owner's ruling wants.

### The count is four plus three, not five

The story asks about "the five operations withheld across v0.9.0 and v0.9.1". No artifact records
five. The repository records **four plus three**:

- **Four** withheld under C-430's `credential_response`, all carrying a credential *incidentally*:
  `postmark-server-list`, `postmark-server-get`, `zoom-meeting-get`, `zoom-meeting-create`. Untouched
  by this story — they are C-79's, and the purpose-versus-incidental rule settled here confirms it.
- **Three** babelforce `/oauth/*` endpoints withheld under the authentication-endpoint rule.
  `authorize` and `revoke` keep their original grounds unchanged. `/oauth/token` is the only
  operation the ruling narrows — and it stays withheld, for the reason above.

**So this restores none of them,** and the counts in `C-426` do not move. Said in
`providers/postmark.toml` and `providers/zoom.toml`. (C-136's own notes already say *"four plus one
rather than five"*, which is the likely origin of the story's five.)

## Notes
- **Verify against the vendored flux source, not against this story.** `credential_boundary.rs` is at
  `~/.cargo/registry/src/index.crates.io-*/codewandler-flux-plugin-0.47.1/src/host/`. It also carries
  `ACTIVATION_URL_FIELD = "authorize_url"` and an activation-refusal path, which suggests flux has
  already thought about the authorize half — read it before assuming.
- The engine line is already **0.47.1**, the latest published, so no upgrade is owed to get this
  behaviour ([C-431](C-431-move-the-flux-pin-to-0-47.md)).
- C-136's mechanism is not wasted: the diversion, the handle, the store write and its seven refusals
  all stand. What changes is that refusing to *emit* stops being the answer for this shape.
