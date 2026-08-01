---
id: C-432
title: "A token endpoint is a connector function that marks its response — not an operation we refuse"
pillar: Spec
status: ready
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
- [ ] **The declaration reaches flux in the vocabulary flux reads.** Establish what
      `credential_boundary.rs` actually keys on — `PlatformSourcing`, the manifest shape, the field
      names in `credential_material` — **before** designing the declaration, and report the finding.
      A marking flux does not recognise is worse than none: it reads as safety and the exchange still
      fails.
- [ ] The three artifacts above are corrected in the same change, so the repository does not carry
      two rules. `AGENTS.md` keeps the `authorize`/`revoke` half — those are still never operations —
      and narrows to say that a token exchange is a function whose response is marked.
- [ ] **`unstated` stays distinguishable from `stated`.** An operation that says nothing about its
      response is not the same as one that says the response is safe. This trap has been hit
      independently by C-235, C-408 and C-430; do not hit it a fourth time.
- [ ] Interaction with [C-430](C-430-no-operation-returns-a-secret.md)'s `credential_response` is
      settled, not left to a reader. That declaration exists and *withholds*; this one *marks and
      ships*. Two declarations about the same fact with opposite consequences need one story to say
      which applies when — or to become one declaration with two dispositions.
- [ ] Say whether this restores any of the five operations withheld across v0.9.0 and v0.9.1, and if
      it restores none, say why in the provider files that name it.

## Progress
- (not started)

## Notes
- **Verify against the vendored flux source, not against this story.** `credential_boundary.rs` is at
  `~/.cargo/registry/src/index.crates.io-*/codewandler-flux-plugin-0.47.1/src/host/`. It also carries
  `ACTIVATION_URL_FIELD = "authorize_url"` and an activation-refusal path, which suggests flux has
  already thought about the authorize half — read it before assuming.
- The engine line is already **0.47.1**, the latest published, so no upgrade is owed to get this
  behaviour ([C-431](C-431-move-the-flux-pin-to-0-47.md)).
- C-136's mechanism is not wasted: the diversion, the handle, the store write and its seven refusals
  all stand. What changes is that refusing to *emit* stops being the answer for this shape.
