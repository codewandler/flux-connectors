//! **Two services of one connector are two configurations (C-197).**
//!
//! The configuration port used to key a tenant's values on `(tenant, provider, kind, name)` — no
//! service — because `catalog::Operation` carried no service for it to key on. So a connector whose
//! two services spell the same `{variable}` in their own `base_url` had exactly one slot for it.
//!
//! `contentful` is that connector, in the shipped catalogue rather than in a fixture:
//!
//! | field | `binds` | service | host |
//! |---|---|---|---|
//! | `delivery_space_id` | `endpoint.space_id` | `delivery` | `cdn.contentful.com` |
//! | `management_space_id` | `endpoint.space_id` | `management` | `api.contentful.com` |
//!
//! `providers/contentful.toml` states in its own prose why the two must be distinct — a product is
//! free to point delivery and management at different spaces — and the runtime port could not hold
//! the distinction. The consequence is the reason this file asserts against the real catalogue: a
//! tenant whose two environments differ got a **`200` from the wrong space with a real management
//! token**, a write into a space nobody named. Not a refusal, and nothing to see in a log.
//!
//! Every test here is written against `catalog::operation(…)`, so it cannot go green while
//! contentful stays broken and it cannot be satisfied by a fixture the fix happens to suit.

use std::sync::Arc;

use catalog::OperationKey;
use connector_pack::{
    ConfigStore, Configuration, Credentials, Egress, Error, Field, MemoryConfig, MemoryStore,
    Operation,
};
use serde_json::json;

/// The tenant both ports answer for.
const TENANT: &str = "t-service-scoped";

/// The two shipped contentful operations this file is about: one per service, on two hosts.
const DELIVERY_READ: &str = "contentful-entry-get";
const MANAGEMENT_WRITE: &str = "contentful-entry-create";

/// This tenant's delivery space, and the one a read must reach.
const DELIVERY_SPACE: &str = "space-for-delivery";
/// This tenant's management space, and the one a write must reach — a *different* space, which is
/// the whole point.
const MANAGEMENT_SPACE: &str = "space-for-management";

fn http() -> Egress {
    Egress::new(flux_runtime::tool_fn(
        flux_spec::ToolSpec {
            name: "http.request".into(),
            description: "a stand-in".into(),
            input_schema: json!({"type": "object"}),
            output_schema: None,
            effects: vec![flux_spec::Effect::Network],
            risk: flux_spec::Risk::Medium,
            idempotency: flux_spec::Idempotency::NonIdempotent,
            access: vec![flux_spec::AccessKind::Network],
            group: None,
        },
        |params| async move { Ok(params) },
    ))
}

fn credentials() -> Credentials {
    Credentials::new(Arc::new(MemoryStore::new()), TENANT).expect("a valid tenant id")
}

/// A port holding **different** values for contentful's two services — the arrangement the defect
/// made unrepresentable.
fn two_spaces() -> MemoryConfig {
    MemoryConfig::new()
        .with_endpoint(TENANT, "contentful", "delivery", "space_id", DELIVERY_SPACE)
        .with_endpoint(TENANT, "contentful", "delivery", "environment_id", "master")
        .with_endpoint(
            TENANT,
            "contentful",
            "management",
            "space_id",
            MANAGEMENT_SPACE,
        )
        .with_endpoint(
            TENANT,
            "contentful",
            "management",
            "environment_id",
            "master",
        )
}

fn projected(id: &str, values: MemoryConfig) -> Operation {
    let entry = catalog::operation(OperationKey::id(id))
        .unwrap_or_else(|| panic!("the shipped catalogue carries `{id}`"));
    let configuration = Configuration::new(Arc::new(values), TENANT).expect("a valid tenant id");
    Operation::project(entry, http(), credentials(), configuration)
        .unwrap_or_else(|error| panic!("`{id}`: {error}"))
}

/// **The catalogue carries the service.** Everything below keys on it, and without this field a
/// consumer has no way to tell contentful's two `space_id`s apart — which is exactly the state
/// C-193 found and could not fix from where it stood.
#[test]
fn the_shipped_catalogue_names_each_operations_service() {
    let read = catalog::operation(OperationKey::id(DELIVERY_READ)).expect("it ships");
    let write = catalog::operation(OperationKey::id(MANAGEMENT_WRITE)).expect("it ships");

    assert_eq!(read.service, "delivery");
    assert_eq!(write.service, "management");
    assert_eq!(read.hosts, ["cdn.contentful.com"]);
    assert_eq!(write.hosts, ["api.contentful.com"]);

    // Not one connector with a service field nobody uses: every shipped operation names one, and
    // the reserved `default` is written out rather than elided, so a consumer may group by service
    // unconditionally.
    for entry in catalog::operations() {
        assert!(
            !entry.service.is_empty(),
            "`{}` names no service, so nothing can be keyed by it",
            entry.id
        );
    }
}

/// **The story, in one assertion.** Two operations, one connector, two services, two values — and
/// the values reach two different hosts, so getting this wrong is not a cosmetic mix-up.
#[test]
fn two_services_of_one_connector_resolve_different_values() {
    let read = projected(DELIVERY_READ, two_spaces())
        .build_request(&json!({ "entry_id": "e-1" }))
        .expect("the delivery read builds");
    let write = projected(MANAGEMENT_WRITE, two_spaces())
        .build_request(&json!({ "content_type_id": "post", "body": { "fields": {} } }))
        .expect("the management write builds");

    assert!(
        read.url.contains(&format!("/spaces/{DELIVERY_SPACE}/")),
        "the delivery read must reach the delivery space: {}",
        read.url
    );
    assert!(
        write.url.contains(&format!("/spaces/{MANAGEMENT_SPACE}/")),
        "the management write must reach the management space: {}",
        write.url
    );
    assert!(
        !write.url.contains(DELIVERY_SPACE),
        "the management write reached the delivery space — a write into a space nobody named: {}",
        write.url
    );
    assert!(
        !read.url.contains(MANAGEMENT_SPACE),
        "the delivery read reached the management space: {}",
        read.url
    );
}

/// **A value bound for one service is not borrowed by another.** The failure mode that made the
/// defect silent was not "two values disagree" but "one value answers for both", so the negative is
/// the sharper statement: configure `delivery` alone and the management write must *refuse*, by
/// name, rather than quietly inherit.
#[test]
fn a_service_with_nothing_bound_refuses_rather_than_reading_its_siblings_value() {
    // Everything both services need **except** management's `space_id` — the one variable the
    // collapsed key let one service borrow from the other. Isolating it is what makes the refusal
    // below name `endpoint.space_id` rather than whichever variable happens to be missing first.
    let space_bound_for_delivery_only = MemoryConfig::new()
        .with_endpoint(TENANT, "contentful", "delivery", "space_id", DELIVERY_SPACE)
        .with_endpoint(TENANT, "contentful", "delivery", "environment_id", "master")
        .with_endpoint(
            TENANT,
            "contentful",
            "management",
            "environment_id",
            "master",
        );

    // The half that must still work: delivery is configured, so the read composes.
    let read = projected(DELIVERY_READ, space_bound_for_delivery_only.clone())
        .build_request(&json!({ "entry_id": "e-1" }))
        .expect("the delivery read builds");
    assert!(read.url.contains(DELIVERY_SPACE), "{}", read.url);

    // The half the defect got wrong: management's own `space_id` is not bound, so the write is
    // refused instead of quietly reaching `DELIVERY_SPACE`.
    let error = projected(MANAGEMENT_WRITE, space_bound_for_delivery_only)
        .build_request(&json!({ "content_type_id": "post", "body": { "fields": {} } }))
        .expect_err("management's space_id is not bound, so no URL composes");

    assert!(
        matches!(&error, Error::MissingConfig { service, field, provider, .. }
            if service == "management" && field == "endpoint.space_id" && provider == "contentful"),
        "{error}"
    );
    let rendered = error.to_string();
    assert!(
        rendered.contains("management"),
        "the refusal must name the service, or an operator has two `endpoint.space_id`s and no way \
         to tell which to supply: {rendered}"
    );
    assert!(rendered.contains("was not sent"), "{rendered}");
}

/// **The service reaches the store**, rather than being resolved and discarded on the way. A store
/// that answers by service is the shape a real host has — one row per `(tenant, service, field)` —
/// and this records the exact arguments the port is asked with.
#[test]
fn the_store_is_asked_for_the_operations_own_service() {
    /// Answers `<service>-space` for `endpoint.space_id`, so the value a request carries names the
    /// service it was fetched under.
    struct ByService;

    impl ConfigStore for ByService {
        fn get(
            &self,
            _tenant: &str,
            provider: &str,
            service: &str,
            field: Field<'_>,
        ) -> Option<String> {
            assert_eq!(provider, "contentful");
            match field {
                Field::Endpoint("space_id") => Some(format!("{service}-space")),
                Field::Endpoint("environment_id") => Some("master".to_string()),
                _ => None,
            }
        }
    }

    let configuration = Configuration::new(Arc::new(ByService), TENANT).expect("a valid tenant id");
    let entry = catalog::operation(OperationKey::id(MANAGEMENT_WRITE)).expect("it ships");
    let request = Operation::project(entry, http(), credentials(), configuration)
        .expect("it projects")
        .build_request(&json!({ "content_type_id": "post", "body": { "fields": {} } }))
        .expect("the request builds");

    assert!(
        request.url.contains("/spaces/management-space/"),
        "the store was asked under the wrong service: {}",
        request.url
    );
}

/// **The provider is exercised, not merely supported.** If contentful ever stops declaring two
/// services that bind one variable name, every test above would still pass while asserting nothing
/// about the case they exist for. This is the control, and it reads the shipped catalogue.
#[test]
fn contentful_still_binds_one_variable_name_in_two_services() {
    let services: Vec<&str> = catalog::operations_of(catalog::ProviderKey::id("contentful"))
        .iter()
        .map(|entry| entry.service)
        .collect();

    assert!(
        services.contains(&"delivery") && services.contains(&"management"),
        "contentful no longer ships two services, so this file names the wrong case: {services:?}"
    );

    for id in [DELIVERY_READ, MANAGEMENT_WRITE] {
        let operation = projected(id, two_spaces());
        assert!(
            operation
                .endpoint_variables()
                .iter()
                .any(|variable| variable == "space_id"),
            "`{id}` no longer carries `{{space_id}}`, so this file names the wrong case"
        );
    }
}
