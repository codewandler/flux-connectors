---
id: C-509
title: "Persist connector secrets owner-only on every Flux platform"
pillar: Bridge
status: in-progress
design: docs/designs/portable-owner-only-secret-store.md
epic: all-integrations-connectors
areas: [connector-secrets, persistence, security, windows, release]
note: "Milestone 1 blocker — Exchange X-127 cannot support Windows or ship a complete local composition while connector-secrets 0.19 exposes its durable store only on Unix"
---

# Persist connector secrets owner-only on every Flux platform

## Goal

Make `connector-secrets` own one portable durable credential backend for every platform in the Flux
release set. Exchange can bind the same public `FileStore`/`SecretStore` composition on Linux,
macOS and Windows without duplicating the file format, inspecting crate-private batch mutations or
falling back to memory.

## Acceptance

- [ ] `FileStore` and its public `SecretStore` implementation compile for
      `aarch64-apple-darwin`, `x86_64-apple-darwin`, `aarch64-unknown-linux-gnu`,
      `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-msvc`. The Windows build contains the real
      durable backend; a cfg-disabled type, in-memory substitute or compile-only façade is refused.
- [ ] The logical v1 file format, credential addressing, bounded reads/writes, atomic whole-file
      replacement and `SecretBatch` all-or-nothing semantics remain one implementation contract.
      An external host uses the existing public port and never needs access to a credential value or
      to crate-private mutation representation in order to obtain the platform-native backend.
- [ ] Unix creation remains `0700` for the state directory and `0600` for the file. Existing wrong
      owner, wider mode, symlink/wrong object kind or uninspectable metadata refuses before a value
      is read and is never repaired silently.
- [ ] Windows creation uses a protected DACL owned by the process identity's SID, with access
      granted only to that SID. An inherited allow entry, allow entry for another SID, foreign owner,
      reparse point, wrong object kind or unreadable security descriptor refuses before a value is
      read or written and is never repaired silently.
- [ ] Failing-first native Unix and Windows fixtures widen each relevant permission/ownership
      property, prove the affected path is named without any value or credential address, and prove
      refusal leaves the planted metadata byte-for-byte/security-descriptor-equivalent unchanged.
      Native Windows CI runs the backend and restart/batch tests; cross-compilation alone is not
      acceptance evidence.
- [ ] A credential path directly beneath a shared directory such as `/tmp` remains refused, but the
      message never recommends `chmod 700 /tmp` or narrowing another shared ancestor. It directs the
      operator to create an owner-only child or use a conventional per-user state root. A
      failing-first diagnostic test pins this exact safety property.
- [ ] A native restart proof writes more than one credential, applies a first-to-second connection
      migration as one batch, reopens the store in a new instance and resolves only the committed
      post-migration addresses. An injected write failure exposes neither a partial batch nor a
      truncated prior file on both Windows and Unix.
- [ ] `AGENTS.md`, the crate README/rustdoc and both changelogs describe the platform-native
      protection honestly: Unix modes and Windows owner/DACL checks are distinct, neither is
      encryption, administrator/root and copied backups remain outside the guarantee, and the
      backend is still for one local operator.
- [ ] The complete Rust workspace gate, publish-closure dry runs and native platform CI pass. The
      merged change is release-ready for the coordinator's patch release; dependency completion for
      Exchange X-127 is the verified crates.io publication, not merely this repository merge.

## Progress

- 2026-08-04: Filed after Exchange X-127 verified that released `connector-secrets` 0.19 exports
  `FileStore` only under `cfg(unix)`. Its public `SecretStore::apply(&SecretBatch)` preserves the
  atomic contract, so the correction is to make the connector-owned backend portable rather than
  expose batch internals or duplicate persistence in Exchange.
- 2026-08-04: The same audit reproduced `/tmp/flux-secret` refusing the shared `/tmp` parent and
  advising `chmod 700 /tmp`. Refusal is correct; changing permissions on the shared ancestor is not.

## Notes

- Cross-repository authority:
  `../flux-roadmap/decisions/0004-flux-manages-a-verified-local-exchange.md` and
  `../flux-roadmap/programs/integrations.tsv` at roadmap commit `79eb73e`.
- Exchange X-127 preserves its Unix/all-or-nothing development-state work while this publishes.
  It resumes only from the released connector line; no path or git dependency substitutes for the
  crates.io boundary.
- The story does not authorize automatic repair of existing permissions or a second credential
  schema. Refuse unsafe state, preserve evidence, and keep the v1 logical bytes portable.
