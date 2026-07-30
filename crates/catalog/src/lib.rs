//! Every generated connector operation, embedded at compile time.
//!
//! This is flux-connectors made consumable with `cargo add` instead of by copying artifacts into
//! `~/.flux/flows`. Adding the crate *is* getting the catalog: every operation's Flux source and
//! the metadata a caller needs to decide whether to run it are `&'static` data baked into the
//! binary by `include_str!` and a generated table. There is no filesystem lookup, no parsing, no
//! initialization, and no dependency.
//!
//! It stays inside the charter (`AGENTS.md`): a library that hands out **text**, not a runtime.
//! Nothing here executes an operation or touches the network — flux does that, from the module it
//! loads.
//!
//! ```
//! use catalog::{OperationKey, ProviderKey, Risk};
//!
//! let show = catalog::operation(OperationKey::id("zendesk-ticket-show")).unwrap();
//! assert_eq!(show.risk, Risk::Low);
//! assert!(show.flux.starts_with("op zendesk-ticket-show("));
//!
//! // "every operation in this provider" is one call.
//! let zendesk = catalog::operations_of(ProviderKey::id("zendesk"));
//! assert!(zendesk.iter().all(|operation| operation.provider == "zendesk"));
//! ```
//!
//! # What is generated and what is not
//!
//! `flux-connectors build` writes three kinds of file into this crate, all committed and reviewed
//! like every other generated artifact in the repository. **Nothing here is hand-written.**
//!
//! - `ops/<provider>/<operation>.flux` — one rendering per operation, byte for byte the same text
//!   the provider's module carries for it;
//! - `src/generated/<provider>.rs` — the table that embeds those renderings alongside their
//!   metadata;
//! - `src/generated.rs` — [`generated`], the index naming every provider's module.
//!
//! The first two are **per provider**, so `flux-connectors build --provider zendesk` regenerates
//! exactly what it compiled. The index is not: it names every provider at once, so it is written by
//! a **full** build only and a scoped run leaves the committed file untouched rather than rendering
//! an index that drops the providers the run never looked at. `web/public/catalog.json` follows the
//! same rule for the same reason.
//!
//! It was hand-written until C-104 on exactly that reasoning, and the cost was structural: two lines
//! per provider, appended by hand, made this the one file every provider story touched — so any two
//! provider stories conflicted and connectors shipped one at a time. `tests/embedded_operations.rs`
//! still holds the list against `providers/`, which is now a staleness check on a committed artifact
//! rather than a reminder to add a line.
//!
//! `crates/connector-cli/tests/catalog_artifacts.rs` is what makes the embedded data a *checked*
//! artifact: it recomputes everything here from `providers/*.toml` and compares byte for byte, so a
//! stale catalog — which still compiles and still answers every query — fails the build.
//!
//! # The per-provider module is still what ships
//!
//! `connectors/<provider>.flux` is unchanged in role: it is the artifact flux loads from
//! `~/.flux/flows`, and it declares every one of that provider's operations. The renderings here
//! are **additional** — the catalog's unit, not a substitute for the module — and each one is a
//! substring of it (`tests/embedded_operations.rs` pins that).
//!
//! # Addressing, and what C-37 changes
//!
//! Operations are keyed today by [`Operation::id`], the declarable Flux symbol
//! (`zendesk-ticket-show`). C-37 introduces a global address — an `oip` such as
//! `com.zendesk.api/support/tickets:v2#show` — and the key types here exist so that lands
//! **additively**: [`OperationKey`] and [`ProviderKey`] are opaque, constructed only through named
//! constructors, so C-37 adds `OperationKey::oip` and `ProviderKey::pid` and no signature in this
//! crate moves. There is deliberately no `From<&str>` for either: a bare string cannot say whether
//! it is a symbol or an address, and guessing is exactly the ambiguity two identifiers exist to
//! avoid.

mod generated;

/// How much damage an operation can do, in flux's own vocabulary.
///
/// A mirror of `connector_spec::Risk`, not a re-export: this crate has no dependencies, which is
/// what makes it cheap to add. The mapping is exhaustive at the point of generation
/// (`connector-cli`'s `catalog` module matches on every variant), so a variant added upstream is a
/// compile error there rather than a silent omission here, and
/// `tests/embedded_operations.rs::metadata_agrees_with_the_embedded_flux` pins every value against
/// the `risk "…"` line of the Flux it describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Risk {
    /// Reads, and writes that cannot surprise anyone.
    Low,
    /// Writes with limited blast radius.
    Medium,
    /// Writes a reviewer would want to see first.
    High,
    /// Deletes or otherwise irreversible.
    Destructive,
}

impl Risk {
    /// The spelling flux's approval gate reads, and the one the embedded Flux declares.
    pub const fn as_str(self) -> &'static str {
        match self {
            Risk::Low => "low",
            Risk::Medium => "medium",
            Risk::High => "high",
            Risk::Destructive => "destructive",
        }
    }
}

/// Whether repeating an operation is safe, in flux's own vocabulary.
///
/// Mirrors `connector_spec::Idempotency` for the same reason [`Risk`] does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Idempotency {
    /// Repeating the call has the same effect as making it once.
    Idempotent,
    /// Repeating the call repeats its effect.
    NonIdempotent,
    /// Idempotent only under a condition the caller supplies (e.g. an idempotency key).
    Conditional,
}

impl Idempotency {
    /// The spelling the embedded Flux declares.
    pub const fn as_str(self) -> &'static str {
        match self {
            Idempotency::Idempotent => "idempotent",
            Idempotency::NonIdempotent => "non_idempotent",
            Idempotency::Conditional => "conditional",
        }
    }
}

/// One operation: its Flux source, and what a caller needs in order to decide whether to use it.
///
/// `#[non_exhaustive]` because C-37 adds the global address to this struct and C-10 adds the
/// resolved endpoint spec; neither should be a breaking change for a consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Operation {
    /// The Flux symbol the operation is declared and called by, e.g. `zendesk-ticket-show`. Unique
    /// across the whole catalog, and the key [`OperationKey::id`] matches on.
    pub id: &'static str,
    /// The [`Provider::id`] this operation belongs to.
    pub provider: &'static str,
    /// What the operation does, in one line — the same text the model sees as the tool description.
    pub description: &'static str,
    /// How much damage it can do.
    pub risk: Risk,
    /// Whether repeating it is safe.
    pub idempotency: Idempotency,
    /// The credentials the operation needs, as **alternatives of mechanisms**: the outer slice is
    /// an OR over ways to authenticate, and each inner slice is the set of credentials that must
    /// all be satisfied on the same request (AND).
    ///
    /// `&[&["babelforce.access_id", "babelforce.access_token"]]` is one mechanism needing two
    /// headers together, not two ways to authenticate — the distinction babelforce forces and that
    /// a flat list of credential names cannot express. An empty outer slice means the operation
    /// needs no credential at all.
    ///
    /// The names are credential *references*. No secret, and no environment variable's value, is
    /// ever in this crate — flux resolves the reference and applies the scheme.
    pub credentials: &'static [&'static [&'static str]],
    /// The hosts a call reaches, as the connector's base URL spells them — templating included, so
    /// Zendesk is `{subdomain}.zendesk.com` rather than a tenant nobody has chosen yet. What a
    /// caller does with this is decide whether their egress policy admits the call.
    pub hosts: &'static [&'static str],
    /// The operation's Flux source: exactly the `op` declaration
    /// `connectors/<provider>.flux` carries for it.
    pub flux: &'static str,
}

/// **Where a credential goes on the way out** — the *placement* axis of
/// `docs/designs/unified-auth.md`.
///
/// The design's central claim is carried by one field: `prefix` on [`Header`](Self::Header) is what
/// turns `Bearer` from an enum variant into data, so `Bearer `, `Basic `, `Token `, `GenieKey ` and
/// the empty prefix are one code path rather than five variants. A flat scheme enum conflates
/// *where* a secret goes with *what it is prefixed by*, and needs a new variant the moment a vendor
/// wants `Token <t>` in `Authorization`.
///
/// Deliberately **not** `#[non_exhaustive]`, unlike [`Provider`] and [`Operation`]. A consumer
/// placing a credential must match every variant, and a new placement must be a compile error at
/// every such site rather than a silently skipped credential — a request that quietly went out
/// without one is exactly the failure this type exists to prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Placement {
    /// `<name>: <prefix><value>`. `Authorization: Bearer <t>` is
    /// `Header { name: "Authorization", prefix: "Bearer " }`; a raw-value header such as
    /// `X-Shopify-Access-Token` is the same variant with an empty prefix.
    Header {
        /// The header name.
        name: &'static str,
        /// The literal text the value is prefixed with, **trailing space included**. Empty for a
        /// raw-value header.
        prefix: &'static str,
    },
    /// `?<name>=<value>` — a query-parameter key.
    Query {
        /// The parameter name.
        name: &'static str,
    },
    /// **Never placed on an outgoing request.** A webhook signing secret verifies bytes that
    /// arrived, so it has no answer to "where does this go on the way out" — the one deliberate
    /// divergence from flux's vocabulary, recorded in `AGENTS.md`'s authentication contract. It is
    /// here so that one credential namespace covers both directions.
    Inbound,
}

/// **How stored material becomes the value that is placed** — the *acquisition* axis.
///
/// Only the two pure acquisitions the shipped catalogue needs are modelled. Effectful ones —
/// `oauth2`, `session` — are the host's, by the line `docs/designs/unified-auth.md` draws: they need
/// a network round trip, a token cache and refresh-on-401, and a connector that ran them would have
/// to pass a raw token through a bound symbol.
///
/// Not `#[non_exhaustive]`, for the reason [`Placement`] is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Acquisition {
    /// The stored secret, unchanged.
    Static,
    /// `base64(<user><user_suffix>:<secret>)` — HTTP Basic, composed rather than pre-composed.
    ///
    /// The user half is **config, not a gated secret**: it is an email address or an account name,
    /// and it resolves from the declared environment variables rather than from the secret store.
    /// `user_suffix` is Zendesk's `/token` marker — public API syntax, and the reason a bare env
    /// value is not enough (`docs/designs/auth-seam.md` §7.5).
    ///
    /// The secret occupies the **password** position. Freshdesk's shape, where the API key occupies
    /// the *username* position, is not expressible here because the IR cannot yet say so either
    /// (C-16 owns that decision); Freshdesk therefore declares no credential at all, which
    /// `AGENTS.md` records as an intentional gap rather than an oversight.
    BasicJoin {
        /// Environment-variable **keys** holding the user half, tried in order. Never a value.
        user_env: &'static [&'static str],
        /// A literal appended to the resolved user half before the `user:secret` join.
        user_suffix: &'static str,
    },
}

/// One credential a connector declares: what it is called, where its value is kept, and how it
/// reaches the wire.
///
/// **No value, ever.** [`Credential::leaf`] is the last segment of a *path*; [`Acquisition`] names
/// environment variables. Nothing in this crate holds a secret, and
/// `crates/connector-cli/tests/no_secrets.rs` is what keeps that a checked statement.
///
/// Not `#[non_exhaustive]`: a consumer constructs one in a test to prove its own placement code, and
/// the type is small, declarative and complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Credential {
    /// The flat-namespace name an operation references, e.g. `zendesk.api_token`. Vendor-prefixed,
    /// because credentials share one namespace across the connector.
    pub name: &'static str,
    /// The last segment of the credential's address — `api_token`, not `zendesk.api_token`. The
    /// path already carries the authority, so the vendor prefix would be said twice.
    pub leaf: &'static str,
    /// How stored material becomes the value that is placed.
    pub acquire: Acquisition,
    /// Where that value goes on the request.
    pub place: Placement,
}

/// One connector, and every operation the catalog carries for it.
///
/// `#[non_exhaustive]` for the same reason [`Operation`] is: C-37's `pid` lands here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Provider {
    /// The connector id, e.g. `zendesk`. Names `connectors/<id>.flux`, and is the key
    /// [`ProviderKey::id`] matches on.
    pub id: &'static str,
    /// The vendor's display name.
    pub vendor: &'static str,
    /// What the connector is for, in one line.
    pub description: &'static str,
    /// The reverse-DNS authority the connector publishes under (`com.slack.api`) — C-37's `pid`,
    /// and the second segment of every credential address this connector's secrets live at.
    ///
    /// `None` for a connector that has not declared one yet. That is not cosmetic: without an
    /// authority no credential address renders at all, so a consumer resolving credentials can only
    /// refuse. Refusing is the correct answer — a request sent without the credential it declares is
    /// the failure worth preventing — but the diagnostic must say *which* fact is missing.
    pub authority: Option<&'static str>,
    /// The API base URL, templating included.
    pub base_url: &'static str,
    /// Every credential the connector declares, in declaration order and keyed by
    /// [`Credential::name`] — the vocabulary [`Operation::credentials`] references by name.
    ///
    /// Declared at **provider** level, exactly as the IR declares them: a credential belongs to the
    /// connector, not to one of its services, which is why its address elides the service segment.
    pub auth: &'static [Credential],
    /// Every operation, in the order the provider declares them — which is also the order they
    /// appear in `connectors/<id>.flux`.
    pub operations: &'static [Operation],
}

impl Provider {
    /// One of this provider's operations, by key.
    pub fn operation(&self, key: OperationKey<'_>) -> Option<&'static Operation> {
        self.operations
            .iter()
            .find(|operation| key.matches(operation))
    }

    /// The credential this connector declares under `name`, or `None` when nothing declares it.
    ///
    /// A lookup rather than a map because the list is short, ordered and generated; the ordering is
    /// the connector's own, and an alternative-selection rule that depends on declaration order
    /// needs it preserved.
    pub fn credential(&self, name: &str) -> Option<&'static Credential> {
        self.auth.iter().find(|credential| credential.name == name)
    }
}

/// How a caller names one operation.
///
/// A key type rather than a bare `&str` so that C-37's global address becomes a *constructor* —
/// `OperationKey::oip(…)` — instead of a reshape of every lookup in the crate. The field is
/// private and there is no `From<&str>`: a caller states which kind of name they hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OperationKey<'a> {
    /// The Flux symbol. C-37 turns this field into an enum over `{ id, oip }`; because the field is
    /// private and every constructor is named, that is an additive change.
    id: &'a str,
}

impl<'a> OperationKey<'a> {
    /// Name an operation by its Flux symbol, e.g. `zendesk-ticket-show`.
    pub const fn id(id: &'a str) -> Self {
        Self { id }
    }

    /// Whether this key names `operation`.
    fn matches(&self, operation: &Operation) -> bool {
        operation.id == self.id
    }
}

/// How a caller names one provider.
///
/// The sibling of [`OperationKey`], and additive in the same way: C-37's `pid`
/// (`com.zendesk.api`) lands as `ProviderKey::pid`, and its middle level — the `gid`, a versioned
/// resource group — lands as a `GroupKey` beside it without disturbing anything here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProviderKey<'a> {
    /// The connector id. C-37 turns this field into an enum over `{ id, pid }`.
    id: &'a str,
}

impl<'a> ProviderKey<'a> {
    /// Name a provider by its connector id, e.g. `zendesk`.
    pub const fn id(id: &'a str) -> Self {
        Self { id }
    }

    /// Whether this key names `provider`.
    fn matches(&self, provider: &Provider) -> bool {
        provider.id == self.id
    }
}

/// Every provider in the catalog, ordered by [`Provider::id`].
pub fn providers() -> &'static [&'static Provider] {
    generated::PROVIDERS
}

/// One provider, by key.
pub fn provider(key: ProviderKey<'_>) -> Option<&'static Provider> {
    providers()
        .iter()
        .copied()
        .find(|provider| key.matches(provider))
}

/// **Every operation in one provider** — the listing that makes a provider addressable rather than
/// merely nameable.
///
/// An unknown provider yields an empty slice rather than an error; use [`provider`] when the
/// difference between "no such connector" and "a connector with nothing in it" matters.
pub fn operations_of(key: ProviderKey<'_>) -> &'static [Operation] {
    provider(key).map_or(&[], |provider| provider.operations)
}

/// Every operation in the catalog, provider by provider.
pub fn operations() -> impl Iterator<Item = &'static Operation> {
    providers()
        .iter()
        .flat_map(|provider| provider.operations.iter())
}

/// **The lookup.** One operation by key, with its Flux source and its metadata.
///
/// A linear scan: the catalog holds tens of operations today and low hundreds once spec ingest
/// selects more (a spec-ingested babelforce alone offers 163), which is well inside the range where
/// a scan over contiguous `&'static` data beats any index worth maintaining — and an index would be
/// a second generated artifact to keep in step with this one.
pub fn operation(key: OperationKey<'_>) -> Option<&'static Operation> {
    operations().find(|operation| key.matches(operation))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_operation_is_found_by_its_symbol() {
        let operation = operation(OperationKey::id("zendesk-ticket-show"))
            .expect("the shipped catalog carries zendesk-ticket-show");
        assert_eq!(operation.provider, "zendesk");
        assert!(operation.flux.contains("op zendesk-ticket-show("));
    }

    #[test]
    fn an_unknown_key_is_none_rather_than_a_panic() {
        assert!(operation(OperationKey::id("zendesk-ticket-obliterate")).is_none());
        assert!(provider(ProviderKey::id("salesforce")).is_none());
        assert!(operations_of(ProviderKey::id("salesforce")).is_empty());
    }

    /// Listing by provider is the whole point of the middle level, so it must agree with the flat
    /// listing rather than being a second, drifting source.
    #[test]
    fn listing_by_provider_partitions_the_catalog() {
        let listed: usize = providers()
            .iter()
            .map(|provider| operations_of(ProviderKey::id(provider.id)).len())
            .sum();
        assert_eq!(listed, operations().count());
        assert!(
            listed > 0,
            "an empty catalog would pass every other test here"
        );
    }

    /// Ids are what lookups key on, so a duplicate would make one of the two unreachable.
    #[test]
    fn operation_ids_are_unique_across_the_catalog() {
        let mut ids: Vec<&str> = operations().map(|operation| operation.id).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "duplicate operation id in the catalog");
    }

    #[test]
    fn providers_are_listed_in_a_stable_order() {
        let ids: Vec<&str> = providers().iter().map(|provider| provider.id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
    }
}
