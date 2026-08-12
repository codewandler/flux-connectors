//! Auth in the connector IR: the credentials a connector **declares**, and the ones an operation
//! **requires**.
//!
//! Two separate things live here, and conflating them is the mistake this module exists to prevent:
//!
//! - an [`AuthMethod`] says *how* one named credential is injected into a request — it is a
//!   declaration, and it belongs to the connector;
//! - an [`AuthRequirement`] says *which* of those credentials a given operation needs — it is a
//!   reference, by credential name, and it belongs to the operation.
//!
//! The requirement model is OpenAPI's, deliberately: an operation carries a **list** of
//! requirements, each requirement is a **set** of credentials that must all be satisfied on the
//! same request (AND), and the list itself is a set of alternatives any one of which suffices (OR).
//! Babelforce is why: its manager API declares `oauth2`, `accessId` (`X-Auth-Access-Id`) and
//! `accessToken` (`X-Auth-Access-Token`), where the two api-key headers travel **together** and are
//! an alternative to OAuth2 — `security: [{oauth2: [*]}, {accessId: [], accessToken: []}]`. A
//! requirement collapsed to a single credential could not express that connector at all.
//!
//! # Why an AND-group is a *mechanism* and its members are *credentials*
//!
//! The naming is load-bearing, and the babelforce pair settles it. Two headers on one request are
//! **two credentials** making up **one mechanism**:
//!
//! ```text
//! credentials = ["access_id", "access_token"]   // right: two credentials, one mechanism
//! mechanisms  = ["access_id", "access_token"]   // wrong: that is one mechanism, not two
//! ```
//!
//! So an [`AuthRequirement`] *is* a mechanism, and the names it holds are credential names.
//! `zendesk.api_token` says what the thing **is**, which is what a name should do.
//!
//! # Relationship to flux's `purpose`
//!
//! flux's plugin protocol calls the same identifier a `purpose`
//! (`flux_plugin_protocol::AuthMethod::purpose`,
//! `../flux/crates/flux-plugin-protocol/src/lib.rs:424`), and the `$auth` marker in generated Flux
//! spells the key `purpose` because that is flux's wire format, which this repo does not get to
//! rename. **Our [`AuthMethod::name`] resolves to flux's `purpose` field**: one value, two
//! spellings, translated at the manifest boundary in C-10. Inside this crate the word is
//! *credential*, because that describes what the value identifies rather than why someone wanted
//! it.

use indexmap::IndexSet;
use serde::{Deserialize, Deserializer, Serialize};

/// How the host injects a resolved secret into an HTTP request.
///
/// This is a deliberate mirror of `flux_plugin_protocol::AuthScheme`
/// (`../flux/crates/flux-plugin-protocol/src/lib.rs:344`): same four variants, same
/// `rename_all = "snake_case"` externally-tagged encoding, same `Bearer` default. flux already
/// models exactly these shapes for plugins, and a connector manifest resolves a credential through
/// the *same* injection logic the plugin host uses — so inventing a second vocabulary here would
/// mean translating between two spellings of one idea forever (see `docs/designs/auth-seam.md`).
///
/// It is a mirror rather than a re-export because flux-connectors depends on `flux-lang` from
/// crates.io and on nothing else of flux's; `flux-plugin-protocol` is not a dependency of this
/// workspace. The wire form is therefore pinned by test (`tests/ir_roundtrip.rs`,
/// `auth_scheme_matches_the_flux_plugin_protocol_vocabulary`) so drift on either side is caught.
///
/// **Forward requirement — JWT.** Babelforce ships SSO-issued Bearer today and plans JWT, so its
/// scheme list keeps growing. This enum expresses JWT without reshaping: a minted JWT reaches the
/// wire as `Authorization: Bearer <jwt>`, which is [`Bearer`](Self::Bearer), or as a custom header,
/// which is [`Header`](Self::Header). What a JWT adds is not a new *injection* shape but a new
/// *acquisition* step — signing claims with a key, refreshing on expiry — and acquisition is
/// already modelled on the sibling axis, as [`AuthMethod::oauth2`]. So JWT lands as an additive
/// `Option<JwtSpec>` field next to `oauth2`, and `AuthScheme` stays exactly as it is. The one case
/// that *would* force a reshape is a scheme whose wire form is neither a header nor a query
/// parameter — request-body or signature-based auth (AWS SigV4, HMAC over the payload) — and none
/// of the target providers use one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum AuthScheme {
    /// `Authorization: Bearer <secret>`.
    #[default]
    Bearer,
    /// `Authorization: Basic base64(<user>:<secret>)` — `<user>` from the method's `user_env`.
    Basic,
    /// A custom header `<name>: <prefix><secret>` (e.g. `X-Auth-Access-Id`, `PRIVATE-TOKEN`, or
    /// `Authorization: SSWS <token>`).
    Header {
        /// The header name.
        name: String,
        /// Literal text placed **in front of** the credential, trailing space included. Empty — the
        /// default, and every value in the shipped catalogue before C-184 — is a header whose whole
        /// value is the secret.
        ///
        /// # This is connector data, never a credential
        ///
        /// `SSWS `, `OAuth ` and `Token token=` are public API syntax, printed in the vendor's own
        /// documentation; `<token>` is a runtime secret this repository never holds. The seam is
        /// deliberately shaped so that only the first can be written here: the credential is always
        /// appended by the host, so there is no substitution point an author could aim at. The
        /// loader additionally refuses a prefix that spells a resolution marker, names a declared
        /// credential or its environment variable, or contains anything but visible ASCII —
        /// see `provider::validate`.
        ///
        /// # Why there is no `suffix`, and no template
        ///
        /// [C-161](../../../docs/stories/C-161-provider-okta.md) measured all three blocked vendors
        /// and found one shape, not three: the credential **ends the value** in every case, PagerDuty
        /// included, whose `Token token=` is a prefix that happens to contain `=`. A template with a
        /// substitution point would be more expressive and strictly worse — it can spell a credential
        /// substituted twice, or zero times, and the zero case sends an unauthenticated request that
        /// reads as authenticated. A prefix makes both unspellable rather than merely refused.
        /// `docs/designs/unified-auth.md` §"The prefix axis, as built" records the decision.
        ///
        /// # Why the presets are still variants
        ///
        /// [`Bearer`](Self::Bearer) and [`Basic`](Self::Basic) *are* prefixes — `"Bearer "` and
        /// `"Basic "` on `Authorization` — and the catalogue lowers all three to one
        /// `Placement::Header { name, prefix }`. They stay named variants because `Basic` also
        /// selects an *acquisition* (the base64 join), and because collapsing `Bearer` into a
        /// spelled-out prefix would move 15 providers' committed artifacts to say something they
        /// already say. Being unit variants is also what makes `bearer` + a second prefix
        /// unspellable.
        #[serde(default, skip_serializing_if = "String::is_empty")]
        prefix: String,
    },
    /// A query parameter `?<name>=<secret>`.
    Query {
        /// The query parameter name.
        name: String,
    },
    /// A shared secret that is **never placed in a request** — it verifies an inbound one.
    ///
    /// **This is the one variant that is not in `flux_plugin_protocol::AuthScheme`**, and the mirror
    /// is broken deliberately rather than by drift. Every other variant answers "where does this
    /// secret go on the way out"; a webhook signing secret has no answer, because it never goes out
    /// at all. It is read by the host to recompute an HMAC over bytes that arrived
    /// ([`HmacSpec::secret`](crate::HmacSpec::secret)).
    ///
    /// The alternative was a second credential namespace beside `[[auth]]`, and it is worse: a
    /// connector manifest names every credential the connector requires (vision principle 5), and an
    /// operator provisioning one would have had to know that inbound secrets live somewhere else.
    /// One namespace, one list, one place to look — at the cost of a variant flux's plugin protocol
    /// has no use for, since a plugin has no inbound direction to verify.
    Signing,
}

/// **Whose authority a credential carries when it is used** (C-528).
///
/// This is the "on behalf of" axis, and it is independent of every other one. [`AuthScheme`] says
/// where the value goes; [`OAuth2Spec`] says how it is obtained; this says *who the vendor thinks is
/// acting* once it arrives. Slack is the case that forces it: one OAuth v2 grant returns two tokens
/// in one response — `access_token` is the workspace's bot (`xoxb-`) and
/// `authed_user.access_token` is the signed-in person (`xoxp-`). They are placed identically, they
/// are acquired by the same grant, and they differ in nothing an existing axis can express, while
/// differing in **who they can act as and how much they can reach**.
///
/// Three consequences ride on it, which is why it is a declaration rather than a host convention:
///
/// - **Blast radius.** An app-subject token carries the integration's own grant across the whole
///   workspace; a user-subject token is bounded by one person's permissions. A host that cannot tell
///   them apart cannot bound either.
/// - **Who supplies it.** An app credential is provisioned once by whoever runs the product; a user
///   credential is provisioned once *per person*, and storing one under a tenant-wide address would
///   let one member act as another.
/// - **Whether a grant may be delegated at all.** Only a user-subject credential can answer "act on
///   behalf of the signed-in user".
///
/// # Why there is an `Unstated` variant, and why it is the default
///
/// [`OperationDirection`](crate::OperationDirection) has no default: C-516 reviewed all 829
/// operations and made both front-ends state it. That is the stronger design and it is not available
/// here yet — 55 connectors ship credentials today and **none has been reviewed for this**, some
/// genuinely ambiguously: `providers/github.toml`'s single `github.token` is documented as covering
/// GitHub App installation tokens (app-subject) *and* personal access tokens (user-subject), which
/// is one declaration standing for two different answers.
///
/// So the default is [`Unstated`](Self::Unstated) and it means exactly *"nobody has reviewed this"*,
/// not *"app"*. A host needing the distinction **refuses on `Unstated`** rather than assuming the
/// safer-sounding value, because assuming `app` over-grants and assuming `user` silently fails.
/// Defaulting to `App` would have been the marking `AGENTS.md` warns about: one that reads as a
/// safety decision while recording only that the question was never asked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Subject {
    /// **Not reviewed.** Carries no claim in either direction; a consumer that needs the answer
    /// refuses rather than choosing one.
    #[default]
    Unstated,
    /// The integration itself. Slack's `xoxb-` bot token, a GitHub App installation token.
    App,
    /// The person who granted it — "on behalf of". Slack's `xoxp-` user token, an Atlassian 3LO
    /// token, a personal access token.
    User,
}

impl Subject {
    /// The stable spelling published by manifests and catalogues.
    pub const fn word(self) -> &'static str {
        match self {
            Self::Unstated => "unstated",
            Self::App => "app",
            Self::User => "user",
        }
    }

    /// Whether this is the unreviewed default, which is what `skip_serializing_if` keys on so that
    /// no already-published artifact moves.
    pub const fn is_unstated(&self) -> bool {
        matches!(self, Self::Unstated)
    }
}

/// A token grant an [`OAuth2Spec`] credential allows the host to run. Mirrors
/// `flux_plugin_protocol::OAuthGrant`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuthGrant {
    /// Browser + loopback-callback PKCE.
    AuthorizationCode,
    /// Resource-owner password grant — babelforce's flow.
    Password,
    /// Refresh an expired access token from a stored refresh token.
    RefreshToken,
    /// Two-legged client-credentials grant (no user).
    ClientCredentials,
}

/// The loopback redirect an `authorization_code` login binds. Mirrors
/// `flux_plugin_protocol::OAuthRedirect`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthRedirect {
    /// The loopback port to bind.
    pub port: u16,
    /// The callback path the browser is redirected to.
    pub path: String,
}

/// Declares that an [`AuthMethod`] is OAuth2-backed: the host runs every token grant and injects
/// only a fresh bearer, so nothing this repo generates ever performs OAuth itself.
///
/// Mirrors `flux_plugin_protocol::OAuth2Spec` field for field, including its all-defaulted shape —
/// a credential with no `oauth2` block is a plain env-to-secret credential and must round-trip
/// unchanged.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OAuth2Spec {
    /// The declared endpoint name whose base URL the paths below resolve against — its host
    /// allow-list is what admits the token exchange through flux's egress gate.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub endpoint: String,
    /// The declared service name whose base URL the [`token_path`](Self::token_path) resolves
    /// against, when the token endpoint lives on a **different host** from the authorize endpoint
    /// (C-556). Empty — the default and every shipped declaration's value — means the token exchange
    /// resolves against [`endpoint`](Self::endpoint), which is today's behaviour byte-for-byte.
    ///
    /// # Why a second service *name* and never a URL
    ///
    /// Anthropic's subscription flow authorizes on `claude.ai` and redeems its token on
    /// `platform.claude.com`; one `endpoint` cannot express two hosts. A URL here would name a host
    /// nothing had admitted — the host set is derived from **declared services**, so `http_hosts`,
    /// declared-authority validation and X-154's `NoDeclaredDefault` composition rule all keep
    /// working only because this is a reference into that set. The loader refuses a name no
    /// `[[services]]` entry declares, for exactly the reason a bare URL is unacceptable.
    ///
    /// # The consumer contract (X-154)
    ///
    /// A host redeems the token by joining [`token_path`](Self::token_path) onto **this** service's
    /// base URL when it is set, and onto [`endpoint`](Self::endpoint)'s otherwise — the declared
    /// defaults rule applies to it identically. The authorize leg is unaffected: it always resolves
    /// against [`endpoint`](Self::endpoint).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub token_endpoint: String,
    /// The authorize endpoint path, joined onto the endpoint base URL.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub authorize_path: String,
    /// The token endpoint path (every grant and refresh POSTs here). It resolves against
    /// [`token_endpoint`](Self::token_endpoint)'s service base URL when that is set, and against
    /// [`endpoint`](Self::endpoint)'s otherwise.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub token_path: String,
    /// The OAuth2 client id. Never a secret — the client *secret* is resolved from env like any
    /// other credential and never appears in a provider TOML or a generated artifact.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub client_id: String,
    /// Requested scopes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    /// The grants the host may run for this credential.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grants: Vec<OAuthGrant>,
    /// The loopback redirect for the `authorization_code` login flow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redirect: Option<OAuthRedirect>,
    /// **Whether this is a public PKCE client that issues and uses no client secret** (C-556).
    ///
    /// `false` — the default and every shipped declaration's value — is a *confidential* client: it
    /// registers a client secret, which a hosted product supplies once at the operator level. A
    /// public client (Anthropic's Console OAuth is the first) authenticates the token exchange with
    /// PKCE alone, so there is no secret for an operator to supply and asking for one would collect a
    /// value nothing uses.
    ///
    /// # The consumer contract
    ///
    /// A public client still needs a `client_id`, which is public by specification, but must **not**
    /// be required to publish a `client_secret`. `auth_archetypes.rs` reads this discriminator: it
    /// requires the secret operator-level `oauth.client_secret` config field only when the client is
    /// confidential. The flag is a property of *how the client authenticates*, on the acquisition
    /// axis, independent of grant, placement and subject.
    #[serde(default, skip_serializing_if = "is_false")]
    pub public_client: bool,
}

/// `skip_serializing_if` for a `bool` that defaults to `false`, so a confidential OAuth2 client — the
/// case every shipped declaration is in — serializes with no `public_client` key and no already
/// published artifact moves.
fn is_false(value: &bool) -> bool {
    !*value
}

/// **A named weakness in how a credential is obtained** (C-440).
///
/// This is the axis a host refuses on. An operator who wants no password-grant authentication
/// anywhere says that once, about a declared property, and every connector carrying it refuses —
/// including the fifty-sixth, added next month by somebody who never read their policy. The
/// alternative is a list of connector names, which is correct on the day it is written and silently
/// wrong afterwards.
///
/// # Why the set is closed, and refused rather than defaulted
///
/// A free-form `hazard = "..."` string carrying its own citations is more expressive and strictly
/// worse, because it makes the consuming filter a string match. A near-miss spelling matches no
/// allow-list entry, reads as *no hazard declared*, and is admitted by the very deployment that
/// refused the thing it names. So an unrecognised spelling is a loader refusal — serde's own
/// `unknown variant` message, which names the rejected value and the accepted one, exactly as
/// [`Runtime`](crate::Runtime) already does. The cost is that a new hazard is a deliberate edit
/// here, and that cost is the point.
///
/// # This is a *kind*. [`Risk`](crate::Risk) is a *level*
///
/// They are not interchangeable, and the difference is load-bearing because `Risk` is **ordered**
/// and selectors compare against that ordering. A hazard has no position on that ladder: a password
/// grant that buys a **read-only** token is `Risk::Low` *and* hazardous. A fifth rung would be wrong
/// in one direction or the other — high enough to catch it and every destructive operation inherits
/// a weakness it does not have; low enough not to and an `at_most` selector silently admits
/// password-grant authentication to every rule an operator has already written.
///
/// It is likewise **not** a field on an operation. A hazard is a property of an *acquisition*, which
/// happens once per connection; an operation happens per call. Putting it on
/// [`Operation`](crate::Operation) would restate one connection's fact on babelforce's 389 rows and
/// invite a per-call answer to a per-connection question.
///
/// # The spelling is a contract with a consumer that already shipped
///
/// flux-exchange's `crates/exchange-host/src/acquisition.rs` maps its own `AuthHazard` variant to
/// the string `resource_owner_secret_shared`, and its `FLUX_EXCHANGE_ALLOW_AUTH_HAZARDS` deployment
/// gate filters on exactly that word. What this repository emits is what that filter reads, which is
/// why [`word`](Self::word) matches exhaustively rather than deferring to the serde encoding, and
/// why `tests/auth_hazard.rs` pins the two together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthHazard {
    /// The resource owner's own password is presented to **the host** rather than to the
    /// authorization server — the OAuth 2.0 resource owner password credentials grant.
    ///
    /// The name states the property a filter is written against and an auditor can check — *the
    /// resource owner's secret was shared* — rather than naming the grant that happens to be the
    /// shipped instance of it.
    ///
    /// **RFC 9700 §2.4** (OAuth 2.0 Security Best Current Practice, 2025) says the grant **MUST
    /// NOT** be used, for three reasons that together are the whole hazard: it exposes the resource
    /// owner's credentials to the client; it widens where those credentials can leak, beyond the
    /// authorization server; and it cannot carry two-factor or any other multi-step authentication.
    /// **RFC 6749 §4.3** makes discarding them once a token is obtained a MUST for the client, and
    /// a host running this grant *is* that client. **CWE-522**, *Insufficiently Protected
    /// Credentials*, is the nearest weakness-catalogue entry. OAuth 2.1 drops the grant entirely.
    ///
    /// It remains worth declaring rather than refusing outright, because a vendor that offers
    /// nothing else offers this or nothing — which is a decision for a deployment to make knowingly.
    ResourceOwnerSecretShared,
}

impl AuthHazard {
    /// The stable published spelling, which a host's deployment filter matches as an exact word.
    ///
    /// Matched exhaustively rather than obtained by serialising the enum. Deployment policy is
    /// configuration an operator audits, and changing a serde attribute must not silently change the
    /// string that grants a weaker authentication path.
    pub const fn word(self) -> &'static str {
        match self {
            Self::ResourceOwnerSecretShared => "resource_owner_secret_shared",
        }
    }
}

/// **What a connector's authentication surface does that its own document does not say** (C-440).
///
/// The word and its discipline are already here — `quirks.pagination` and `quirks.rate_limit` are
/// declarations rather than behaviour, and reach the IR and the loader and no artifact. What is new
/// is the **scope**: [`Quirks`](crate::Quirks) hangs off an operation, and an authentication
/// endpoint is never an operation, so a token endpoint's measured departures had nowhere to live.
///
/// Kept deliberately narrow. Owner-decided 2026-08-02: *if it is not in the specification, it does
/// not become a general thing*. The occasion was a token lifetime, and the argument against
/// generalising it is one vendor's own numbers — babelforce's token endpoint reads a request
/// `expires_in` its document never declares, defaulting it to *never expires* on one grant, clamping
/// it to sixty seconds on another, and ignoring it on a third. A general `requested_ttl` would be a
/// hard cap here, ignored there, and the difference between an hour and forever somewhere else,
/// while inviting fifty-four other providers to be assumed to honour something none of them
/// declares.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthQuirks {
    /// Measured behaviours of the credential's token endpoint, at most one per vendor `grant_type`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub token_endpoint: Vec<TokenEndpointQuirk>,
}

impl AuthQuirks {
    /// Whether the credential declares no quirk at all — the case every shipped connector but one
    /// is in, and what `skip_serializing_if` keys on so no published manifest moved.
    pub fn is_empty(&self) -> bool {
        self.token_endpoint.is_empty()
    }
}

/// One measured departure of a token endpoint from the document that describes it.
///
/// Every field is required, and [`attribution`](Self::attribution) and [`measured`](Self::measured)
/// are why. A quirk is asserted against a vendor's *implementation* and contradicted by that
/// vendor's own *document*; a reader a year from now needs to know which of the two this repository
/// checked and when, or the declaration is indistinguishable from a guess that aged.
/// `providers/babelforce.toml` already carries one unattributed open question to a vendor's API
/// owners that nobody can now answer, which is what an unattributed claim costs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenEndpointQuirk {
    /// The vendor's own `grant_type` word this measurement is about.
    ///
    /// A string rather than an [`OAuthGrant`], and deliberately. `OAuthGrant` mirrors flux's
    /// vocabulary field for field, and a vendor's undeclared grant is precisely the thing that
    /// vocabulary does not have a variant for — babelforce serves a fifth, `link`, that appears in
    /// no document and in no specification. A quirk that could only name grants the specification
    /// already knows would be unable to record the ones worth recording.
    pub grant: String,
    /// What was measured, in prose.
    ///
    /// Prose rather than a field, because a field is a promise every other connector is then assumed
    /// to keep. See [`AuthQuirks`] for the ruling this follows.
    pub behaviour: String,
    /// What was read, and by whom — enough for a reader to repeat the measurement or date it.
    pub attribution: String,
    /// The day the measurement was made, `YYYY-MM-DD`. The loader refuses anything else, because
    /// "recently" does not let a reader decide whether it predates the vendor release they are
    /// debugging.
    pub measured: String,
}

/// One credential a connector declares: its name, and how it reaches the wire.
///
/// Mirrors `flux_plugin_protocol::AuthMethod`, except that flux's identifying field is called
/// `purpose` and ours is [`name`](Self::name) — see this module's docs. The generated Flux carries
/// only that name, as flux's `{"$auth": {"purpose": "babelforce.access_id"}}` marker, and flux
/// resolves `env` to a value, applies `scheme`, and registers the result with its redactor.
///
/// **No credential value ever appears in this type**, in a provider TOML, in a generated `.flux`
/// file, or in the lockfile; `env` and `user_env` name environment variables, never their contents.
/// [`user_suffix`](Self::user_suffix) is the one literal string the type carries, and it is
/// deliberately not a credential — it is Zendesk's `/token` marker, which is public API syntax.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthMethod {
    /// The credential name an [`AuthRequirement`] references, e.g. `"babelforce.access_id"`. It
    /// says what the credential *is*. Resolves to flux's `AuthMethod.purpose` at the manifest
    /// boundary.
    pub name: String,
    /// How the resolved secret is injected into the request.
    ///
    /// **Mandatory on deserialization, deliberately.** [`AuthScheme`] still has a `Default` of
    /// `Bearer` — flux's does too, and the mirror has to hold — but a *provider TOML* that omits
    /// `scheme` must be rejected rather than quietly given one. This is the same reasoning
    /// [`Risk`](crate::Risk) and [`Idempotency`](crate::Idempotency) carry: how a secret reaches the
    /// wire is not a decision to make by silence, and the silent answer here is "send it as a
    /// bearer token", which is a request going out with the wrong credential shape.
    pub scheme: AuthScheme,
    /// Env-var **keys** to resolve the secret from, tried in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<String>,
    /// For [`AuthScheme::Basic`]: env-var keys holding the username half, tried in order. These are
    /// config rather than a gated secret, so they resolve directly from declared env.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub user_env: Vec<String>,
    /// For [`AuthScheme::Basic`]: a literal string appended to the resolved
    /// [`user_env`](Self::user_env) value before the `user:secret` join.
    ///
    /// Zendesk is why. Its user half is `<email>/token` — an env value **plus a literal suffix** —
    /// where the trailing `/token` is what tells Zendesk the password is an API token rather than a
    /// password (`docs/designs/auth-seam.md:74`, `docs/designs/provider-operation-inventory.md:163`).
    /// Without this field the only way to author Zendesk is to tell the operator to paste
    /// `me@corp.com/token` into `ZENDESK_USER`, which stores a value that is not the thing it is
    /// named after and that nothing can validate — the pre-composed-credential mistake
    /// [`auth-seam.md §7.5`](../../../docs/designs/auth-seam.md) rejects explicitly.
    ///
    /// **Scope.** This is the *authoring* half only, and it is deliberately the narrower of the two
    /// gaps §7.5 records. The sibling gap — Freshdesk's `base64(<api_key>:X)`, where the **secret
    /// occupies the user position** and the password is a literal — is a security question about
    /// which half is gated and redacted, and `provider-operation-inventory.md §6.2` reserves it for
    /// C-16 to decide. Nothing here pre-empts that. Composing the half host-side is flux's F-9 /
    /// C-274; until it lands, a connector can *say* what Zendesk needs even though flux cannot yet
    /// apply it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_suffix: Option<String>,
    /// Human-readable description, surfaced when a user is asked to supply the credential.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// When set, this credential is OAuth2-backed and the host runs the token grants. `None` for a
    /// plain env-to-secret credential.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth2: Option<OAuth2Spec>,
    /// Whose authority this credential carries when it is used — see [`Subject`].
    ///
    /// Defaults to [`Subject::Unstated`] and is skipped when it is, so adding this axis moved no
    /// already-published manifest byte.
    #[serde(default, skip_serializing_if = "Subject::is_unstated")]
    pub subject: Subject,
    /// The declared weakness in **obtaining** this credential, when it has one — see [`AuthHazard`].
    ///
    /// `None` is the case every connector but babelforce is in, and it means *no weakness is
    /// declared* rather than *this was reviewed and found safe*. A host's fail-closed default is
    /// written against the presence of a value, so an absent field admits and a declared one is
    /// refused unless a deployment opted in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hazard: Option<AuthHazard>,
    /// What this credential's authentication surface does that its own document does not say — see
    /// [`AuthQuirks`].
    #[serde(default, skip_serializing_if = "AuthQuirks::is_empty")]
    pub quirks: AuthQuirks,
}

impl AuthMethod {
    /// A bearer-token credential: `Authorization: Bearer <env>`.
    pub fn bearer(name: impl Into<String>, env: Vec<String>) -> Self {
        Self {
            name: name.into(),
            scheme: AuthScheme::Bearer,
            env,
            ..Self::default()
        }
    }

    /// A Basic credential: `Authorization: Basic base64(<user_env><user_suffix>:<env>)`.
    ///
    /// The suffix is separate from `user_env` on purpose — see
    /// [`user_suffix`](Self::user_suffix). Pass `None` for a vendor whose user half is a bare env
    /// value, `Some("/token")` for Zendesk.
    pub fn basic(
        name: impl Into<String>,
        user_env: Vec<String>,
        user_suffix: Option<String>,
        env: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            scheme: AuthScheme::Basic,
            env,
            user_env,
            user_suffix,
            ..Self::default()
        }
    }

    /// A custom-header credential whose whole value is the secret: `<header>: <env>`.
    pub fn header(name: impl Into<String>, header: impl Into<String>, env: Vec<String>) -> Self {
        Self::prefixed_header(name, header, "", env)
    }

    /// A custom-header credential carrying a literal scheme word: `<header>: <prefix><env>`.
    ///
    /// `prefix` is public API syntax — `"SSWS "`, `"OAuth "`, `"Token token="` — and never any part
    /// of a credential value. See [`AuthScheme::Header::prefix`].
    pub fn prefixed_header(
        name: impl Into<String>,
        header: impl Into<String>,
        prefix: impl Into<String>,
        env: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            scheme: AuthScheme::Header {
                name: header.into(),
                prefix: prefix.into(),
            },
            env,
            ..Self::default()
        }
    }
}

/// **One mechanism** for authenticating an operation: every credential named here must be
/// satisfied on the *same* request.
///
/// This is the AND half of the model. `AuthRequirement::all(["b.access_id", "b.access_token"])`
/// means both headers travel together — it is not two alternatives, and it does not collapse to one
/// credential. The OR half is the surrounding `Vec<AuthRequirement>` on
/// [`Operation::auth`](crate::Operation::auth) and
/// [`Connector::default_auth`](crate::Connector::default_auth).
///
/// The credentials are held in an [`IndexSet`], whose iteration order is insertion order and
/// therefore never depends on a hash seed the way a `HashSet` would. On top of that, the set is
/// **normalized to sorted order** on construction and on deserialization, so the encoding is a
/// function of the set's *members* alone and not of the order an author happened to type them. That
/// is stricter than "identical inputs produce identical output" requires, and deliberately so:
/// `connectors.lock` (C-7) hashes this, and two connectors that are `==` must never hash apart.
/// Order carries no meaning within a mechanism — all its credentials are sent on one request — so
/// sorting loses nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthRequirement {
    #[serde(deserialize_with = "deserialize_canonical_credentials")]
    credentials: IndexSet<String>,
}

impl AuthRequirement {
    /// A mechanism satisfied only when **all** the given credentials are (AND). Duplicates collapse
    /// and the result is held in canonical order.
    pub fn all<I, S>(credentials: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            credentials: canonicalize(credentials.into_iter().map(Into::into)),
        }
    }

    /// A mechanism naming exactly one credential — the common case, and a readable shorthand for
    /// [`all`](Self::all) with a single element.
    pub fn single(credential: impl Into<String>) -> Self {
        Self::all([credential.into()])
    }

    /// The credentials this mechanism names, in canonical order.
    pub fn credentials(&self) -> &IndexSet<String> {
        &self.credentials
    }

    /// Iterates the credential names in canonical order.
    pub fn iter(&self) -> indexmap::set::Iter<'_, String> {
        self.credentials.iter()
    }

    /// Whether the mechanism names the given credential.
    pub fn contains(&self, credential: &str) -> bool {
        self.credentials.contains(credential)
    }

    /// How many credentials must be satisfied together.
    pub fn len(&self) -> usize {
        self.credentials.len()
    }

    /// Whether the mechanism names no credential at all.
    ///
    /// An empty mechanism is degenerate — "no auth" is expressed as an empty *alternatives list*
    /// (`Some(vec![])` on an operation), not as a list holding one empty mechanism. Rejecting it is
    /// the provider loader's job (C-3), not the IR's.
    pub fn is_empty(&self) -> bool {
        self.credentials.is_empty()
    }
}

impl<'a> IntoIterator for &'a AuthRequirement {
    type Item = &'a String;
    type IntoIter = indexmap::set::Iter<'a, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.credentials.iter()
    }
}

/// Sorts and deduplicates into an [`IndexSet`], giving the canonical order described on
/// [`AuthRequirement`].
fn canonicalize<I: IntoIterator<Item = String>>(credentials: I) -> IndexSet<String> {
    let mut sorted: Vec<String> = credentials.into_iter().collect();
    sorted.sort_unstable();
    sorted.dedup();
    sorted.into_iter().collect()
}

/// Deserializes credential names through [`canonicalize`], so a hand-authored TOML that lists them
/// in any order still produces the one canonical value.
fn deserialize_canonical_credentials<'de, D>(deserializer: D) -> Result<IndexSet<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(canonicalize(Vec::<String>::deserialize(deserializer)?))
}
