# Handoff: flux stories for verified inbound webhooks — **FILED**

> **This file is a handoff artifact, not a tracked backlog.** Nothing in it is a story on *this*
> repo's board, and `/track:board` must never pick these up.
>
> Source design: [verified-webhook-seam.md](verified-webhook-seam.md) (the flux-side design) ·
> [inbound-events.md](inbound-events.md) (the parent) · Parent story:
> [C-64](../stories/C-64-design-verified-webhook-seam.md)

## Status: filed on flux's board, 2026-07-30

The five draft blocks this document used to carry as ready-to-paste text **have been written directly
into flux** and are no longer drafts. The filed versions are the authority; they carry more evidence
than the drafts did, because writing them required reading the request path rather than summarizing it.

| was | filed as | flux path |
|---|---|---|
| F-1 raw body + `verify` block | **C-291** | `../flux/docs/stories/C-291-webhook-verify-raw-body.md` |
| F-2 scheme matrix, constant-time, tolerance | **C-292** | `../flux/docs/stories/C-292-webhook-signature-schemes.md` |
| F-3 challenge/handshake | **C-293** | `../flux/docs/stories/C-293-webhook-challenge-handshake.md` |
| F-4 discriminator → trigger routing | **C-294** | `../flux/docs/stories/C-294-webhook-discriminator-routing.md` |
| F-5 delivery id | **C-295** (widened) | `../flux/docs/stories/C-295-delivery-envelope-verified-flag.md` |

All five carry `epic: verified-webhook-channel` and `pillar: Core`, `status: backlog`, and no
`design:` field — flux has no design doc for this seam, and a `design:` pointing at a file flux does
not have is worse than none. Each story's `## Notes` points at
[verified-webhook-seam.md](verified-webhook-seam.md) in this repository instead.

**C-295 is wider than F-5 was.** F-5 asked only for the delivery id. The filed story also carries the
`verified` flag, because C-82's invariant — a deliberately-unverifiable surface must be
distinguishable from a verified one — is *false in effect* if flux normalises the distinction away at
delivery. Without it, `verification = "none"` is loud in the manifest and invisible to the flow.

**These ids were free at the moment of writing** (flux's highest `C-` was `C-290`, checked
immediately before each file was created). They are now taken. flux's board was **not** regenerated —
the files are uncommitted in flux's working tree, and whoever commits them runs flux's own
`/track:board`.

## The lessons that made this handoff work — keep them

- **IDs are consumed concurrently.** The previous handoff (`auth-seam`) claimed `C-266 … C-276`; by
  the time anyone looked, flux's fleet had consumed that entire range with unrelated work. The rule
  that replaced "claim a range in advance" is: **check immediately before each write**, with

  ```bash
  ls ../flux/docs/stories | grep -oP '^C-\d+' | sort -t- -k2 -n | tail -1
  ```

  and re-check after, to catch a concurrent filer.
- **Naming: this is `webhook signature verification`, never "the inbound auth seam".** flux already
  has a **done** `request-auth-seam` (`docs/designs/request-auth-seam.md`, D-64/D-68) covering inbound
  *bearer → principal* resolution. A story titled "inbound auth" reads as a duplicate of shipped work
  and gets closed. Every filed story repeats this in its Notes.
- **A design doc must exist in flux if a block sets `design:`.** None of the five does; they cite the
  path in this repository from `## Notes`, which survives whether or not flux ever ports the design.
- **Every flux-side claim is anchored to a symbol and a verified tree.** All citations in the filed
  stories and in [verified-webhook-seam.md](verified-webhook-seam.md) were read at flux
  `v0.40.0-4-g2abd0a13` (workspace version `0.40.0`). Line numbers move; symbol names do not.

## Sequencing

```
C-291  raw-body capture + the `verify` declaration      ← the foundation
C-292  scheme matrix, constant-time compare, tolerance  ← depends on C-291
C-293  challenge/handshake, answered without a turn     ← depends on C-291, independent of C-292
C-294  discriminator → trigger-label routing            ← depends on C-291
C-295  delivery envelope: id + verified flag            ← depends on C-291
```

C-291 is the only hard prerequisite; C-292 through C-295 can run in parallel afterwards.

## The load-bearing facts, re-verified at flux `v0.40.0-4-g2abd0a13`

Re-grep by symbol, not by line number.

- `crates/flux-channels/src/config.rs:18-32` — `WebhookSettings { addr, path, async, token }`, an
  optional **static bearer token**, and the struct derives `Debug` while holding the resolved
  plaintext `token` at `:31`.
- `crates/flux-channels/src/adapters/webhook.rs` — **no HMAC path anywhere in the file.** The bearer
  check at `:88-97` is the only authentication. `constant_time_eq` at `:123-132` is reusable.
- `crates/flux-channels/src/adapters/webhook.rs:86` — **`Json(body): Json<Value>` is an axum
  extractor**, so the body is already parsed by the time the handler runs. This is the finding that
  makes C-291 a structural change rather than an added `if`.
- `crates/flux-channels/src/adapters/mod.rs:48` — `build_channels` dispatches `"webhook" | "http"`;
  an unknown kind is a hard error at `:63`; an unresolved `{"$secret":…}` marker is refused at
  `:39-45` (`first_unresolved_secret`, `:23-32`; test at `:75`).
- `crates/flux-app/src/secrets.rs:43` — `resolve_secrets` registers every resolved secret with the
  redactor **before** channels are built, recursing into nested records (`:47-58`). The secret-supply
  path for `verify` needs no new machinery.
- `crates/flux-app/src/bus.rs:115-118` — `Event { label, payload }`, which is why C-295 exists.
- `crates/flux-app/src/app.rs:1988-1996` — `seed_payload` binds every top-level payload field as a
  flow symbol, which is why "put the delivery id in the payload" collides with vendor fields.
- `Cargo.toml:150-153` — `base64`, `sha2`, `hmac`, `hex` are already workspace dependencies;
  `crates/flux-providers/src/bedrock.rs:32,42` already computes HMAC-SHA256. No new third-party
  dependency is needed.
- **Correction to the previous handoff.** It claimed `AppDeliverer` serializes deliveries behind a
  mutex, so a verified channel would inherit a one-delivery-at-a-time property. **That is false at
  `v0.40.0`.** `AppDeliverer` is `{ app: Arc<App> }` and does nothing but forward
  (`crates/flux-channels/src/deliver.rs:22-39`); admission is a **semaphore**, not a mutex —
  `DEFAULT_MAX_INFLIGHT_DELIVERIES = 64` with a `FLUX_MAX_INFLIGHT_DELIVERIES` override
  (`crates/flux-app/src/admission.rs:49`, `:51-68`). Up to 64 verified deliveries run concurrently, so
  nothing about the seam may assume delivery serialization.
