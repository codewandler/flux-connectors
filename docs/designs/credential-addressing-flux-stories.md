# Handoff: credential-store stories for flux's board

**This is a handoff artifact, not this board's backlog.** Paste-ready stories for
`codewandler/flux`, following the precedent of
[auth-seam-flux-stories.md](auth-seam-flux-stories.md), C-64 and C-84.

Filed from [credential-addressing.md](credential-addressing.md) (C-90). Every fact below was verified
against flux at 2026-07-30 and cites the file it came from; re-check before filing, since flux moves.

---

## Why these three

flux has one credential-persistence trait, `flux_credentials::CredentialStore`, with a complete
`VaultCredentialStore` that **has zero callers** — it exists for a host app to construct and inject
through `SystemHostCaps::with_credential_store` (`crates/flux-plugin/src/host.rs:304`).

That is a sound design. Three gaps in it block a multi-tenant deployment, and all three are flux's to
close — a sibling repository can adapt around them but cannot fix them.

---

## F-a · Put the account dimension back in the credential key

**Pillar:** Core · **Epic:** `plugin-oauth`

### Goal
Let one flux serve two tenants with different credentials for the same integration.

### Why now
**This was already accepted and then dropped.** D-83's own acceptance criterion reads:

> generalize `save_stored`/`store_path`/`TokenSource` … to key by **`plugin+purpose[+account]`**
> instead of the two provider consts

and its Progress note records what shipped: *"Keying is `plugin:<name>:<purpose>`."* The `[+account]`
never landed. `docs/designs/plugin-oauth.md` still advertises the intent, naming the consumer:
*"Also unblocks the UI-configured Integrations pillar (per-customer OAuth tokens → Vault, never a file
on a pod)."*

### Acceptance
- [ ] The key carries an optional account, and the single-tenant case encodes **exactly as it does
      today** — so no existing `~/.flux/credentials.toml` entry moves and no deployment re-provisions.
- [ ] All five construction sites go through one function rather than five `format!`s:
      `crates/flux-plugin/src/host.rs:399` and `:444`, `crates/flux-cli/src/auth_cmd.rs:171` and
      `:385`, `crates/flux-cli/src/plugin_cmd.rs:1011`.
- [ ] **The key gets a parser and validation.** It is `&str` end to end today with no escaping, so a
      plugin named `a:b` and a plugin `a` with purpose `b:x` collide — and on the Vault backend
      `data_url` does `key.replace(':', "/")`, so a `/`-bearing component traverses the KV path
      (`crates/flux-credentials/src/lib.rs:872`). Not attacker-controlled today, since plugin names
      come from installed manifests; unenforced all the same.
- [ ] The account reaches the key from the **authenticated principal**, never from request input.
      `AuthContext.account` already exists (`crates/flux-auth/src/request.rs:16`) and is documented as
      *"a tenancy/storage key only"*; today it reaches the event store and stops.
- [ ] Recorded: `SystemHostCaps` is **session-lived, built once at startup**
      (`host.rs:199`), so per-request tenant switching needs a lifecycle decision, not just a key
      change. That decision is the story's real content.

### Notes
- flux already has **two incompatible key shapes**: `plugin:slack:bot_token` (`CredentialStore`) and
  `flux_secret::Ref`'s `plugin/slack/main/bot_token` — which has a parser, a `Display` round-trip, and
  an `instance` slot that is the closest existing thing to this dimension. Worth deciding whether this
  story converges them or deliberately keeps them apart.

---

## F-b · `delete` on `CredentialStore`, so a credential can actually be cleared

**Pillar:** Core · **Epic:** `plugin-oauth`

### Goal
Make `flux auth set --clear` work against every backend, not only the file one.

### Why now
`delete_token` exists **only as a free function hard-wired to the file backend**
(`crates/flux-credentials/src/lib.rs:606`), and `crates/flux-cli/src/auth_cmd.rs:173` calls it
directly. So a Vault-backed deployment cannot clear a credential through flux at all — and more
subtly, the CLI's *write* path and the runtime's *read* path use different mechanisms, so an injected
store is **read-only in practice**. The two halves do not meet.

### Acceptance
- [ ] `async fn delete(&self, key: &str) -> Result<()>` on the trait; missing entries stay a no-op, as
      the file backend already behaves.
- [ ] `save_token` / `load_token` / `delete_token` route through the injected store when there is one,
      so the CLI and the runtime agree about where a credential lives.
- [ ] A test asserts an injected store observes a `flux auth set` and a `--clear`.
- [ ] Provider tokens (`claude`/`codex`) are considered explicitly: `claude_token_source`
      (`lib.rs:1185`) and `RefreshingToken::refresh_locked` (`lib.rs:1135`) call the file functions
      unconditionally, so the trait covers only the plugin auth-purpose path today. Either bring them
      in or record why not.

---

## F-c · Distinguish "not configured" from "the backend is down"

**Pillar:** Core · **Epic:** `plugin-oauth`

### Goal
Stop a Vault outage from looking exactly like an unconfigured integration.

### Why now
`CredentialStore::load` returns `Option`, and `VaultCredentialStore::load` swallows transport errors
with `.ok()?` (`crates/flux-credentials/src/lib.rs:1063`). So a network failure, an expired Vault
token and "this customer never connected Slack" are the same value. `resolve_purpose` then falls
through to the env keys and finally errors with a message naming *configuration* paths
(`crates/flux-plugin/src/host.rs:466`) — which is actively misleading when the real cause is that
Vault was unreachable.

### Acceptance
- [ ] `load` returns a result that separates absence from failure. The single-tenant file backend's
      behaviour is unchanged.
- [ ] A backend failure does **not** fall through to the env fallback: falling back on an outage means
      quietly using a different credential than the operator configured.
- [ ] The error a user sees names the cause. "Vault is unreachable" and "you have not connected this
      integration" lead to different actions.
- [ ] `flux plugin status` reflects the distinction. It is the closest thing flux has to a
      configuration surface (`crates/flux-cli/src/plugin_cmd.rs:1010`), and it currently renders
      hand-formatted prose with no `--json` and **no `ConfigSpec` entries at all** — so jira's
      `cloud_id` is invisible. Worth folding in if the story is open anyway.
