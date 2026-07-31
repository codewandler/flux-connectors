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
    /// A `{var}` in a service's `base_url` — `subdomain`, `shop`, `site`, `account_host`.
    ///
    /// The name is the placeholder as the connector's own Flux spells it, with no braces.
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
}

/// **A host's connection settings**, as the pack asks for them.
///
/// Synchronous and infallible by signature — see the module documentation for why
/// `permission_subjects` leaves no other option. `None` means "this tenant has not configured it",
/// and the pack turns that into a refusal naming the field rather than a request to a host with a
/// brace in it.
pub trait ConfigStore: Send + Sync {
    /// The value bound to `field` of `provider`, for `tenant`.
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

    /// One field of `provider`'s configuration, or the refusal that names what is missing.
    ///
    /// # Errors
    ///
    /// [`Error::MissingConfig`], naming the tenant, the connector and the `binds` target — the three
    /// facts an operator needs in order to go and supply the value.
    pub(crate) fn require(
        &self,
        operation: &str,
        provider: &str,
        field: Field<'_>,
    ) -> Result<String, Error> {
        self.values
            .get(&self.tenant, provider, field)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| Error::MissingConfig {
                operation: operation.to_owned(),
                provider: provider.to_owned(),
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
    pub(crate) fn lookup(&self, provider: &str, field: Field<'_>) -> Option<String> {
        self.values
            .get(&self.tenant, provider, field)
            .filter(|value| !value.is_empty())
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

        assert!(configuration
            .lookup("zendesk", Field::Endpoint("subdomain"))
            .is_none());
        let error = configuration
            .require(
                "zendesk-ticket-show",
                "zendesk",
                Field::Endpoint("subdomain"),
            )
            .expect_err("an empty subdomain is not a subdomain");
        assert!(error.to_string().contains("endpoint.subdomain"), "{error}");
    }
}
