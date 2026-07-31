//! **The connection-configuration port**: where a connector's *non-secret* tenant values —
//! `{subdomain}`, `{shop}`, a Basic user half — become the text a request is built from.
//!
//! It is the sibling of [`crate::credentials`], and it exists because the credential port answered
//! only half the question. A resolved token addressed at
//! `tenants/<tenant>/<authority>/<credential>` is worthless against
//! `https://{subdomain}.zendesk.com`: the request is authenticated and the host does not resolve.
//!
//! # Bound at construction, never looked up — and never read from the environment
//!
//! [`Configuration`] is an argument to [`crate::pack`], exactly as [`Credentials`](crate::Credentials)
//! is, and every operation it installs holds the one that was handed in. There is no global, no
//! `OnceLock`, no ambient default, and — the part this module is stricter about than the code it
//! replaced — **no process-environment fallback**. A tenant's subdomain is that tenant's value; a
//! variable in the server's own environment can only ever hold one of them, so reading one would
//! make a multi-tenant host quietly single-tenant and would do it without an error.
//!
//! # The port is synchronous, and that is forced rather than chosen
//!
//! [`Tool::permission_subjects`](flux_runtime::Tool::permission_subjects) returns a `Vec<String>`
//! and **cannot fail and cannot await**. It is also the only place flux's egress allow-list is
//! consulted for this pack's inner call (see [`crate::tool`]), so the substituted host has to be
//! computable from inside it. An `async` port would be unusable there, and the subject would fall
//! back to the un-substituted template — the exact failure this module exists to remove.
//!
//! So a host that keeps connection settings in a database resolves them **eagerly**, when it builds
//! the pack, and binds a snapshot. That is the right posture independently: a subdomain is stable
//! configuration rather than a per-request lookup, and doing IO inside a permission check would put
//! a network round trip in front of every gate decision.
//!
//! # The snapshot is taken here, not asked of the host (C-198)
//!
//! Advising a host to bind a snapshot is not the same as requiring one, and the difference is a
//! hole through the egress allow-list. [`Tool::permission_subjects`](flux_runtime::Tool::permission_subjects)
//! and [`Tool::execute`](flux_runtime::Tool::execute) are two calls; each used to read this port
//! independently, so a [`ConfigStore`] with interior mutability could answer the **gate** with one
//! host and the **request** with another. The pack calls `http.request`'s `execute` directly, which
//! is why that is a bypass rather than an inconsistency — see [`crate::tool`].
//!
//! So the pack no longer trusts the advice: [`Configuration::snapshot`] resolves every value one
//! operation can ask for **once**, at [`Operation::project`](crate::Operation::project), and the
//! projected operation holds the resulting [`Snapshot`] and no handle to the store at all. A store
//! that would have answered twice is consulted once, so there is no second answer to diverge. The
//! requirement on [`ConfigStore::get`] is stated anyway, because a host may resolve the same field
//! for two operations, and those are still two reads.
//!
//! # What is *not* here
//!
//! No file format, no environment convention, no discovery. The port takes values from the host;
//! how a host obtains them is the host's business.
//!
//! Nor does this module publish a connector's configuration *surface* — the labels, help text and
//! `binds` targets [the design](../../../docs/designs/connector-configuration.md) specifies. That is
//! C-87's, it is a breaking change to the manifest and the catalogue, and it is deliberately not
//! required to make a URL resolve: the variables an operation needs are read off the operation's own
//! emitted Flux (see [`crate::request::endpoint_variables`]), so this port lands against the IR as
//! it stands.

use std::collections::BTreeMap;
use std::sync::Arc;

use connector_secrets::validate_tenant;

use crate::Error;

/// **Which non-secret value is being asked for.**
///
/// The two variants are the two non-secret, connection-level rows of the design's `binds` table —
/// `endpoint.<var>` and `username.<name>`. The secret rows (`credential.<name>`,
/// `oauth.client_secret`) are deliberately absent: they are [`Credentials`](crate::Credentials)'
/// job, and a value that arrives through this port carries no redaction guarantee, so letting a
/// secret through it would be a downgrade disguised as a convenience.
///
/// Not `#[non_exhaustive]`: a host implementing [`ConfigStore`] must decide what to do with every
/// kind of value the pack can ask for, and a new kind should be a compile error at that decision
/// rather than a `None` that reads as "not configured".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Field<'a> {
    /// A `{var}` in a **connector's** `base_url` — `subdomain`, `shop`, `site`, `account_host`.
    ///
    /// The name is the placeholder as the connector's own Flux spells it, with no braces.
    ///
    /// # It is keyed by connector, not by service — and that is a defect, not a simplification
    ///
    /// The key is `(tenant, provider, kind, name)`, with **no service in it**, because
    /// `catalog::Operation` carries no service for this port to key on. So two services of one
    /// connector that spell the same variable in their own `base_url` collapse to **one value**.
    ///
    /// `contentful` is the shipped case: `delivery_space_id` and `management_space_id` are two
    /// declared configuration fields, both binding `endpoint.space_id` under different services
    /// (`providers/contentful.toml`), and this port can hold only one of them. A tenant whose
    /// delivery and management environments differ reads the wrong one and gets a `200` — a real
    /// answer from a real space, which is why nothing here refuses.
    ///
    /// [C-197](../../../docs/stories/C-197-config-collapses-across-services.md) is the fix. It needs
    /// `service` on `catalog::Operation`, which moves every generated artifact and is a breaking
    /// change to a published type, so it is a story of its own rather than a caveat on this one.
    Endpoint(&'a str),
    /// The **non-secret** user half of a `basic` credential, named by the credential it joins —
    /// `zendesk.api_token`, `jira.api_token`.
    ///
    /// Zendesk's `user/token` suffix is *not* part of this value: the suffix is the connector's
    /// declared data (`Acquisition::BasicJoin::user_suffix`) and is appended by the pack, so a host
    /// binds the plain account identifier and cannot get the join wrong.
    Username(&'a str),
}

impl Field<'_> {
    /// The kind, as a stable key a store can index on.
    fn kind(&self) -> &'static str {
        match self {
            Field::Endpoint(_) => "endpoint",
            Field::Username(_) => "username",
        }
    }

    /// The name, without its kind.
    fn name(&self) -> &str {
        match self {
            Field::Endpoint(name) | Field::Username(name) => name,
        }
    }

    /// How the field is spelled in a diagnostic, and in the design's `binds` vocabulary.
    fn binding(&self) -> String {
        format!("{}.{}", self.kind(), self.name())
    }

    /// The field as a [`Snapshot`] key: the kind, and the name owned.
    ///
    /// The kind is `&'static str` and only the name is allocated, which is what lets a snapshot
    /// outlive the borrowed `Field` it was taken for.
    fn key(&self) -> (&'static str, String) {
        (self.kind(), self.name().to_owned())
    }
}

/// **A host's connection settings**, as the pack asks for them.
///
/// Synchronous and infallible by signature — see the module documentation for why
/// `permission_subjects` leaves no other option. `None` means "this tenant has not configured it",
/// and the pack turns that into a refusal naming the field rather than a request to a host with a
/// brace in it.
pub trait ConfigStore: Send + Sync {
    /// The value bound to `field` of `provider`, for `tenant`.
    ///
    /// # It must be stable, and that is a requirement rather than an expectation
    ///
    /// **An implementation must answer the same `(tenant, provider, field)` with the same value for
    /// as long as the store is bound.** A store that reads a database on every call, or a cache that
    /// can expire between two calls, does not satisfy this and must resolve its values eagerly and
    /// hand over a fixed set instead — which is what [`MemoryConfig`] is for.
    ///
    /// The consequence of breaking it is specific. A connector's host is *substituted from this
    /// port*, and the pack calls `http.request`'s `execute` directly, bypassing
    /// `Executor::dispatch` — so [`Tool::permission_subjects`](flux_runtime::Tool::permission_subjects)
    /// on the projected operation is the only place a host's egress allow-list is consulted for the
    /// inner call. A store that answers the gate `gate.example.com` and the request
    /// `elsewhere.example.com` sends traffic through the allow-list to a host that was never
    /// checked, and leaves an audit record naming the host that was never called.
    ///
    /// The pack does not merely rely on this: `Configuration::snapshot` reads every value an
    /// operation can ask for **once**, when the operation is projected, so within one operation a
    /// drifting store has no second call to drift on. The requirement remains because a host binds
    /// one store across many operations, and each projection is a fresh read — a store that drifts
    /// between them gives two connectors two different views of one tenant.
    fn get(&self, tenant: &str, provider: &str, field: Field<'_>) -> Option<String>;
}

/// **The configuration adapter a host binds when it constructs the pack.**
///
/// Holds a store and the tenant every lookup is made for, mirroring
/// [`Credentials`](crate::Credentials) exactly — including the tenant validation, so a
/// misconfiguration is a startup failure rather than a runtime one. Cloning is cheap and shares the
/// store, which is what lets one bound port serve every operation of every provider in a pack.
#[derive(Clone)]
pub struct Configuration {
    values: Arc<dyn ConfigStore>,
    tenant: String,
}

/// `Arc<dyn ConfigStore>` is not `Debug`, and the tenant is the part worth seeing. The store is
/// unnamed for the same reason [`Credentials`](crate::Credentials)' is: a connection setting is not
/// a secret, but it is a customer's, and it has no business in a log line nobody asked for.
impl std::fmt::Debug for Configuration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Configuration")
            .field("tenant", &self.tenant)
            .finish_non_exhaustive()
    }
}

impl Configuration {
    /// Bind `values` as the place this pack's connection settings are read from, for `tenant`.
    ///
    /// # Errors
    ///
    /// [`Error::Tenant`] when `tenant` is not a usable identifier — empty, over-long, or a spelling
    /// that would traverse. The same validation [`Credentials::new`](crate::Credentials::new)
    /// applies, and it is applied here too so the two ports cannot disagree about what a tenant is.
    pub fn new(values: Arc<dyn ConfigStore>, tenant: &str) -> Result<Self, Error> {
        validate_tenant(tenant).map_err(|reason| Error::Tenant {
            tenant: tenant.to_owned(),
            reason,
        })?;
        Ok(Self {
            values,
            tenant: tenant.to_owned(),
        })
    }

    /// The tenant every lookup this port makes is for.
    pub fn tenant(&self) -> &str {
        &self.tenant
    }

    /// **Read every field in `fields` once**, and hand back the frozen result.
    ///
    /// This is the enforcement point for [`ConfigStore::get`]'s stability requirement, and the
    /// reason it can exist at all is that the set of fields is knowable in advance: an operation's
    /// endpoint variables come from its own emitted Flux and its Basic user halves from its
    /// connector's declared credentials, both `&'static` catalogue data. So the pack never has to
    /// ask the store a question it did not already know it would ask.
    ///
    /// An empty value is dropped rather than stored, so [`Snapshot`] holds only values that are
    /// actually bound — left alone, an empty subdomain would substitute into `https://.zendesk.com`,
    /// a host that does not resolve, arrived at without an error.
    pub(crate) fn snapshot<'a>(
        &self,
        provider: &'static str,
        fields: impl IntoIterator<Item = Field<'a>>,
    ) -> Snapshot {
        let values = fields
            .into_iter()
            .filter_map(|field| {
                self.values
                    .get(&self.tenant, provider, field)
                    .filter(|value| !value.is_empty())
                    .map(|value| (field.key(), value))
            })
            .collect();
        Snapshot {
            tenant: self.tenant.clone(),
            provider,
            values,
        }
    }
}

/// **One operation's connection settings, resolved once and frozen.**
///
/// The value [`Configuration::snapshot`] produces, held by a projected
/// [`Operation`](crate::Operation) *in place of* the port it came from. That substitution is the
/// whole point: an operation holding a [`Configuration`] can read the store twice, and one holding a
/// `Snapshot` cannot read it at all. The gate and the request are then two reads of the same map
/// rather than two calls to a store that may have changed its mind.
///
/// It carries the tenant and the connector alongside the values because both appear in every refusal
/// this port produces, and neither is reachable once the port itself has been dropped.
#[derive(Debug, Clone)]
pub(crate) struct Snapshot {
    tenant: String,
    provider: &'static str,
    /// Keyed by `(kind, name)` — the same partition [`Field::kind`] draws, so an endpoint variable
    /// and a credential of one spelling stay two values.
    values: BTreeMap<(&'static str, String), String>,
}

impl Snapshot {
    /// The tenant these settings belong to.
    pub(crate) fn tenant(&self) -> &str {
        &self.tenant
    }

    /// One field, or the refusal that names what is missing.
    ///
    /// # Errors
    ///
    /// [`Error::MissingConfig`], naming the tenant, the connector and the `binds` target — the three
    /// facts an operator needs in order to go and supply the value.
    pub(crate) fn require(&self, operation: &str, field: Field<'_>) -> Result<String, Error> {
        self.lookup(field).ok_or_else(|| Error::MissingConfig {
            operation: operation.to_owned(),
            provider: self.provider.to_owned(),
            tenant: self.tenant.clone(),
            field: field.binding(),
        })
    }

    /// One field, best-effort — for the one caller that cannot fail.
    ///
    /// [`Tool::permission_subjects`](flux_runtime::Tool::permission_subjects) returns a `Vec` and
    /// has nowhere to put a refusal, so its fallback path substitutes what it has and leaves the
    /// rest verbatim. That is deliberately **fail-closed**: an unsubstituted `{subdomain}` is a
    /// subject no allow-list matches, so the call is refused by the gate rather than admitted
    /// against a subject nobody can audit.
    pub(crate) fn lookup(&self, field: Field<'_>) -> Option<String> {
        self.values.get(&field.key()).cloned()
    }
}

/// **An in-memory [`ConfigStore`]**, for a host that already holds its settings and for tests.
///
/// The counterpart of [`MemoryStore`](connector_secrets::MemoryStore) on the credential side, and
/// the shape a host binding a snapshot wants: build it once, hand it over, never touch it again.
#[derive(Debug, Default, Clone)]
pub struct MemoryConfig {
    /// Keyed by `(tenant, provider, kind, name)` — the whole address, so one instance can serve
    /// every tenant a host knows about rather than one per tenant.
    values: BTreeMap<(String, String, &'static str, String), String>,
}

impl MemoryConfig {
    /// An empty store. A pack bound to one refuses every templated connector by name, which is the
    /// correct starting state rather than an unhelpful one.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind a `{var}` of `provider`'s base URL for `tenant`.
    #[must_use]
    pub fn with_endpoint(self, tenant: &str, provider: &str, variable: &str, value: &str) -> Self {
        self.with(tenant, provider, Field::Endpoint(variable), value)
    }

    /// Bind the non-secret user half of `provider`'s `credential` for `tenant`.
    #[must_use]
    pub fn with_username(
        self,
        tenant: &str,
        provider: &str,
        credential: &str,
        value: &str,
    ) -> Self {
        self.with(tenant, provider, Field::Username(credential), value)
    }

    fn with(mut self, tenant: &str, provider: &str, field: Field<'_>, value: &str) -> Self {
        self.values.insert(
            (
                tenant.to_owned(),
                provider.to_owned(),
                field.kind(),
                field.name().to_owned(),
            ),
            value.to_owned(),
        );
        self
    }
}

impl ConfigStore for MemoryConfig {
    fn get(&self, tenant: &str, provider: &str, field: Field<'_>) -> Option<String> {
        self.values
            .get(&(
                tenant.to_owned(),
                provider.to_owned(),
                field.kind(),
                field.name().to_owned(),
            ))
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tenant id reaches no path from this port, but it must still be the *same* notion of a
    /// tenant the credential port validates — the two are asserted to agree at install
    /// ([`crate::Error::TenantMismatch`]), and an agreement between two differently-validated
    /// strings would be worth nothing.
    #[test]
    fn a_traversing_tenant_is_refused_when_the_port_is_bound() {
        let error = Configuration::new(Arc::new(MemoryConfig::new()), "../../etc")
            .expect_err("a traversing tenant cannot name a connection");
        assert!(matches!(error, Error::Tenant { .. }), "{error}");
    }

    /// The two kinds share a namespace of names — a connector could plausibly have an endpoint
    /// variable and a credential of the same spelling — so the kind has to be part of the key.
    #[test]
    fn an_endpoint_and_a_username_of_the_same_name_are_different_values() {
        let store = MemoryConfig::new()
            .with_endpoint("t", "acme", "account", "acme-endpoint")
            .with_username("t", "acme", "account", "acme-username");

        assert_eq!(
            store.get("t", "acme", Field::Endpoint("account")),
            Some("acme-endpoint".to_string())
        );
        assert_eq!(
            store.get("t", "acme", Field::Username("account")),
            Some("acme-username".to_string())
        );
    }

    /// One store, many tenants — the shape a host binding a snapshot per tenant needs, and the one
    /// that makes a cross-tenant mix-up visible rather than structural.
    #[test]
    fn a_value_belongs_to_one_tenant() {
        let store = MemoryConfig::new().with_endpoint("t-one", "zendesk", "subdomain", "one");

        assert_eq!(
            store.get("t-one", "zendesk", Field::Endpoint("subdomain")),
            Some("one".to_string())
        );
        assert_eq!(
            store.get("t-two", "zendesk", Field::Endpoint("subdomain")),
            None
        );
    }

    /// An empty string is not a value. Left alone it would substitute into
    /// `https://.zendesk.com` — a host that does not resolve, arrived at without an error.
    #[test]
    fn an_empty_value_is_missing_rather_than_bound() {
        let configuration = Configuration::new(
            Arc::new(MemoryConfig::new().with_endpoint("t", "zendesk", "subdomain", "")),
            "t",
        )
        .expect("a valid tenant id");
        let settings = configuration.snapshot("zendesk", [Field::Endpoint("subdomain")]);

        assert!(settings.lookup(Field::Endpoint("subdomain")).is_none());
        let error = settings
            .require("zendesk-ticket-show", Field::Endpoint("subdomain"))
            .expect_err("an empty subdomain is not a subdomain");
        assert!(error.to_string().contains("endpoint.subdomain"), "{error}");
    }

    /// **The stability requirement, enforced rather than advised (C-198).** A snapshot answers from
    /// what it read, so a store that changes its mind afterwards changes nothing about a projected
    /// operation — the property `permission_subjects` and `execute` depend on for agreeing about
    /// which host a call reaches.
    #[test]
    fn a_snapshot_answers_from_what_it_read_rather_than_from_the_store() {
        /// A store that answers with the number of times it has been asked.
        #[derive(Default)]
        struct Counting(std::sync::atomic::AtomicUsize);

        impl ConfigStore for Counting {
            fn get(&self, _tenant: &str, _provider: &str, _field: Field<'_>) -> Option<String> {
                Some(
                    self.0
                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                        .to_string(),
                )
            }
        }

        let store = Arc::new(Counting::default());
        let configuration = Configuration::new(store.clone(), "t").expect("a valid tenant id");
        let settings = configuration.snapshot("zendesk", [Field::Endpoint("subdomain")]);

        assert_eq!(
            settings.lookup(Field::Endpoint("subdomain")).as_deref(),
            Some("0")
        );
        assert_eq!(
            settings.lookup(Field::Endpoint("subdomain")).as_deref(),
            Some("0")
        );
        assert_eq!(
            store.0.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the snapshot went back to the store"
        );
    }

    /// A field the snapshot was never taken for is missing rather than fetched. Without this a
    /// `Snapshot` would be a cache with a hole in it, and the hole would be a live store call on
    /// exactly the path that must not make one.
    #[test]
    fn a_field_outside_the_snapshot_is_missing_rather_than_looked_up() {
        let configuration = Configuration::new(
            Arc::new(
                MemoryConfig::new()
                    .with_endpoint("t", "zendesk", "subdomain", "acme")
                    .with_username("t", "zendesk", "zendesk.api_token", "ops@acme.test"),
            ),
            "t",
        )
        .expect("a valid tenant id");
        let settings = configuration.snapshot("zendesk", [Field::Endpoint("subdomain")]);

        assert_eq!(
            settings.lookup(Field::Endpoint("subdomain")).as_deref(),
            Some("acme")
        );
        assert!(settings
            .lookup(Field::Username("zendesk.api_token"))
            .is_none());
    }
}
