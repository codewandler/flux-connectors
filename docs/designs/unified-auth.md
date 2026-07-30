# Design: unified auth — one model for every provider's credentials

**Status:** proposed · **Pillar:** Bridge · **Stories:** C-19 … C-23

## Why

Every connector we will ever ship differs from its neighbours mostly in **how it authenticates**.
Endpoints are boring — a method, a path, some parameters. Credentials are where providers are
genuinely, irreducibly different, and where a naive model runs out of room fastest.

The three providers already in scope prove it before we have even shipped one:

| Provider | Credential shape |
|---|---|
| zendesk | `Authorization: Basic base64("<email>/token" + ":" + <api_token>)` |
| freshdesk | `Authorization: Basic base64(<api_key> + ":" + "X")` |
| babelforce | `Authorization: Bearer <SSO-issued token>`, with **JWT** planned |

And the long tail is worse: raw-value headers (`x-api-key`, `PRIVATE-TOKEN`), query-parameter keys,
OAuth2 in four grant flavours, locally-signed JWTs, HMAC request signing (AWS SigV4), and login
endpoints that hand back a session token.

**The trap to avoid** is the one flux's current vocabulary is already in: enumerating credential
*shapes* as flat variants. `AuthScheme::{Bearer, Basic, Header{name}, Query{name}}`
(`../flux/crates/flux-plugin-protocol/src/lib.rs:344`) covers four cases well and then needs a new
variant for every fifth. `Bearer` means two things at once — *prefix the value with `Bearer `* **and**
*put it in the `Authorization` header* — so the moment a provider wants `Token <t>` in
`Authorization`, or a Bearer token in a cookie, the enum has no way to say it without growing.

action-proxy hit exactly this wall and answered it with a bespoke template function:
`Authorization: 'Basic {{base64:encode (append (append context.user "/token:") context.api_token)}}'`.
That is credential assembly smuggled into a config template — untyped, unauditable, and with the raw
token passing through a string interpolator. We are not doing that.

## Approach

**Separate three concerns that every credential scheme actually has, and that flat enums conflate.**

```
purpose ──▶  Source  ──▶  Acquisition  ──▶  Placement  ──▶  the request
             where raw     how it becomes    where it goes
             material      a usable          on the wire
             comes from    credential
```

```rust
struct AuthMethod {
    purpose: String,          // the name an operation references
    source:  Source,          // env vars, token store, key file
    acquire: Acquisition,     // static | basic_join | oauth2 | jwt | session | hmac
    place:   Placement,       // header{name, prefix} | query{name} | cookie{name} | signature
}
```

### The three axes

**1 · Source — where raw material comes from.** `env` (names tried in order, as flux's `AuthMethod`
already does), `token_store` (flux's existing OAuth token store), `file` (a PEM key for JWT signing).
Never a literal value, in any artifact, ever.

**2 · Acquisition — how raw material becomes a usable credential.**

| Acquisition | Produces | Effectful? |
|---|---|---|
| `static` | the secret unchanged | no |
| `basic_join { user_source }` | `base64(user + ":" + secret)` | no |
| `jwt { key_source, alg, claims, ttl }` | a locally-signed JWT | no |
| `oauth2 { grant, token_url, scopes }` | an access token | **yes** — network, cache, refresh |
| `session { login_op, extract }` | a token from a login endpoint | **yes** |
| `hmac { alg, canonical }` | a per-request signature | no (but request-dependent) |

**3 · Placement — where the credential goes.** `header { name, prefix }`, `query { name }`,
`cookie { name }`, or `signature` (where the acquisition computes placement itself, as SigV4 does).

**`prefix` on header placement is the single highest-value element of this whole design.** It is what
turns "Bearer" from an enum variant into data. With it, `Bearer `, `Basic `, `Token `, `GenieKey ` and
the empty prefix are all one code path.

### It is a strict superset of flux's vocabulary, not a rival to it

This matters more than elegance: the `$auth` seam has to land in **flux**, and flux's maintainers
will rightly reject a parallel auth vocabulary. Every existing `AuthScheme` variant is a *preset* of
the model, so the two agree by construction:

| flux `AuthScheme` | Unified model |
|---|---|
| `Bearer` | `static` + `header{ "Authorization", "Bearer " }` |
| `Basic` | `basic_join{user_env}` + `header{ "Authorization", "Basic " }` |
| `Header { name }` | `static` + `header{ name, "" }` |
| `Query { name }` | `static` + `query{ name }` |

So a connector that only uses the four presets serializes to exactly what flux understands today, and
the richer forms are additive. **We ship the presets first and grow along the axes**, rather than
proposing a rewrite of flux's auth model to get one provider working.

### The line that decides who executes what

**Effectful acquisition runs in the host, never in generated Flux.**

Pure acquisitions (`static`, `basic_join`, `jwt`) are a local computation at request time. Effectful
ones (`oauth2`, `session`) need a network round trip, a token cache, expiry tracking, and
refresh-on-401. If those ran in generated Flux, every connector would re-implement token refresh, and
— fatally — the raw token would have to pass through a bound Flux symbol, defeating redaction and
putting credentials in model-visible state.

So the generated `.flux` **only ever names a purpose**. Everything behind that name is the host's.
This is also why the connector manifest, not the Flux module, carries the auth declaration.

### Requirement sets sit above this

An operation does not reference a *method*; it references **requirement sets** over purposes — all
purposes in a set (AND), any one set among alternatives (OR), an explicit empty set for
unauthenticated. Each purpose then resolves through source → acquisition → placement independently,
which is what lets one request carry two credentials in two different places.

**Alternative selection must be deterministic and recorded:** choose the first requirement set whose
purposes are all *configured* (their sources resolve), and record the choice in the manifest so a
regeneration is stable and a reader can see why that scheme was picked.

### What this buys, concretely

Adding a provider archetype becomes adding **one value on one axis**, not a new variant crossing all
of them:

- A provider wanting `Authorization: Token <t>` → a prefix string. No code.
- A provider putting an API key in a cookie → one `Placement` variant.
- Babelforce's planned JWT → one `Acquisition` variant; placement is the Bearer preset it already
  uses, so nothing else moves.

## Alternatives considered

- **Keep flat scheme variants and add one per provider shape.** Simplest today, and it is what flux
  has. Rejected: it is combinatorial — every (assembly × placement) pair needs its own variant — and
  it is what forced action-proxy into template functions.
- **Let generated Flux assemble credentials** using a `base64` builtin and string interpolation.
  Rejected on the same grounds as in [auth-seam.md](auth-seam.md): the raw token would land in a
  bound symbol, defeating redaction, and credential assembly would sit in model-visible code.
- **Model auth as an opaque per-provider Rust plugin.** Maximum flexibility, and exactly the
  hand-written-adapter cost this repo exists to eliminate.
- **Adopt OpenAPI's `securityScheme` shape verbatim as the IR.** Tempting for ingest fidelity, but it
  has the same conflation (`http`+`scheme: bearer`) and no vocabulary for JWT signing or HMAC. We map
  *from* it (C-5) rather than adopting it.

## Risks & open questions

- **Scope discipline.** This model can express far more than the three providers need. Ship the
  presets and `oauth2`; declare `jwt`, `session` and `hmac` in the schema but implement them only
  when a provider demands one. The schema must accept them without reshaping — that is the actual
  requirement, not working code for all six.
- **flux must accept the seam.** The whole design assumes flux grows a purpose-resolving `$auth`
  marker. If flux's maintainers prefer a different shape, this model still stands, but the *marker*
  changes — keep the two decoupled.
- **Token cache semantics are unspecified here** — lifetime, scope (per-user? per-session?),
  refresh-on-401, concurrent refresh. That is real design work for the OAuth2 story, and it is where
  effectful acquisition gets genuinely hard.
- **HMAC is the one archetype that may not fit.** Request signing needs the method, path, headers,
  body and a timestamp — it is not a credential slot but a transformation of the whole request. The
  `signature` placement is a placeholder acknowledging this, not a solved problem. If a SigV4
  provider becomes real, expect this design to need revision.
- **Deprecated schemes must be expressible as excluded.** Babelforce's `X-Auth-Access-*` pair is
  still in its spec but must never be emitted. "Known and deliberately excluded" is a different state
  from "absent", and the IR needs to say so.

## Acceptance / done

- The IR expresses all three in-scope providers plus raw-header, query-key, and OAuth2 archetypes,
  with no provider-specific code.
- The four flux `AuthScheme` presets round-trip exactly, proving the model is a superset.
- A conformance test asserts one case per archetype, so a new provider shape fails loudly at the
  model rather than silently at request time.
- No credential value appears in any provider TOML, generated `.flux`, manifest, or lockfile.
- Effectful acquisition is declared in the manifest and executed by the host — never emitted into
  generated Flux.
