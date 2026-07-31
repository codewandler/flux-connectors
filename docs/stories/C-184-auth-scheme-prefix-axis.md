---
id: C-184
title: "AuthScheme cannot spell a credential with a prefix, and three connectors are blocked on it"
pillar: Spec
status: ready
priority: 1
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

- [ ] A credential can be placed inside a header value with text before it, and — for PagerDuty's
      shape — text after it. Decide whether that is one axis (`prefix` + `suffix`) or a single
      template with one substitution point, and **record the decision with its reason**. A template
      is more expressive and also lets an author write something the emitter cannot check; say which
      you chose and what it refuses.
- [ ] **The credential value itself is never authored.** This is the constraint that makes the story
      subtle: `SSWS ` is connector data, `<token>` is a runtime secret, and the whole point is that
      the TOML carries the first and never the second. A test must assert that no generated artifact
      contains anything but a credential *reference*.
- [ ] Whatever lands is registered with the redactor as the value that actually travels, not as the
      raw credential — [C-159](C-159-request-debug-and-query-encoding.md) §2 found precisely this
      class of bug for query placement, where `query_encode` meant the registered string and the
      travelling string differed. A prefixed header value has the same hazard: registering `<token>`
      while `SSWS <token>` travels is fine, but registering the prefixed form and scrubbing the bare
      one is not. Say which is registered and why.
- [ ] **Failing-first test:** a provider declaring the new spelling does not load today.
- [ ] Every existing provider's emitted module is **byte-identical** across this change — 23
      providers, so `bearer` and `basic` must be untouched by whatever is added.
- [ ] The build stays a fixed point and the scoped gate is green.

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
