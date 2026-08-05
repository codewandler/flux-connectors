//! The two properties C-49 rests on: **services partition the operation set**, and **an address
//! round-trips through the `default` elision**.
//!
//! Both are properties rather than examples, so both are tested over a generated corpus. There is no
//! `proptest` here on purpose — the crate takes no new dependency for this — so the generator is a
//! deterministic LCG. Deterministic is what a property test in this repository wants anyway: a
//! failure is reproducible from the seed printed in the assertion, and CI cannot go red on a shape
//! nobody can rebuild.

use connector_spec::{
    Connector, Gid, HttpMethod, Idempotency, Oip, Operation, OperationDirection, ParamSet, Pid,
    Provenance, Quirks, Risk, Service, DEFAULT_SERVICE,
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
        direction: OperationDirection::Read,
        path: "/v1/things".to_owned(),
        description: String::new(),
        risk: Risk::Low,
        idempotency: Idempotency::Idempotent,
        semantic_effects: Vec::new(),
        repeatable_because: None,
        expose: true,
        auth: None,
        params: ParamSet::default(),
        response_schema: None,
        credential_response: Vec::new(),
        produces_credential: None,
        quirks: Quirks::default(),
    }
}

fn connector(services: Vec<Service>, operations: Vec<Operation>) -> Connector {
    Connector {
        id: "acme".to_owned(),
        authority: Some("com.acme".to_owned()),
        runtime: connector_spec::Runtime::Http,
        api_version: Some("v1".to_owned()),
        services,
        vendor: "Acme".to_owned(),
        base_url: "https://api.acme.example".to_owned(),
        description: String::new(),
        auth: Vec::new(),
        default_auth: Vec::new(),
        operations,
        events: Vec::new(),
        channels: Vec::new(),
        config: Vec::new(),
        verify: None,
        graphs: Vec::new(),
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

/// Component spellings the generators draw from: valid ones **and** the hostile ones the validators
/// exist to reject.
///
/// The corpus is mixed on purpose. A generator drawing only from hand-picked valid components proves
/// that the renderer and the parser agree with each other and nothing else — it cannot see a component
/// that renders into a string which parses back as something *different*, and that is the failure that
/// actually occurred. So the property below is stated over the whole corpus with the **validator as
/// the gate**.
const AUTHORITIES: &[&str] = &[
    "com.amazonaws",
    "com.zendesk.api",
    "io.example.sub.domain",
    // Hostile: an embedded separator renders an address that reparses as a *different* one.
    "com.acme/s3",
    "com.acme:1",
    "com.acme#x",
    "",
    "acme",
    "Com.ACME",
    "com..acme",
    "com.acme.",
];

const SERVICES: &[&str] = &[
    DEFAULT_SERVICE,
    "s3",
    "bedrock-runtime",
    "support",
    "v2-beta",
    // Hostile: `/` splits the segment, and `..` would also reach the emitted file path.
    "a/b",
    "../../../../outside/pwned",
    "My Service",
    "S3",
    "",
    "s3.",
];

const VERSIONS: &[&str] = &[
    "v1",
    "v2",
    "2006-03-01",
    "2023-09-30",
    "",
    "v2/beta",
    "v2:1",
    "v2#x",
];

/// Whether the address grammar admits a triple at all. `default` is admissible as a service — it
/// renders as nothing, which is the elision under test.
fn is_admissible(authority: &str, service: &str, api_version: &str) -> bool {
    connector_spec::address::validate_authority(authority).is_ok()
        && (service == DEFAULT_SERVICE
            || connector_spec::address::validate_service_name(service).is_ok())
        && connector_spec::address::validate_api_version(api_version).is_ok()
}

/// **The validators are exactly the gate on round-tripping.** Over the mixed corpus: every triple they
/// admit round-trips, and every triple they reject fails to parse back — rather than parsing back as
/// something else.
///
/// The second half is what catches a masquerade. `authority = "com.acme/s3"` renders `com.acme/s3:v2`,
/// which parses *successfully* — as `authority = com.acme`, `service = s3`. Unvalidated, that is a
/// valid-looking address for a connector that never declared one, which is why the loader refuses the
/// component and why this test asserts the refusal rather than the round trip.
#[test]
fn the_validators_decide_which_addresses_round_trip() {
    let mut admissible = 0;
    let mut rejected = 0;

    for authority in AUTHORITIES {
        for service in SERVICES {
            for api_version in VERSIONS {
                let gid = Gid::new(authority, service, api_version);
                let rendered = gid.to_string();

                if is_admissible(authority, service, api_version) {
                    assert_eq!(
                        Gid::parse(&rendered).ok(),
                        Some(gid.clone()),
                        "an admissible gid must round-trip: {rendered}"
                    );
                    assert!(
                        !rendered.contains(&format!("/{DEFAULT_SERVICE}")),
                        "`{DEFAULT_SERVICE}` must never reach a rendered address: {rendered}"
                    );
                    admissible += 1;
                } else {
                    assert_ne!(
                        Gid::parse(&rendered).ok().as_ref(),
                        Some(&gid),
                        "a gid the validators reject must not round-trip, or it masquerades as a \
                         valid address: {rendered}"
                    );
                    rejected += 1;
                }
            }
        }
    }

    // A corpus that stopped covering one side of the gate would make this test vacuous.
    assert!(admissible > 50, "only {admissible} admissible triples");
    assert!(rejected > 50, "only {rejected} rejected triples");
}

/// **Every address the loader lets through round-trips.** The property stated where it matters: over
/// provider files rather than over constructed values, because the loader is the only gate between a
/// content field and a published address.
///
/// A file that loads must yield a gid that parses back to itself; a component the grammar rejects must
/// make the file fail to load. There is no third outcome — in particular, no "loads, renders, reparses
/// as something else".
#[test]
fn a_provider_file_that_loads_publishes_only_round_tripping_addresses() {
    let mut loaded = 0;
    let mut refused = 0;

    for authority in AUTHORITIES {
        for service in SERVICES {
            for api_version in VERSIONS {
                let source = format!(
                    "id = \"acme\"\nbase_url = \"https://api.acme.example\"\n\
                     authority = {authority:?}\napi_version = {api_version:?}\n\n\
                     [[services]]\nname = {service:?}\n\n\
                     [[operations]]\nid = \"acme-thing-get\"\nservice = {service:?}\n\
                     method = \"GET\"\ndirection = \"read\"\npath = \"/v1/things\"\nrisk = \"low\"\n\
                     idempotency = \"idempotent\"\n"
                );

                match connector_spec::provider::load("providers/fuzz.toml", &source) {
                    Err(_) => refused += 1,
                    Ok(loaded_provider) => {
                        loaded += 1;
                        let connector = loaded_provider.connector;
                        for name in connector.service_names() {
                            let Some(gid) = connector.gid_of(name) else {
                                continue;
                            };
                            let rendered = gid.to_string();
                            assert_eq!(
                                Gid::parse(&rendered).ok(),
                                Some(gid),
                                "the loader accepted a provider whose service address does not parse \
                                 back: {rendered}\n{source}"
                            );
                        }
                    }
                }
            }
        }
    }

    // `default` is a reserved service name, so every triple naming it is refused — which still leaves
    // the valid triples loading. Both counts must be non-trivial or the property proves nothing.
    assert!(loaded > 20, "only {loaded} provider files loaded");
    assert!(refused > 50, "only {refused} provider files were refused");
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
                legacy: false,
                description: "Object storage.".to_owned(),
                base_url: Some("https://s3.amazonaws.com".to_owned()),
                api_version: Some("2006-03-01".to_owned()),
                roles: Vec::new(),
                tags: Vec::new(),
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
