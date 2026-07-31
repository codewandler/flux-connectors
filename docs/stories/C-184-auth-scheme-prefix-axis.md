---
id: C-184
title: "AuthScheme cannot spell a credential with a prefix, and three connectors are blocked on it"
pillar: Spec
status: done
design: docs/designs/unified-auth.md
epic: provider-fleet-2
areas: [connector-spec, connector-flux]
note: "measured by C-161: AuthScheme is a closed 5-variant enum with no prefix field. unified-auth.md called a prefix 'the single highest-value element of this whole design' and it was never built. Blocks Okta, PagerDuty, Statuspage"
---

# AuthScheme cannot spell a credential with a prefix, and three connectors are blocked on it

## Goal

Let a connector send a credential inside a header value it does not wholly occupy, so that a vendor
using anything other than `Bearer` can be addressed.

## What was measured

[C-161](C-161-provider-okta.md) probed this rather than assuming it. `AuthScheme`
(`crates/connector-spec/src/auth.rs:70-102`) is closed with `deny_unknown_fields`:

```rust
pub enum AuthScheme { Bearer, Basic, Header { name: String }, Query { name: String }, Signing }
```

There is **no `prefix` field anywhere**. `docs/designs/unified-auth.md:75-77` proposed exactly one and
called it *"the single highest-value element of this whole design"* — it was never implemented. Two
probe fixtures in `crates/connector-flux/tests/okta_connector.rs` pin the refusals: `scheme = "ssws"`
fails as `unknown variant`, and `header = { name = "Authorization", prefix = "SSWS " }` fails as
`unexpected keys in table`.

**Three connectors are blocked, each on a different shape of the same gap:**

| story | vendor wants | why `Header { name }` is not enough |
|---|---|---|
| [C-161](C-161-provider-okta.md) | `Authorization: SSWS <token>` | a scheme word before the credential |
| [C-162](C-162-provider-pagerduty.md) | `Authorization: Token token=<key>` | the credential is a **field inside** the value, not its tail |
| [C-181](C-181-provider-statuspage.md) | `Authorization: OAuth <key>` | a scheme word again, and `OAuth` is not OAuth2's bearer usage |

**Two are not blocked, and that distinction is the finding's other half.** C-175 (LaunchDarkly) and
C-178 (ClickUp) send the token raw, which is already `Header { name: "Authorization" }`.

## Acceptance

- [x] A credential can be placed inside a header value with text before it, and — for PagerDuty's
      shape — text after it. Decide whether that is one axis (`prefix` + `suffix`) or a single
      template with one substitution point, and **record the decision with its reason**. A template
      is more expressive and also lets an author write something the emitter cannot check; say which
      you chose and what it refuses.
- [x] **The credential value itself is never authored.** This is the constraint that makes the story
      subtle: `SSWS ` is connector data, `<token>` is a runtime secret, and the whole point is that
      the TOML carries the first and never the second. A test must assert that no generated artifact
      contains anything but a credential *reference*.
- [x] Whatever lands is registered with the redactor as the value that actually travels, not as the
      raw credential — [C-159](C-159-request-debug-and-query-encoding.md) §2 found precisely this
      class of bug for query placement, where `query_encode` meant the registered string and the
      travelling string differed. A prefixed header value has the same hazard: registering `<token>`
      while `SSWS <token>` travels is fine, but registering the prefixed form and scrubbing the bare
      one is not. Say which is registered and why.
- [x] **Failing-first test:** a provider declaring the new spelling does not load today.
- [x] Every existing provider's emitted module is **byte-identical** across this change — 23
      providers, so `bearer` and `basic` must be untouched by whatever is added.
- [x] The build stays a fixed point and the scoped gate is green.

## Outcome

**The axis is `prefix` alone — no `suffix`, no template.** `AuthScheme::Header` now carries
`{ name, prefix }`, where `prefix` defaults to empty and does not serialize when it is.

The decision turned on evidence this story did not have to gather: C-161 had already measured
PagerDuty's `Token token=<key>` as *"the whole value is a fixed literal followed directly by the raw
key, which is a prefix exactly like `SSWS `, just longer"*. So the story's own framing — that
PagerDuty needs *"text after it"* — was the one thing that did not survive contact with the
measurement. All three blocked vendors put the credential at the **tail**:

| vendor | header | prefix |
|---|---|---|
| Okta (C-161) | `Authorization: SSWS <token>` | `SSWS ` |
| Statuspage (C-181) | `Authorization: OAuth <key>` | `OAuth ` |
| PagerDuty (C-162) | `Authorization: Token token=<key>` | `Token token=` |

A template was rejected for being more expressive in exactly the wrong direction: it can spell a
credential substituted **zero** times — an unauthenticated request that every artifact describes as
authenticated — or twice, or with text after it that no vendor pins. A prefix makes each unspellable
rather than merely refused, because the host appends the credential and there is no substitution
point to aim at. `suffix` was not built: nothing needs it, and an unused axis would sit in 41
providers' catalogue rows with no vendor to say what belongs in it.

**What it refuses.** `provider::validate` rejects a prefix that spells a resolution marker (`${…}`,
`{{…}}`, `$secret`), names a declared credential or the env var that resolves it, or holds anything
but visible ASCII, space and tab — the last being header injection, since a prefix reaches a header
value verbatim. It deliberately does *not* consult `CREDENTIAL_VALUE_PREFIXES`, which exists to catch
a pasted credential in a constant header; a scheme word is that same text in the one position where
it is correct, and PagerDuty's prefix is literally `Token token=`.

**Redaction: the bare credential is registered; the prefix is not.** The rule the two axes divide on
is whether the value is recoverable from one the redactor already holds. Acquisition can *transform*
the secret — `base64(user:secret)` does not contain it, hence its second registration — while
placement only *surrounds* it: `SSWS <token>` contains `<token>` verbatim, so the existing
registration already scrubs it to `SSWS <redacted>`. Registering the prefixed form would repeat
C-159 §2's divergence in the other direction: it would put the public word `SSWS ` into the redactor
and leave the **bare** token — the form a 401 body echoes back — unheld.

**Byte-identity, proven mechanically.** A full build after the change wrote **one** artifact:
`web/public/catalog.json`. Every `.flux` module, every manifest, the embedded Rust catalogue and
`connectors.lock` are untouched, because an empty prefix does not serialize and the catalogue's
`Header` arm already emitted `prefix: ""` when it was hard-coded. The catalog.json diff is purely
additive — one `prefix` key per credential (31 `"Bearer "`, 13 `""`, 3 `"Basic "`), no value changed.
That key is published deliberately: without it Okta's `Authorization: SSWS <token>` and
LaunchDarkly's raw `Authorization: <token>` flatten to the same two keys, so a consumer would build
one while believing the catalogue had described it. `web/data/catalog.mts` was updated to match.

**The runtime needed no change.** `catalog::Placement::Header { name, prefix }` and
`connector_pack::auth::place`'s `format!("{prefix}{value}")` have composed `Bearer ` as data since
the pack landed. The gap was only ever in `AuthScheme` — the half an author writes — which is why
unified-auth.md's claim that prefix is "the single highest-value element" was half-true for four
waves: built at the bottom, missing at the top.

**Unblocked:** C-161 (Okta) moved from `blocked` to `ready`; C-162 and C-181 can now be authored.
None of the three connectors ships here — this story built the seam only.

**Recorded, not built:** `Query { name }` has the same expressive gap. The committed catalogue has
zero query placements, so it stays a note in `docs/designs/unified-auth.md` rather than a field.

## Notes

- **This runs solo.** It changes `connector-spec`'s auth model and `connector-flux`'s auth emission,
  which every provider reads, so no provider story may share its wave.
- Read `docs/designs/unified-auth.md` first and update it — it is the design that proposed this and
  then did not get it, so the record should say what finally landed and how it differs.
- C-161's test asserts today's refusals by name. **It will need deliberate revisiting when this
  lands**, and it says so in its own doc comment — that is intended, not breakage.
- Do not solve this by widening `Header { name }` to accept a value template that the author fills
  with the credential themselves. That would put a credential value one typo away from the TOML,
  which is the rule this repository does not bend.
- Worth checking while here: whether `Query { name }` has the same expressive gap. Nothing needs it
  yet — the committed catalogue has zero query placements — so do not build for it, just record it.
