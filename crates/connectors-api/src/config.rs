//! The host's side of the configuration port.
//!
//! `connector-pack` asks for two kinds of value through [`ConfigStore`]: the `{placeholder}` in a
//! service's `base_url`, and the non-secret user half of a `basic` credential. Nine of the 44
//! shipped connectors carry a templated base URL, covering 53 of 248 operations, and without a value
//! for each one the pack refuses rather than sending a request with a brace in it.

use std::collections::BTreeMap;
use std::sync::RwLock;

use connector_pack::{ConfigStore, Field};

/// A tenant's connection settings, in memory.
///
/// # Why the key is a five-tuple
///
/// `(tenant, provider, service, kind, name)`, and the **service** is the segment that is easy to
/// drop and expensive to have dropped. `contentful` declares `delivery_space_id` and
/// `management_space_id`, both binding `endpoint.space_id`, under two services that reach two
/// different hosts. Keyed without the service they are one slot, and a management write lands in
/// whichever space the delivery reads were configured with — a `200` from a real server, which is
/// why nothing refuses. That is C-197, and it is a defect this type is shaped to be incapable of.
#[derive(Debug, Default)]
pub struct Settings {
    values: RwLock<BTreeMap<Key, String>>,
}

type Key = (String, String, String, &'static str, String);

impl Settings {
    /// An empty set of settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind `value` to one field of one service of one connector, for one tenant.
    pub fn set(
        &self,
        tenant: &str,
        provider: &str,
        service: &str,
        field: Field<'_>,
        value: impl Into<String>,
    ) {
        let (kind, name) = decompose(field);
        self.values.write().expect("not poisoned").insert(
            (
                tenant.to_owned(),
                provider.to_owned(),
                service.to_owned(),
                kind,
                name,
            ),
            value.into(),
        );
    }

    /// Every field this tenant has bound for `provider`, as `("endpoint.subdomain", "acme")` pairs.
    ///
    /// Used to render "what is still missing" in the UI. These are connection settings, not secrets
    /// — a subdomain is a customer's, not a credential — so returning the value is correct here in a
    /// way it never is for [`crate::state::App::secrets`].
    pub fn bound_for(&self, tenant: &str, provider: &str) -> Vec<(String, String)> {
        self.values
            .read()
            .expect("not poisoned")
            .iter()
            .filter(|((t, p, _, _, _), _)| t == tenant && p == provider)
            .map(|((_, _, service, kind, name), value)| {
                (format!("{service}/{kind}.{name}"), value.clone())
            })
            .collect()
    }
}

/// The field, split into the two parts a key needs.
///
/// Written out rather than delegated because `Field`'s own `kind`/`name` are private to the pack —
/// and because matching every variant here is the point. `Field` is deliberately not
/// `#[non_exhaustive]`: a new kind of configuration value must be a compile error at this site
/// rather than a `None` that reads as "the tenant has not configured it".
fn decompose(field: Field<'_>) -> (&'static str, String) {
    match field {
        Field::Endpoint(name) => ("endpoint", name.to_owned()),
        Field::Username(name) => ("username", name.to_owned()),
        Field::ChannelQuery { channel, parameter } => {
            ("channel_query", format!("{channel}\0{parameter}"))
        }
    }
}

impl ConfigStore for Settings {
    /// # Stability
    ///
    /// The port requires that a bound store answer the same `(tenant, provider, service, field)`
    /// with the same value for as long as it is bound, because the pack consults it once for the
    /// permission gate and once for the request, and a store that drifts between them sends traffic
    /// through an allow-list to a host that was never checked.
    ///
    /// This one *can* be written to while bound, and it is safe here for a specific reason: the host
    /// builds a fresh pack per request, and `Configuration::snapshot` reads every value an operation
    /// can ask for when the operation is projected. So a write between two requests is a new
    /// binding, and there is no second read within one request for it to race.
    fn get(&self, tenant: &str, provider: &str, service: &str, field: Field<'_>) -> Option<String> {
        let (kind, name) = decompose(field);
        self.values
            .read()
            .expect("not poisoned")
            .get(&(
                tenant.to_owned(),
                provider.to_owned(),
                service.to_owned(),
                kind,
                name,
            ))
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The C-197 defect, as a test: two services of one connector binding the same variable name
    /// must be two values.
    #[test]
    fn two_services_of_one_connector_keep_their_own_value() {
        let settings = Settings::new();
        settings.set(
            "t1",
            "contentful",
            "delivery",
            Field::Endpoint("space_id"),
            "cdn-space",
        );
        settings.set(
            "t1",
            "contentful",
            "management",
            Field::Endpoint("space_id"),
            "api-space",
        );

        assert_eq!(
            settings.get("t1", "contentful", "delivery", Field::Endpoint("space_id")),
            Some("cdn-space".to_owned())
        );
        assert_eq!(
            settings.get(
                "t1",
                "contentful",
                "management",
                Field::Endpoint("space_id")
            ),
            Some("api-space".to_owned())
        );
    }

    /// Two tenants are two sets of settings. The same assertion the credential store owes, on the
    /// half that is not secret.
    #[test]
    fn two_tenants_do_not_share_a_value() {
        let settings = Settings::new();
        settings.set(
            "a",
            "zendesk",
            "default",
            Field::Endpoint("subdomain"),
            "acme",
        );

        assert_eq!(
            settings.get("b", "zendesk", "default", Field::Endpoint("subdomain")),
            None,
            "tenant b read tenant a's setting"
        );
    }

    /// An endpoint variable and a Basic user half of the same name are different fields.
    #[test]
    fn the_kind_is_part_of_the_key() {
        let settings = Settings::new();
        settings.set(
            "t1",
            "jira",
            "default",
            Field::Endpoint("domain"),
            "acme.atlassian.net",
        );

        assert_eq!(
            settings.get("t1", "jira", "default", Field::Username("domain")),
            None
        );
    }
}
