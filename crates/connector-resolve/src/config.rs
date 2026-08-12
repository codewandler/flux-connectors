//! **The configuration port**: how a host supplies a tenant's non-secret connection values to the
//! engine-free endpoint resolver (C-557).
//!
//! It is the engine-free twin of `connector-pack`'s `ConfigStore`/`Snapshot`, and it exists for the
//! same reason the credential port does: a consumer deriving a request plan without `connector-pack`
//! and without a flux `ToolContext` still has to answer *what is this tenant's `{subdomain}`* before
//! a URL composes. The port answers exactly that, and nothing about where the value is kept —
//! [`resolve_endpoint`](crate::resolve_endpoint) applies the declared defaults, the operator-approval
//! gate and the origin normalisation on top of it, so the same enforcement runs whoever supplies the
//! raw value.
//!
//! # It carries the operator-approval bit, and that is load-bearing
//!
//! A [`ConfigValue`] is a value **and** whether deployment/operator policy approved this exact
//! resolution. An operator-gated field bound to anything other than its reviewed default is refused
//! unless it was approved, so the port cannot flatten the two into a bare string without losing the
//! gate. The constructors are named so a host chooses [`operator_approved`](ConfigValue::operator_approved)
//! at the point where policy was actually evaluated, exactly as `connector-pack`'s `ConfigValue`
//! does — this is that type, moved to the engine-free side.

/// **Which non-secret connection value the resolver is asking for.**
///
/// The three variants are the non-secret, connection-level rows of the configuration design's
/// `binds` table — `endpoint.<var>`, `username.<name>` and `channel.<binding>.query.<parameter>`.
/// Secrets are the credential port's job and never travel through here.
///
/// Not `#[non_exhaustive]`: a host implementing [`ConfigPort`] must decide what to do with every kind
/// the resolver can ask for, and a new kind should be a compile error at that decision rather than a
/// `None` that reads as "not configured".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConfigField<'a> {
    /// A `{var}` in a service's `base_url` — `subdomain`, `shop`, `account_host`. The name is the
    /// placeholder as the connector's own document spells it, with no braces.
    Endpoint(&'a str),
    /// The **non-secret** user half of a `basic` credential, named by the credential it joins —
    /// `zendesk.api_token`, `jira.api_token`. The connector's declared suffix is appended by the
    /// assembler, not asked of the host.
    Username(&'a str),
    /// One connection-time query value of a socket channel's `connect` handshake (C-558).
    ///
    /// Keyed by the channel binding **and** the vendor query parameter, exactly as the credential
    /// addressing keys the operation-path fields: two bindings of one service may spell the same
    /// parameter, so the binding is part of the address rather than a hint.
    ChannelQuery {
        /// The channel binding this query value belongs to.
        channel: &'a str,
        /// The vendor query parameter, as the `connect.query` declaration spells it.
        parameter: &'a str,
    },
}

impl<'a> ConfigField<'a> {
    /// Interpret one endpoint variable as the field kind it addresses.
    ///
    /// `username.` is the one reserved qualifier — it lets a Basic username also scope a request
    /// without being looked up through the unrelated endpoint address. Anything else is an endpoint
    /// variable. This is the same partition `connector-pack`'s `Field::from_placeholder` draws, which
    /// is what keeps the two ports keyed the same way.
    pub fn from_placeholder(variable: &'a str) -> ConfigField<'a> {
        variable
            .strip_prefix("username.")
            .filter(|name| !name.is_empty())
            .map(ConfigField::Username)
            .unwrap_or(ConfigField::Endpoint(variable))
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

/// A setting may be safe to store without being safe to copy into a log. Preserve the policy bit that
/// helps diagnose a refusal while keeping the customer-provided value out of debug output.
impl std::fmt::Debug for ConfigValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigValue")
            .field("operator_approved", &self.operator_approved)
            .finish_non_exhaustive()
    }
}

impl ConfigValue {
    /// A value supplied by a connection owner but not activated by operator policy.
    pub fn proposed(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            operator_approved: false,
        }
    }

    /// A value approved and pinned by deployment/operator policy for this connection.
    pub fn operator_approved(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
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

/// **A host's non-secret connection settings**, as the endpoint resolver asks for them.
///
/// `None` means "this tenant has not configured it", and [`resolve_endpoint`](crate::resolve_endpoint)
/// turns that into a declared default when one exists, or a refusal naming the field otherwise —
/// never a request to a host with a brace in it.
///
/// # Stability is the caller's contract
///
/// The resolver may ask for the same field more than once across an operation; an implementation must
/// answer the same field the same way for as long as it is bound. A store that reads a database on
/// every call, or a cache that can expire, must resolve its values eagerly and hand over a fixed set
/// instead — the same requirement `connector-pack`'s `ConfigStore` documents, and the reason its
/// `Snapshot` exists.
pub trait ConfigPort: Send + Sync {
    /// The value bound to `field`, with its operator-approval state, or `None` when the tenant has
    /// supplied none.
    fn resolve(&self, field: ConfigField<'_>) -> Option<ConfigValue>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_username_prefix_is_the_one_reserved_qualifier() {
        assert_eq!(
            ConfigField::from_placeholder("subdomain"),
            ConfigField::Endpoint("subdomain")
        );
        assert_eq!(
            ConfigField::from_placeholder("username.zendesk.api_token"),
            ConfigField::Username("zendesk.api_token")
        );
        // An empty username is not a username; it stays an endpoint variable spelled `username.`.
        assert_eq!(
            ConfigField::from_placeholder("username."),
            ConfigField::Endpoint("username.")
        );
    }

    #[test]
    fn a_config_value_debug_carries_no_value() {
        let value = ConfigValue::operator_approved("acme.example.com");
        let printed = format!("{value:?}");
        assert!(!printed.contains("acme.example.com"), "{printed}");
        assert!(printed.contains("operator_approved: true"), "{printed}");
        assert!(value.is_operator_approved());
        assert_eq!(value.value(), "acme.example.com");
        assert!(!ConfigValue::proposed("x").is_operator_approved());
    }
}
