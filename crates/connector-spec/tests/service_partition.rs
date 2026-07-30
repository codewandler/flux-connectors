//! The two properties C-49 rests on: **services partition the operation set**, and **an address
//! round-trips through the `default` elision**.
//!
//! Both are properties rather than examples, so both are tested over a generated corpus. There is no
//! `proptest` here on purpose — the crate takes no new dependency for this — so the generator is a
//! deterministic LCG. Deterministic is what a property test in this repository wants anyway: a
//! failure is reproducible from the seed printed in the assertion, and CI cannot go red on a shape
//! nobody can rebuild.

use connector_spec::{
    Connector, Gid, HttpMethod, Idempotency, Oip, Operation, ParamSet, Pid, Provenance, Quirks,
    Risk, Service, DEFAULT_SERVICE,
};

/// A tiny deterministic generator. Numerical Recipes' LCG constants; the values only have to be
/// spread out, not statistically strong.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        self.0 >> 33
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() as usize) % bound
    }
}

fn operation(id: &str, service: &str) -> Operation {
    Operation {
        id: id.to_owned(),
        service: service.to_owned(),
        method: HttpMethod::Get,
        path: "/v1/things".to_owned(),
        description: String::new(),
        risk: Risk::Low,
        idempotency: Idempotency::Idempotent,
        auth: None,
        params: ParamSet::default(),
        response_schema: None,
        quirks: Quirks::default(),
    }
}

fn connector(services: Vec<Service>, operations: Vec<Operation>) -> Connector {
    Connector {
        id: "acme".to_owned(),
        authority: Some("com.acme".to_owned()),
        api_version: Some("v1".to_owned()),
        services,
        vendor: "Acme".to_owned(),
        base_url: "https://api.acme.example".to_owned(),
        description: String::new(),
        auth: Vec::new(),
        default_auth: Vec::new(),
        operations,
        provenance: Provenance::default(),
    }
}

fn service(name: &str) -> Service {
    Service {
        name: name.to_owned(),
        ..Service::default()
    }
}

/// **The invariant that makes "install the whole s3 service" a well-defined set.** For every shape
/// the generator produces: the per-service operation sets are pairwise disjoint, and their union is
/// every operation the connector declares.
#[test]
fn services_partition_the_operation_set() {
    for seed in 0..200u64 {
        let mut rng = Rng(seed.wrapping_mul(2_654_435_761).wrapping_add(1));

        // Half the shapes are single-service, which is the shape the repository actually ships, and
        // the shape where the partition is the whole set falling into `default`.
        let service_count = if seed % 2 == 0 { 0 } else { 1 + rng.below(4) };
        let names: Vec<String> = (0..service_count).map(|i| format!("svc-{i}")).collect();
        let services: Vec<Service> = names.iter().map(|name| service(name)).collect();

        let operation_count = rng.below(12);
        let operations: Vec<Operation> = (0..operation_count)
            .map(|i| {
                let service = if names.is_empty() {
                    DEFAULT_SERVICE.to_owned()
                } else {
                    names[rng.below(names.len())].clone()
                };
                operation(&format!("acme-op-{i}"), &service)
            })
            .collect();

        let connector = connector(services, operations);
        let available = connector.service_names();

        let mut union: Vec<&str> = Vec::new();
        for name in &available {
            for operation in connector.operations_of(name) {
                assert!(
                    !union.contains(&operation.id.as_str()),
                    "seed {seed}: operation `{}` appeared in two services, so the sets are not \
                     disjoint",
                    operation.id
                );
                union.push(&operation.id);
            }
        }

        assert_eq!(
            union.len(),
            connector.operations.len(),
            "seed {seed}: the union of the per-service sets is not every operation ({} of {})",
            union.len(),
            connector.operations.len()
        );
        for operation in &connector.operations {
            assert!(
                union.contains(&operation.id.as_str()),
                "seed {seed}: operation `{}` belongs to no service",
                operation.id
            );
        }
    }
}

/// A connector that declares no service still has exactly one — the reserved `default` — and every
/// operation is in it. The degenerate case of the partition, and the one all six shipped providers
/// are in.
#[test]
fn a_connector_without_services_has_exactly_the_default_one() {
    let connector = connector(
        Vec::new(),
        vec![
            operation("acme-a", DEFAULT_SERVICE),
            operation("acme-b", DEFAULT_SERVICE),
        ],
    );
    assert_eq!(connector.service_names(), vec![DEFAULT_SERVICE]);
    assert!(connector.is_default_only());
    assert_eq!(connector.operations_of(DEFAULT_SERVICE).count(), 2);
}

/// `parse(render(x)) == x` for every level, **including the elision**: a `default` gid renders with no
/// middle segment and parses back to `default`.
#[test]
fn addresses_round_trip_through_the_default_elision() {
    let mut rng = Rng(0x5eed);
    let authorities = ["com.amazonaws", "com.zendesk.api", "io.example.sub.domain"];
    let services = [
        DEFAULT_SERVICE,
        "s3",
        "bedrock-runtime",
        "support",
        "v2-beta",
    ];
    let versions = ["v1", "v2", "2006-03-01", "2023-09-30"];
    let operations = ["object-get", "show", "comment-add", "model-invoke"];

    for _ in 0..500 {
        let authority = authorities[rng.below(authorities.len())];
        let service = services[rng.below(services.len())];
        let version = versions[rng.below(versions.len())];
        let operation = operations[rng.below(operations.len())];

        let pid = Pid::new(authority);
        assert_eq!(Pid::parse(&pid.to_string()).unwrap(), pid);

        let gid = Gid::new(authority, service, version);
        let rendered = gid.to_string();
        assert!(
            !rendered.contains(&format!("/{DEFAULT_SERVICE}")),
            "`{DEFAULT_SERVICE}` must never reach a rendered address: {rendered}"
        );
        assert_eq!(Gid::parse(&rendered).unwrap(), gid, "gid `{rendered}`");
        assert_eq!(gid.pid(), pid);

        let oip = Oip::new(gid, operation);
        let rendered = oip.to_string();
        assert_eq!(Oip::parse(&rendered).unwrap(), oip, "oip `{rendered}`");
    }
}

/// The three shapes the story names, spelled out — a round-trip property proves consistency, not that
/// the rendering is the agreed one.
#[test]
fn the_rendered_forms_are_the_ones_the_design_publishes() {
    assert_eq!(
        Gid::new("com.amazonaws", "s3", "2006-03-01").to_string(),
        "com.amazonaws/s3:2006-03-01"
    );
    assert_eq!(
        Oip::new(Gid::new("com.zendesk.api", "support", "v2"), "show").to_string(),
        "com.zendesk.api/support:v2#show"
    );
    // The elision: freshdesk is single-service, so `default` does not appear.
    assert_eq!(
        Oip::new(
            Gid::new("com.freshdesk.api", DEFAULT_SERVICE, "v2"),
            "create"
        )
        .to_string(),
        "com.freshdesk.api:v2#create"
    );
}

/// Writing the elided name out is refused rather than accepted as a synonym: two spellings of one
/// address is how two consumers come to disagree about whether they hold the same one.
#[test]
fn an_explicit_default_segment_is_refused() {
    let error = Gid::parse("com.freshdesk.api/default:v2").expect_err("`default` must be elided");
    assert!(format!("{error}").contains("elided"), "{error}");
}

/// A deeper resource path is C-37's, and combining it with the elision is genuinely ambiguous — see
/// the module docs on `connector_spec::address`. Refused, not guessed at.
#[test]
fn a_gid_with_more_than_one_service_segment_is_refused() {
    let error = Gid::parse("com.zendesk.api/support/tickets:v2")
        .expect_err("a deeper path is not part of this grammar");
    assert!(format!("{error}").contains("C-37"), "{error}");
}

#[test]
fn a_malformed_address_is_refused_component_by_component() {
    // No version.
    assert!(Gid::parse("com.amazonaws/s3").is_err());
    // Empty version.
    assert!(Gid::parse("com.amazonaws/s3:").is_err());
    // A single-label authority is a name in no namespace.
    assert!(Gid::parse("amazonaws/s3:v1").is_err());
    assert!(Pid::parse("amazonaws").is_err());
    // Uppercase is not the scheme's spelling.
    assert!(Gid::parse("com.Amazonaws/s3:v1").is_err());
    assert!(Gid::parse("com.amazonaws/S3:v1").is_err());
    // An oip is not a gid and vice versa.
    assert!(Gid::parse("com.amazonaws/s3:v1#get").is_err());
    assert!(Oip::parse("com.amazonaws/s3:v1").is_err());
    assert!(Oip::parse("com.amazonaws/s3:v1#").is_err());
}

/// `api_version` resolves service-first, connector-second — the reason a multi-service provider is
/// describable at all. `base_url` resolves the same way.
#[test]
fn a_service_overrides_the_connector_version_and_base_url() {
    let connector = connector(
        vec![
            Service {
                name: "s3".to_owned(),
                description: "Object storage.".to_owned(),
                base_url: Some("https://s3.amazonaws.com".to_owned()),
                api_version: Some("2006-03-01".to_owned()),
            },
            service("inherits"),
        ],
        vec![operation("acme-a", "s3"), operation("acme-b", "inherits")],
    );

    assert_eq!(connector.api_version_of("s3"), Some("2006-03-01"));
    assert_eq!(connector.base_url_of("s3"), "https://s3.amazonaws.com");
    // The connector's values are the default, not a competing answer.
    assert_eq!(connector.api_version_of("inherits"), Some("v1"));
    assert_eq!(
        connector.base_url_of("inherits"),
        "https://api.acme.example"
    );

    assert_eq!(
        connector.gid_of("s3").map(|gid| gid.to_string()),
        Some("com.acme/s3:2006-03-01".to_owned())
    );
    assert_eq!(
        connector
            .oip_of(&connector.operations[0])
            .map(|oip| oip.to_string()),
        Some("com.acme/s3:2006-03-01#acme-a".to_owned())
    );
}

/// A connector missing either half of an address has none. Inventing a placeholder version would put
/// a wrong identifier into circulation under a contract that forbids reusing it.
#[test]
fn a_connector_without_an_authority_or_a_version_renders_no_address() {
    let mut connector = connector(Vec::new(), vec![operation("acme-a", DEFAULT_SERVICE)]);

    connector.authority = None;
    assert!(connector.pid().is_none());
    assert!(connector.gid_of(DEFAULT_SERVICE).is_none());

    connector.authority = Some("com.acme".to_owned());
    connector.api_version = None;
    assert!(connector.gid_of(DEFAULT_SERVICE).is_none());
    assert!(connector.oip_of(&connector.operations[0].clone()).is_none());
}
