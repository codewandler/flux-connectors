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
//! # The key names the service, because the connector does (C-197)
//!
//! A sole-connection value is addressed by **`(tenant, provider, service, kind, name)`**; an
//! instance-aware host additionally keys it by the selected C-406 UUID. The service is not
//! decoration: a *service* owns its own `base_url`, so a `{var}` is a variable of one service's URL
//! and two services of one connector can spell the same one. `contentful` does —
//! `delivery_space_id` and `management_space_id` both bind `endpoint.space_id`, under `delivery`
//! and `management` respectively, because a configuration field's `name` is unique across the whole
//! connector while the placeholder it fills belongs to one service's template.
//!
//! Keying without the service collapsed those two into one slot, and the failure was silent in the
//! worst way available: `contentful-entry-create` on `api.contentful.com` resolved whatever
//! `contentful-entry-get` on `cdn.contentful.com` had been given, so a tenant whose two
//! environments differ got a **write into a space nobody named** — a `200` from a real server with
//! a real management token, not a refusal. The service reached this port through
//! [`catalog::Operation::service`], which is the field C-197 added for it.
//!
//! Every kind is keyed this way, including [`Field::Username`], and that follows the IR rather than
//! being a simplification: `connector_spec::ConfigField::service` is exactly one concrete service
//! for *every* field, whatever it binds. The consequence is worth stating — a connector with two
//! services and a `basic` credential must have its user half bound under each service that asks for
//! it — and it fails closed, as [`Error::MissingConfig`](crate::Error::MissingConfig), rather than
//! by quietly reading the other service's value.
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

use connector_secrets::{validate_tenant, InstanceId};

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
    /// A `{var}` in a **service's** `base_url` — `subdomain`, `shop`, `site`, `account_host`.
    ///
    /// The name is the placeholder as the connector's own Flux spells it, with no braces.
    ///
    /// # It is keyed by service, and that is what makes two of them two values (C-197)
    ///
    /// The key is **`(tenant, provider, service, kind, name)`**, and the service is load-bearing
    /// rather than ornamental: a `base_url` belongs to a *service*, so the placeholder in it does
    /// too, and two services of one connector may spell the same one.
    ///
    /// `contentful` is the shipped case and the reason this sentence is no longer a lie. Its
    /// `delivery_space_id` and `management_space_id` are two declared configuration fields, both
    /// binding `endpoint.space_id`, under `delivery` and `management` respectively
    /// (`providers/contentful.toml`). Keyed without the service they were one slot, so
    /// `contentful-entry-create` on `api.contentful.com` wrote into whatever space
    /// `contentful-entry-get` on `cdn.contentful.com` had been given — a `200` from a real server,
    /// which is why nothing refused. They are now two values, and a service whose value is unbound
    /// refuses by name instead of borrowing its sibling's.
    Endpoint(&'a str),
    /// The **non-secret** user half of a `basic` credential, named by the credential it joins —
    /// `zendesk.api_token`, `jira.api_token`.
    ///
    /// Zendesk's `user/token` suffix is *not* part of this value: the suffix is the connector's
    /// declared data (`Acquisition::BasicJoin::user_suffix`) and is appended by the pack, so a host
    /// binds the plain account identifier and cannot get the join wrong.
    Username(&'a str),
    /// One connection-time query value of one socket binding.
    ChannelQuery {
        /// The channel binding name.
        channel: &'a str,
        /// The vendor query parameter name.
        parameter: &'a str,
    },
}

impl Field<'_> {
    /// Interpret one placeholder emitted for request configuration.
    ///
    /// Bare names are the established endpoint/request-pin vocabulary. `username.` is the one
    /// reserved qualifier: it lets a Basic username also scope a request without being looked up
    /// through the unrelated endpoint address. Provider validation prevents any other field kind
    /// from minting this prefix.
    pub(crate) fn from_placeholder(variable: &str) -> Field<'_> {
        variable
            .strip_prefix("username.")
            .filter(|name| !name.is_empty())
            .map(Field::Username)
            .unwrap_or(Field::Endpoint(variable))
    }

    /// The kind, as a stable key a store can index on.
    fn kind(&self) -> &'static str {
        match self {
            Field::Endpoint(_) => "endpoint",
            Field::Username(_) => "username",
            Field::ChannelQuery { .. } => "channel_query",
        }
    }

    /// The name, without its kind.
    fn name(&self) -> &str {
        match self {
            Field::Endpoint(name) | Field::Username(name) => name,
            Field::ChannelQuery { parameter, .. } => parameter,
        }
    }

    /// How the field is spelled in a diagnostic, and in the design's `binds` vocabulary.
    fn binding(&self) -> String {
        match self {
            Self::ChannelQuery { channel, parameter } => {
                format!("channel.{channel}.query.{parameter}")
            }
            _ => format!("{}.{}", self.kind(), self.name()),
        }
    }

    /// The field as a [`Snapshot`] key: the kind, and the name owned.
    ///
    /// The kind is `&'static str` and only the name is allocated, which is what lets a snapshot
    /// outlive the borrowed `Field` it was taken for.
    fn key(&self) -> (&'static str, String) {
        let name = match self {
            Self::ChannelQuery { channel, parameter } => format!("{channel}\0{parameter}"),
            _ => self.name().to_owned(),
        };
        (self.kind(), name)
    }
}

/// **A host's connection settings**, as the pack asks for them.
///
/// Synchronous and infallible by signature — see the module documentation for why
/// `permission_subjects` leaves no other option. `None` means "this tenant has not configured it",
/// and the pack turns that into a refusal naming the field rather than a request to a host with a
/// brace in it.
pub trait ConfigStore: Send + Sync {
    /// The value bound to `field` of `provider`'s `service`, for `tenant`.
    ///
    /// # The service is part of the address, not a hint (C-197)
    ///
    /// `service` is the operation's own — `delivery`, `s3`, or the reserved `default` for a
    /// connector with one surface — and it is what keeps two services' same-named variables apart.
    /// An implementation that ignores it re-creates the defect this parameter exists to remove: see
    /// the module documentation for `contentful`, where the collapsed key sent a management write
    /// to a space the operator never named.
    ///
    /// This is the whole address, so an implementation answering `None` for a service it was never
    /// given a value for is correct. The pack turns that into [`Error::MissingConfig`], which names
    /// the service.
    ///
    /// # It must be stable, and that is a requirement rather than an expectation
    ///
    /// **An implementation must answer the same `(tenant, provider, instance, service, field)` with the same value for
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
    fn get(&self, tenant: &str, provider: &str, service: &str, field: Field<'_>) -> Option<String>;

    /// The value for a selected connection instance.
    ///
    /// The compatibility default delegates only when no instance was selected. A named instance
    /// never borrows the sole connection's value: existing stores compile unchanged and fail
    /// closed until they deliberately implement instance-aware lookup.
    fn get_for_instance(
        &self,
        tenant: &str,
        provider: &str,
        instance: Option<&InstanceId>,
        service: &str,
        field: Field<'_>,
    ) -> Option<String> {
        match instance {
            None => self.get(tenant, provider, service, field),
            Some(_) => None,
        }
    }

    /// Resolve a value and its operator-approval state as one immutable answer.
    ///
    /// Approval is connection-scoped state, so it travels through the instance-aware lookup and
    /// is frozen alongside the value. The compatibility default deliberately marks every value as
    /// proposed: stores written before operator-approved origins therefore fail closed, and a
    /// store that supports approval must override this method rather than answering two calls that
    /// could race or address different connection instances.
    fn resolve_for_instance(
        &self,
        tenant: &str,
        provider: &str,
        instance: Option<&InstanceId>,
        service: &str,
        field: Field<'_>,
    ) -> Option<ConfigValue> {
        self.get_for_instance(tenant, provider, instance, service, field)
            .map(ConfigValue::proposed)
    }
}

/// One connection setting and the policy decision that may activate it.
///
/// The fields stay private so an implementation cannot accidentally construct an approved value
/// without choosing the named constructor at the point where operator policy was evaluated.
#[derive(Clone, PartialEq, Eq)]
pub struct ConfigValue {
    value: String,
    operator_approved: bool,
}

/// A setting may be safe to store without being safe to copy into a log. Preserve the policy bit
/// that helps diagnose refusal while keeping the customer-provided value out of debug output.
impl std::fmt::Debug for ConfigValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigValue")
            .field("operator_approved", &self.operator_approved)
            .finish_non_exhaustive()
    }
}

impl ConfigValue {
    /// A value supplied by a connection owner but not activated by operator policy.
    pub fn proposed(value: String) -> Self {
        Self {
            value,
            operator_approved: false,
        }
    }

    /// A value approved and pinned by deployment/operator policy for this connection.
    pub fn operator_approved(value: String) -> Self {
        Self {
            value,
            operator_approved: true,
        }
    }

    /// The resolved value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Whether deployment/operator policy activated this exact resolution.
    pub fn is_operator_approved(&self) -> bool {
        self.operator_approved
    }
}

/// **The configuration adapter a host binds when it constructs the pack.**
///
/// Holds a store, tenant and optional connection UUID every lookup is made for, mirroring
/// [`Credentials`](crate::Credentials) exactly — including the tenant validation, so a
/// misconfiguration is a startup failure rather than a runtime one. Cloning is cheap and shares the
/// store, which is what lets one bound port serve every operation of every provider in a pack.
#[derive(Clone)]
pub struct Configuration {
    values: Arc<dyn ConfigStore>,
    tenant: String,
    instance: Option<InstanceId>,
}

/// `Arc<dyn ConfigStore>` is not `Debug`, and the tenant is the part worth seeing. The store is
/// unnamed for the same reason [`Credentials`](crate::Credentials)' is: a connection setting is not
/// a secret, but it is a customer's, and it has no business in a log line nobody asked for.
impl std::fmt::Debug for Configuration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Configuration")
            .field("tenant", &self.tenant)
            .field("instance", &self.instance)
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
            instance: None,
        })
    }

    /// Bind settings for one explicitly selected connection UUID.
    pub fn for_instance(
        values: Arc<dyn ConfigStore>,
        tenant: &str,
        instance: &str,
    ) -> Result<Self, Error> {
        let mut configuration = Self::new(values, tenant)?;
        configuration.instance =
            Some(
                InstanceId::parse(instance).map_err(|reason| Error::Instance {
                    instance: instance.to_owned(),
                    reason,
                })?,
            );
        Ok(configuration)
    }

    /// The tenant every lookup this port makes is for.
    pub fn tenant(&self) -> &str {
        &self.tenant
    }

    /// The selected connection UUID, absent for the sole-connection binding.
    pub fn instance(&self) -> Option<&InstanceId> {
        self.instance.as_ref()
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
    ///
    /// **An all-whitespace value is empty too** (C-214). The filter tested `is_empty` alone, so a
    /// `" "` survived it and travelled: `?teamId=%20` on every request of the service, or a host of
    /// `https://%20.zendesk.com`. A value made of whitespace is not a value an operator meant to
    /// supply, and treating it as absent produces [`Error::MissingConfig`] naming the field — which
    /// is the diagnostic they need — rather than a request nobody can explain.
    ///
    /// `service` is the operation's own, and it is what makes this a snapshot of **one service's**
    /// settings rather than of a connector's (C-197). Two operations of one connector in two
    /// services take two snapshots and read two sets of values, which is the point: they are calls
    /// to two different hosts.
    pub(crate) fn snapshot<'a>(
        &self,
        provider: &str,
        service: &str,
        fields: impl IntoIterator<Item = Field<'a>>,
    ) -> Snapshot {
        let values = fields
            .into_iter()
            .filter_map(|field| {
                self.values
                    .resolve_for_instance(
                        &self.tenant,
                        provider,
                        self.instance.as_ref(),
                        service,
                        field,
                    )
                    .filter(|resolved| !resolved.value().trim().is_empty())
                    .map(|resolved| (field.key(), resolved))
            })
            .collect();
        Snapshot {
            tenant: self.tenant.clone(),
            provider: provider.to_owned(),
            service: service.to_owned(),
            values,
        }
    }

    /// Snapshot every setting a channel handshake may read, applying declared optional defaults.
    pub(crate) fn channel_snapshot(
        &self,
        provider: &'static catalog::Provider,
        channel: &'static catalog::Channel,
    ) -> Snapshot {
        let mut values = BTreeMap::new();
        for declaration in provider
            .config
            .iter()
            .filter(|field| field.service == channel.service)
        {
            let field = if let Some(variable) = declaration.binds.strip_prefix("endpoint.") {
                Some(Field::Endpoint(variable))
            } else if let Some(credential) = declaration.binds.strip_prefix("username.") {
                Some(Field::Username(credential))
            } else {
                declaration
                    .binds
                    .strip_prefix("channel.")
                    .and_then(|rest| rest.split_once(".query."))
                    .filter(|(binding, _)| *binding == channel.name)
                    .map(|(binding, parameter)| Field::ChannelQuery {
                        channel: binding,
                        parameter,
                    })
            };
            let Some(field) = field else { continue };
            let value = self
                .values
                .get_for_instance(
                    &self.tenant,
                    provider.id,
                    self.instance.as_ref(),
                    channel.service,
                    field,
                )
                .filter(|value| !value.trim().is_empty())
                .or_else(|| declaration.default.map(str::to_owned));
            if let Some(value) = value {
                values.insert(field.key(), ConfigValue::proposed(value));
            }
        }
        Snapshot {
            tenant: self.tenant.clone(),
            provider: provider.id.to_owned(),
            service: channel.service.to_owned(),
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
/// It carries the tenant, the connector and the service alongside the values because all three
/// appear in every refusal this port produces, and none is reachable once the port itself has been
/// dropped.
#[derive(Debug, Clone)]
pub(crate) struct Snapshot {
    tenant: String,
    provider: String,
    /// The service these settings were read for. Part of the address every value here was fetched
    /// under, and quoted in the refusal, so an operator told that `endpoint.space_id` is missing is
    /// also told *which* of a connector's two `space_id`s to go and supply (C-197).
    ///
    /// Owned rather than `&'static str` so that a [`Rehearsal`](crate::Rehearsal) — whose connector
    /// is not in the index and whose names are therefore not `'static` — reads the port through
    /// exactly this type rather than through a second one that could answer differently (C-233).
    service: String,
    /// Keyed by `(kind, name)` — the same partition [`Field::kind`] draws, so an endpoint variable
    /// and a credential of one spelling stay two values. The service is **not** in this key and does
    /// not need to be: a snapshot is taken for exactly one service, so every value in it is already
    /// that service's.
    values: BTreeMap<(&'static str, String), ConfigValue>,
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
    /// [`Error::MissingConfig`], naming the tenant, the connector, the service and the `binds`
    /// target — the four facts an operator needs in order to go and supply the value. The service
    /// is one of them because two services of one connector may want the same `binds` target and
    /// different values (C-197); without it the refusal names a field an operator has two of.
    pub(crate) fn require(&self, operation: &str, field: Field<'_>) -> Result<String, Error> {
        self.lookup(field).ok_or_else(|| Error::MissingConfig {
            operation: operation.to_owned(),
            provider: self.provider.clone(),
            service: self.service.clone(),
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
        self.resolved(field)
            .map(|resolved| resolved.value().to_owned())
    }

    pub(crate) fn resolved(&self, field: Field<'_>) -> Option<&ConfigValue> {
        self.values.get(&field.key())
    }

    pub(crate) fn insert_default(&mut self, field: Field<'_>, value: &str) {
        self.values
            .entry(field.key())
            .or_insert_with(|| ConfigValue::proposed(value.to_owned()));
    }
}

/// **Adapts a frozen [`Snapshot`] to the engine-free [`connector_resolve::ConfigPort`]** (C-557).
///
/// The endpoint resolver and the Basic user half both moved to `connector-resolve`, and they read
/// their tenant values through this port. Wrapping the already-read [`Snapshot`] rather than the
/// live [`Configuration`] is what keeps the C-198 guarantee: the store was consulted once, at
/// projection, and this only re-presents the frozen result — a divergence between the gate and the
/// request cannot open here because there is nothing left to read.
pub(crate) struct SnapshotPort<'a>(pub(crate) &'a Snapshot);

impl connector_resolve::ConfigPort for SnapshotPort<'_> {
    fn resolve(
        &self,
        field: connector_resolve::ConfigField<'_>,
    ) -> Option<connector_resolve::ConfigValue> {
        let field = match field {
            connector_resolve::ConfigField::Endpoint(name) => Field::Endpoint(name),
            connector_resolve::ConfigField::Username(name) => Field::Username(name),
        };
        self.0.resolved(field).map(|resolved| {
            if resolved.is_operator_approved() {
                connector_resolve::ConfigValue::operator_approved(resolved.value())
            } else {
                connector_resolve::ConfigValue::proposed(resolved.value())
            }
        })
    }
}

/// **An in-memory [`ConfigStore`]**, for a host that already holds its settings and for tests.
///
/// The counterpart of [`MemoryStore`](connector_secrets::MemoryStore) on the credential side, and
/// the shape a host binding a snapshot wants: build it once, hand it over, never touch it again.
#[derive(Default, Clone)]
pub struct MemoryConfig {
    /// Keyed by `(tenant, provider, service, kind, name)` — the whole address, so one instance can
    /// serve every tenant a host knows about rather than one per tenant, and every service of a
    /// connector rather than one per connector (C-197).
    values: BTreeMap<(String, String, String, &'static str, String), ConfigValue>,
}

/// Keep both configuration values and their customer-owned addresses out of debug output, matching
/// [`Configuration`]'s policy for its erased store.
impl std::fmt::Debug for MemoryConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryConfig").finish_non_exhaustive()
    }
}

impl MemoryConfig {
    /// An empty store. A pack bound to one refuses every templated connector by name, which is the
    /// correct starting state rather than an unhelpful one.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind a `{var}` of `provider`'s `service` base URL for `tenant`.
    ///
    /// `service` is the operation's own — `default` for a connector with a single surface, and one
    /// of the declared names otherwise. It is a required argument rather than an optional one with
    /// a `default` fallback precisely because the fallback would be silent: a host binding
    /// `contentful`'s `space_id` without saying which of its two services meant it would find every
    /// contentful operation refusing, which is the better failure but only if it is the one that
    /// happens.
    #[must_use]
    pub fn with_endpoint(
        self,
        tenant: &str,
        provider: &str,
        service: &str,
        variable: &str,
        value: &str,
    ) -> Self {
        self.with(
            tenant,
            provider,
            service,
            Field::Endpoint(variable),
            value,
            false,
        )
    }

    /// Bind an endpoint value approved and pinned by deployment/operator policy.
    #[must_use]
    pub fn with_approved_endpoint(
        self,
        tenant: &str,
        provider: &str,
        service: &str,
        variable: &str,
        value: &str,
    ) -> Self {
        self.with(
            tenant,
            provider,
            service,
            Field::Endpoint(variable),
            value,
            true,
        )
    }

    /// Retain an endpoint proposal while revoking its deployment/operator approval.
    ///
    /// This produces the same inert state as a newly proposed value. Naming the transition keeps
    /// hosts and tests from modelling revocation as deletion: the proposal may remain visible to an
    /// operator UI while connector execution, permission subjects and evidence all refuse to use
    /// it.
    #[must_use]
    pub fn with_revoked_endpoint(
        self,
        tenant: &str,
        provider: &str,
        service: &str,
        variable: &str,
        value: &str,
    ) -> Self {
        self.with(
            tenant,
            provider,
            service,
            Field::Endpoint(variable),
            value,
            false,
        )
    }

    /// Bind the non-secret user half of `provider`'s `credential`, for `tenant`, under `service`.
    ///
    /// A credential is declared at *connector* level, so its address elides the service — but the
    /// **configuration field that supplies its user half** is declared under exactly one service
    /// like every other (`connector_spec::ConfigField::service`), and this port follows the IR
    /// rather than the address. A connector with two services that both authenticate with one
    /// `basic` credential therefore binds the user half under each; the alternative is one service
    /// silently reading the other's, which is the whole of C-197.
    #[must_use]
    pub fn with_username(
        self,
        tenant: &str,
        provider: &str,
        service: &str,
        credential: &str,
        value: &str,
    ) -> Self {
        self.with(
            tenant,
            provider,
            service,
            Field::Username(credential),
            value,
            false,
        )
    }

    /// Bind one query parameter used only while opening a declared socket channel.
    #[must_use]
    pub fn with_channel_query(
        self,
        tenant: &str,
        provider: &str,
        service: &str,
        channel: &str,
        parameter: &str,
        value: &str,
    ) -> Self {
        self.with(
            tenant,
            provider,
            service,
            Field::ChannelQuery { channel, parameter },
            value,
            false,
        )
    }

    fn with(
        mut self,
        tenant: &str,
        provider: &str,
        service: &str,
        field: Field<'_>,
        value: &str,
        approved: bool,
    ) -> Self {
        self.values.insert(
            (
                tenant.to_owned(),
                provider.to_owned(),
                service.to_owned(),
                field.kind(),
                field.name().to_owned(),
            ),
            if approved {
                ConfigValue::operator_approved(value.to_owned())
            } else {
                ConfigValue::proposed(value.to_owned())
            },
        );
        self
    }
}

impl ConfigStore for MemoryConfig {
    fn get(&self, tenant: &str, provider: &str, service: &str, field: Field<'_>) -> Option<String> {
        self.values
            .get(&(
                tenant.to_owned(),
                provider.to_owned(),
                service.to_owned(),
                field.kind(),
                field.name().to_owned(),
            ))
            .map(|resolved| resolved.value().to_owned())
    }

    fn resolve_for_instance(
        &self,
        tenant: &str,
        provider: &str,
        instance: Option<&InstanceId>,
        service: &str,
        field: Field<'_>,
    ) -> Option<ConfigValue> {
        if instance.is_some() {
            return None;
        }
        self.values
            .get(&(
                tenant.to_owned(),
                provider.to_owned(),
                service.to_owned(),
                field.kind(),
                field.name().to_owned(),
            ))
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_exposes_configuration_values() {
        const SENTINEL: &str = "https://configured-origin.debug-leak.invalid";

        let proposed = ConfigValue::proposed(SENTINEL.to_owned());
        let approved = ConfigValue::operator_approved(SENTINEL.to_owned());
        let store = MemoryConfig::new().with_approved_endpoint(
            "tenant-debug-sentinel",
            "gitlab",
            "default",
            "origin",
            SENTINEL,
        );

        for (surface, rendered) in [
            ("proposed ConfigValue", format!("{proposed:?}")),
            ("approved ConfigValue", format!("{approved:?}")),
            ("MemoryConfig", format!("{store:?}")),
        ] {
            assert!(
                !rendered.contains(SENTINEL),
                "{surface} debug output exposed the configured-origin sentinel: {rendered}"
            );
        }
        assert_eq!(
            format!("{proposed:?}"),
            "ConfigValue { operator_approved: false, .. }"
        );
        assert_eq!(
            format!("{approved:?}"),
            "ConfigValue { operator_approved: true, .. }"
        );
        assert_eq!(format!("{store:?}"), "MemoryConfig { .. }");
    }

    #[test]
    fn a_named_instance_uses_the_instance_aware_config_port() {
        #[derive(Default)]
        struct Instances;

        impl ConfigStore for Instances {
            fn get(
                &self,
                _tenant: &str,
                _provider: &str,
                _service: &str,
                _field: Field<'_>,
            ) -> Option<String> {
                Some("unscoped".to_owned())
            }

            fn get_for_instance(
                &self,
                _tenant: &str,
                _provider: &str,
                instance: Option<&InstanceId>,
                _service: &str,
                _field: Field<'_>,
            ) -> Option<String> {
                instance.map(|instance| format!("instance-{}", instance.as_str()))
            }
        }

        let instance = "0d3f79ae-b6df-4f77-8f77-438436c3b2ef";
        let configuration =
            Configuration::for_instance(Arc::new(Instances), "t", instance).expect("valid binding");
        let snapshot = configuration.snapshot("acme", "default", [Field::Endpoint("account")]);
        assert_eq!(
            snapshot.lookup(Field::Endpoint("account")).as_deref(),
            Some(format!("instance-{instance}").as_str())
        );
    }

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
            .with_endpoint("t", "acme", "default", "account", "acme-endpoint")
            .with_username("t", "acme", "default", "account", "acme-username");

        assert_eq!(
            store.get("t", "acme", "default", Field::Endpoint("account")),
            Some("acme-endpoint".to_string())
        );
        assert_eq!(
            store.get("t", "acme", "default", Field::Username("account")),
            Some("acme-username".to_string())
        );
    }

    /// **The unit-level statement of C-197**, on the store itself: one connector, one variable name,
    /// two services, two values. `contentful`'s `endpoint.space_id` is the shipped case, and
    /// `tests/service_scoped_configuration.rs` asserts it against the shipped catalogue rather than
    /// against this fixture.
    #[test]
    fn one_variable_name_in_two_services_is_two_values() {
        let store = MemoryConfig::new()
            .with_endpoint("t", "contentful", "delivery", "space_id", "delivery-space")
            .with_endpoint(
                "t",
                "contentful",
                "management",
                "space_id",
                "management-space",
            );

        assert_eq!(
            store.get("t", "contentful", "delivery", Field::Endpoint("space_id")),
            Some("delivery-space".to_string())
        );
        assert_eq!(
            store.get("t", "contentful", "management", Field::Endpoint("space_id")),
            Some("management-space".to_string())
        );
        assert_eq!(
            store.get("t", "contentful", "preview", Field::Endpoint("space_id")),
            None,
            "a service nobody bound a value for borrows no sibling's"
        );
    }

    /// One store, many tenants — the shape a host binding a snapshot per tenant needs, and the one
    /// that makes a cross-tenant mix-up visible rather than structural.
    #[test]
    fn a_value_belongs_to_one_tenant() {
        let store =
            MemoryConfig::new().with_endpoint("t-one", "zendesk", "default", "subdomain", "one");

        assert_eq!(
            store.get("t-one", "zendesk", "default", Field::Endpoint("subdomain")),
            Some("one".to_string())
        );
        assert_eq!(
            store.get("t-two", "zendesk", "default", Field::Endpoint("subdomain")),
            None
        );
    }

    /// An empty string is not a value. Left alone it would substitute into
    /// `https://.zendesk.com` — a host that does not resolve, arrived at without an error.
    #[test]
    fn an_empty_value_is_missing_rather_than_bound() {
        let configuration = Configuration::new(
            Arc::new(MemoryConfig::new().with_endpoint("t", "zendesk", "default", "subdomain", "")),
            "t",
        )
        .expect("a valid tenant id");
        let settings = configuration.snapshot("zendesk", "default", [Field::Endpoint("subdomain")]);

        assert!(settings.lookup(Field::Endpoint("subdomain")).is_none());
        let error = settings
            .require("zendesk-ticket-show", Field::Endpoint("subdomain"))
            .expect_err("an empty subdomain is not a subdomain");
        assert!(error.to_string().contains("endpoint.subdomain"), "{error}");
    }

    /// The refusal names the **service** as well as the connector (C-197). Without it an operator
    /// told that `contentful` is missing `endpoint.space_id` has two fields that answer to that
    /// description and no way to tell which one to supply.
    #[test]
    fn a_refusal_names_the_service_it_was_looked_up_under() {
        let configuration =
            Configuration::new(Arc::new(MemoryConfig::new()), "t").expect("a valid tenant id");
        let settings =
            configuration.snapshot("contentful", "management", [Field::Endpoint("space_id")]);

        let error = settings
            .require("contentful-entry-create", Field::Endpoint("space_id"))
            .expect_err("nothing is bound");
        assert!(
            matches!(&error, Error::MissingConfig { service, field, .. }
                if service == "management" && field == "endpoint.space_id"),
            "{error}"
        );
        assert!(error.to_string().contains("management"), "{error}");
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
            fn get(
                &self,
                _tenant: &str,
                _provider: &str,
                _service: &str,
                _field: Field<'_>,
            ) -> Option<String> {
                Some(
                    self.0
                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                        .to_string(),
                )
            }
        }

        let store = Arc::new(Counting::default());
        let configuration = Configuration::new(store.clone(), "t").expect("a valid tenant id");
        let settings = configuration.snapshot("zendesk", "default", [Field::Endpoint("subdomain")]);

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
                    .with_endpoint("t", "zendesk", "default", "subdomain", "acme")
                    .with_username(
                        "t",
                        "zendesk",
                        "default",
                        "zendesk.api_token",
                        "ops@acme.test",
                    ),
            ),
            "t",
        )
        .expect("a valid tenant id");
        let settings = configuration.snapshot("zendesk", "default", [Field::Endpoint("subdomain")]);

        assert_eq!(
            settings.lookup(Field::Endpoint("subdomain")).as_deref(),
            Some("acme")
        );
        assert!(settings
            .lookup(Field::Username("zendesk.api_token"))
            .is_none());
    }
}
