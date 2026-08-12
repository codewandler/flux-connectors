//! Credential addressing: the tenant-scoped path a secret store is wrapped around.
//!
//! Two properties carry this module, and they are tested as properties rather than examples:
//!
//! - **`parse(render(r)) == r`** through the `default`-service elision — the law that makes a path an
//!   *identifier* and not merely a destination. A host reading a store back must be able to say what
//!   it found.
//! - **No input can render a traversing path.** Every segment lands in a filesystem-like store, and
//!   the tenant id is untrusted. The cautionary precedent is close to home: action-proxy puts
//!   `x-babelforce-customer-id` and `x-babelforce-integration-id` — both client headers — into a
//!   Vault path with no validation.
//!
//! There is no `proptest` here, for the same reason `service_partition.rs` has none: the crate takes
//! no new dependency, and a deterministic generator makes a failure reproducible from the seed in the
//! assertion rather than from a shape nobody can rebuild.

use connector_spec::credential::{
    validate_instance, validate_tenant, INSTANCES_SEGMENT, MAX_TENANT, TENANTS_ROOT,
};
use connector_spec::{
    provider, Connector, CredentialRef, InstanceId, Layout, TenantInstances, TenantLayout,
    DEFAULT_SERVICE,
};

use crate::shipped_provider;

/// The same tiny LCG `service_partition.rs` uses.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        self.0 >> 33
    }

    fn pick<'a, T>(&mut self, from: &'a [T]) -> &'a T {
        &from[(self.next() as usize) % from.len()]
    }
}

/// Components the generator draws from: valid spellings **and** the hostile ones the validators exist
/// to reject.
///
/// Mixed on purpose, and stated with the **validator as the gate** — a generator drawing only from
/// hand-picked valid components proves that the renderer and the parser agree with each other and
/// nothing else. It cannot see a component that renders into a string which parses back as something
/// *different*, and that is the failure that actually matters here: a traversing segment does not
/// produce an error, it produces a path to somebody else's secret.
const TENANTS: &[&str] = &[
    "9f3a4b2c-1d5e-4f60-8a7b-2c3d4e5f6071",
    "acme_corp",
    "tenant.eu-west-1",
    "1",
    // Hostile.
    "..",
    "../../etc",
    "a/b",
    "",
    ".",
    "with space",
    "tenants",
];

const AUTHORITIES: &[&str] = &[
    "com.zendesk.api",
    "com.amazonaws",
    "io.example.sub.domain",
    // Hostile: an embedded separator renders a path that means something else.
    "com.acme/s3",
    "acme",
    "",
    "Com.ACME",
];

const SERVICES: &[&str] = &[
    DEFAULT_SERVICE,
    "support",
    "s3",
    "bedrock-runtime",
    // Hostile.
    "a/b",
    "../../../../outside",
    "",
    "Support",
];

const CREDENTIALS: &[&str] = &[
    "api_token",
    "signing_secret",
    "access-key",
    // Hostile: a dot would read as nesting under a layout that splits on it.
    "zendesk.api_token",
    "a/b",
    "..",
    "",
];

/// Instances, drawn the same way. `None` is the ordinary single-connection case and is weighted
/// heavily on purpose: it is the address every stored credential is at, so most draws must exercise
/// the form that must not move.
const INSTANCES: &[Option<&str>] = &[
    None,
    None,
    None,
    Some("7c1e9a02-6b3d-4f11-9c8a-2d5e7f0b4c63"),
    Some("00000000-0000-4000-8000-000000000001"),
    // Hostile. A uuid is one value with one spelling here, so the uppercase, braced, URN and
    // unhyphenated forms are refused rather than normalised — each would be a second address for one
    // connection. The rest would traverse or forge a level.
    Some("7C1E9A02-6B3D-4F11-9C8A-2D5E7F0B4C63"),
    Some("{7c1e9a02-6b3d-4f11-9c8a-2d5e7f0b4c63}"),
    Some("urn:uuid:7c1e9a02-6b3d-4f11-9c8a-2d5e7f0b4c63"),
    Some("7c1e9a026b3d4f119c8a2d5e7f0b4c63"),
    Some("00000000-0000-0000-0000-000000000000"),
    Some("../../../../etc/passwd"),
    Some("a/b"),
    Some(""),
    Some("default"),
];

/// **The law.** Over the whole mixed corpus: every reference the constructor admits round-trips, and
/// every one it rejects is genuinely unrepresentable — never silently repaired into a different
/// address.
#[test]
fn every_admissible_reference_round_trips_and_no_rejected_one_renders() {
    let mut admitted = 0;
    let mut rejected = 0;
    let mut instanced = 0;

    // The corpus is mostly hostile by design, so only a small fraction of draws is admissible — the
    // seed count is what makes both sides of the assertion meaningful.
    for seed in 0..9000u64 {
        let mut rng = Rng(seed.wrapping_mul(2_654_435_761).wrapping_add(1));
        let tenant = rng.pick(TENANTS);
        let authority = rng.pick(AUTHORITIES);
        let service = rng.pick(SERVICES);
        let credential = rng.pick(CREDENTIALS);
        let instance = *rng.pick(INSTANCES);

        let built = match instance {
            Some(instance) => {
                CredentialRef::for_instance(tenant, authority, instance, service, credential)
            }
            None => CredentialRef::new(tenant, authority, service, credential),
        };
        match built {
            Ok(reference) => {
                admitted += 1;
                instanced += usize::from(reference.instance().is_some());
                let rendered = TenantLayout.render(&reference);

                // No admitted reference may render a segment that traverses.
                assert!(
                    !rendered.contains("/../") && !rendered.contains("//"),
                    "seed {seed}: rendered a traversing path {rendered:?}"
                );
                assert!(
                    !rendered.contains(&format!("/{DEFAULT_SERVICE}/")),
                    "seed {seed}: `{DEFAULT_SERVICE}` must never reach a path, got {rendered:?}"
                );

                assert_eq!(
                    TenantLayout.parse(&rendered),
                    Ok(reference),
                    "seed {seed}: {rendered:?} did not parse back to what rendered it"
                );
            }
            Err(_) => rejected += 1,
        }
    }

    // The corpus must actually exercise both sides, or the assertions above are vacuous.
    assert!(admitted > 50, "only {admitted} references were admitted");
    assert!(rejected > 50, "only {rejected} references were rejected");
    // And both address forms, or the law would be proven only for the one that existed before C-406.
    assert!(
        instanced > 20,
        "only {instanced} instanced references were admitted"
    );
    assert!(
        admitted - instanced > 20,
        "only {} un-instanced references were admitted",
        admitted - instanced
    );
}

#[test]
fn the_default_service_is_elided_and_the_elision_stays_unambiguous() {
    let elided = CredentialRef::new("t1", "com.slack.api", DEFAULT_SERVICE, "signing_secret")
        .expect("valid");
    let named =
        CredentialRef::new("t1", "com.slack.api", "events", "signing_secret").expect("valid");

    assert_eq!(
        TenantLayout.render(&elided),
        "tenants/t1/com.slack.api/signing_secret"
    );
    assert_eq!(
        TenantLayout.render(&named),
        "tenants/t1/com.slack.api/events/signing_secret"
    );

    // One optional middle segment, so the two forms cannot be confused — the property `Gid::parse`
    // rests on, restated here because a path with one more level would break it.
    assert_eq!(
        TenantLayout.parse(&TenantLayout.render(&elided)),
        Ok(elided)
    );
    assert_eq!(TenantLayout.parse(&TenantLayout.render(&named)), Ok(named));
}

// ---------------------------------------------------------------------------------------------
// One tenant, two connections to the same connector (C-406)
// ---------------------------------------------------------------------------------------------

/// The tenant that holds them. A uuid, because a real one is.
const TENANT: &str = "9f3a4b2c-1d5e-4f60-8a7b-2c3d4e5f6071";
const INSTANCE_US: &str = "7c1e9a02-6b3d-4f11-9c8a-2d5e7f0b4c63";
const INSTANCE_EU: &str = "b48d2f57-0a91-4c3e-8d16-5f2b7e904ac8";

fn two_instances() -> [InstanceId; 2] {
    [
        InstanceId::parse(INSTANCE_US).expect("a uuid"),
        InstanceId::parse(INSTANCE_EU).expect("a uuid"),
    ]
}

/// **The bug.** A tenant connects `acme.zendesk.com` and then `acme-eu.zendesk.com`. Before C-406
/// nothing in the address varied per connection, so both rendered one path: the second write
/// overwrote the first, and every later call resolved whichever credential survived — a `200` from
/// the wrong account rather than a refusal.
#[test]
fn two_instances_of_one_connector_for_one_tenant_render_different_addresses() {
    let connector = shipped("zendesk");
    let held = two_instances();

    let us = connector
        .credential_ref_for(
            TENANT,
            "zendesk.api_token",
            TenantInstances::held(&held, Some(&held[0])),
        )
        .expect("the tenant is valid and one connection is named")
        .expect("zendesk declares an authority");
    let eu = connector
        .credential_ref_for(
            TENANT,
            "zendesk.api_token",
            TenantInstances::held(&held, Some(&held[1])),
        )
        .expect("the tenant is valid and one connection is named")
        .expect("zendesk declares an authority");

    assert_ne!(
        TenantLayout.render(&us),
        TenantLayout.render(&eu),
        "two connections of one connector, for one tenant, must not share an address"
    );
    assert_eq!(
        TenantLayout.render(&us),
        format!("tenants/{TENANT}/com.zendesk.api/@instances/{INSTANCE_US}/api_token")
    );
    assert_eq!(
        TenantLayout.render(&eu),
        format!("tenants/{TENANT}/com.zendesk.api/@instances/{INSTANCE_EU}/api_token")
    );

    // And the law still holds over the new level: an address is an identifier, not merely a
    // destination.
    assert_eq!(TenantLayout.parse(&TenantLayout.render(&us)), Ok(us));
    assert_eq!(TenantLayout.parse(&TenantLayout.render(&eu)), Ok(eu));
}

/// **The property that makes the component safe to add at all.** A tenant with one connection keeps
/// the address it already has, byte for byte — an address that shifted would strand every credential
/// already stored, under every deployment, at once.
///
/// The literal is written out rather than derived, because deriving it from the same code that
/// renders it would assert only that the renderer agrees with itself.
#[test]
fn a_single_instance_address_is_byte_identical_to_the_four_component_form() {
    let connector = shipped("zendesk");
    const BEFORE: &str = "tenants/9f3a4b2c-1d5e-4f60-8a7b-2c3d4e5f6071/com.zendesk.api/api_token";

    let sole = connector
        .credential_ref_for(TENANT, "zendesk.api_token", TenantInstances::sole())
        .expect("valid")
        .expect("declared");
    assert_eq!(TenantLayout.render(&sole), BEFORE);
    assert!(sole.instance().is_none());

    // A host that knows its tenant's one connection and names it unconditionally gets the same
    // address: the instance is carried when it *distinguishes* something, and with one connection
    // there is nothing to distinguish. This is what lets a host pass the connection it is acting for
    // without branching on how many the tenant happens to hold.
    let held = [InstanceId::parse(INSTANCE_US).expect("a uuid")];
    let named = connector
        .credential_ref_for(
            TENANT,
            "zendesk.api_token",
            TenantInstances::held(&held, Some(&held[0])),
        )
        .expect("valid")
        .expect("declared");
    assert_eq!(TenantLayout.render(&named), BEFORE);
    assert_eq!(named, sole);

    // The service-scoped form is unchanged too — the instance sits above the service, so neither
    // optional level disturbs the other.
    assert_eq!(
        TenantLayout.render(
            &CredentialRef::new("t1", "com.zendesk.api", "support", "api_token").expect("valid")
        ),
        "tenants/t1/com.zendesk.api/support/api_token"
    );
}

/// **The ambiguous case refuses.** Two connections and a reference naming neither has no answer, and
/// the refusal names the ones that would have worked rather than picking one.
#[test]
fn several_instances_and_no_uuid_is_a_refusal_naming_what_would_have_worked() {
    let connector = shipped("zendesk");
    let held = two_instances();

    let refusal = connector
        .credential_ref_for(
            TENANT,
            "zendesk.api_token",
            TenantInstances::held(&held, None),
        )
        .expect_err("two connections and no uuid has no address");
    let message = refusal.to_string();

    assert!(
        message.contains(INSTANCE_US) && message.contains(INSTANCE_EU),
        "the refusal must name what would have worked, and this one says: {message}"
    );

    // Never a default and never the first match: nothing renders at all.
    assert!(
        !message.contains(TENANTS_ROOT),
        "the refusal must not offer an address it declined to render: {message}"
    );

    // A uuid the tenant does not hold is refused as well, rather than resolving to a plausible
    // address with nothing at it.
    let stranger = InstanceId::parse("11111111-2222-4333-8444-555555555555").expect("a uuid");
    assert!(connector
        .credential_ref_for(
            TENANT,
            "zendesk.api_token",
            TenantInstances::held(&held, Some(&stranger)),
        )
        .is_err());
}

/// A uuid that is not a uuid is refused at construction, and the refusal names the component — the
/// same guarantee every other component of an address already gives, for the same reason: a
/// reference can be built from outside a loaded `Connector`.
#[test]
fn an_instance_that_is_not_a_uuid_is_refused_and_the_refusal_names_the_component() {
    for hostile in [
        "production",
        "acme-eu",
        "../../etc",
        "7c1e9a02-6b3d-4f11-9c8a-2d5e7f0b4c6",
        "7c1e9a02-6b3d-4f11-9c8a-2d5e7f0b4c633",
        // One uuid with two spellings would be one connection at two addresses.
        "7C1E9A02-6B3D-4F11-9C8A-2D5E7F0B4C63",
        "7c1e9a026b3d4f119c8a2d5e7f0b4c63",
        "{7c1e9a02-6b3d-4f11-9c8a-2d5e7f0b4c63}",
        "urn:uuid:7c1e9a02-6b3d-4f11-9c8a-2d5e7f0b4c63",
        // The nil uuid names no connection, and "no connection" is already spelled by omitting it.
        "00000000-0000-0000-0000-000000000000",
        "",
    ] {
        assert!(
            validate_instance(hostile).is_err(),
            "instance {hostile:?} must be refused"
        );
        assert!(InstanceId::parse(hostile).is_err());

        let reason =
            CredentialRef::for_instance("t1", "com.zendesk.api", hostile, DEFAULT_SERVICE, "token")
                .expect_err("a reference must not be constructible from it");
        assert!(
            reason.contains("instance"),
            "the refusal must name the component, and this one says: {reason}"
        );
    }
}

/// The instanced form is a **second optional level**, and it stays unambiguous the same way the
/// service elision does: there is exactly one spelling of any one address.
#[test]
fn an_instanced_path_cannot_be_confused_with_a_service() {
    // The marker is unspellable as a component, so nothing can forge a level. That is a property of
    // the grammars, restated here because it is what the parse rests on.
    assert!(INSTANCES_SEGMENT.starts_with('@'));
    assert!(connector_spec::address::validate_service_name(INSTANCES_SEGMENT).is_err());
    assert!(connector_spec::address::validate_member_name(INSTANCES_SEGMENT).is_err());
    assert!(connector_spec::address::validate_authority(INSTANCES_SEGMENT).is_err());
    assert!(validate_tenant(INSTANCES_SEGMENT).is_err());

    // A uuid *is* a well-formed service name, which is exactly why the marker exists: without it,
    // `tenants/t1/com.acme.api/<uuid>/token` would be two addresses wearing one spelling.
    assert!(connector_spec::address::validate_service_name(INSTANCE_US).is_ok());

    let instanced =
        CredentialRef::for_instance("t1", "com.acme.api", INSTANCE_US, "support", "token")
            .expect("valid");
    assert_eq!(
        TenantLayout.render(&instanced),
        format!("tenants/t1/com.acme.api/@instances/{INSTANCE_US}/support/token")
    );
    assert_eq!(
        TenantLayout.parse(&TenantLayout.render(&instanced)),
        Ok(instanced)
    );

    for forged in [
        // A uuid where a service goes is a service named like a uuid, and nothing else.
        format!("tenants/t1/com.acme.api/{INSTANCE_US}/token"),
        // The marker without a uuid under it, the marker misspelled as a name, the elided service
        // written out, a uuid that is not one, and a level too many.
        format!("tenants/t1/com.acme.api/@instances/{INSTANCE_US}"),
        format!("tenants/t1/com.acme.api/instances/{INSTANCE_US}/token"),
        format!("tenants/t1/com.acme.api/@instances/{INSTANCE_US}/default/token"),
        "tenants/t1/com.acme.api/@instances/not-a-uuid/token".to_owned(),
        format!("tenants/t1/com.acme.api/@instances/{INSTANCE_US}/a/b/token"),
    ] {
        let parsed = TenantLayout.parse(&forged);
        assert!(
            parsed.as_ref().map(|r| r.instance().is_some()) != Ok(true),
            "path {forged:?} must not parse as an instanced address: {parsed:?}"
        );
    }
}

/// Writing `default` out by hand must not be a second spelling of the elided form.
#[test]
fn an_explicitly_spelled_default_service_does_not_parse() {
    assert!(TenantLayout
        .parse("tenants/t1/com.slack.api/default/signing_secret")
        .is_err());
}

#[test]
fn a_path_from_another_convention_is_refused_rather_than_guessed_at() {
    for foreign in [
        // action-proxy's shape.
        "customer/9f3a/integrations/abcd",
        // The Go credentials-store shape.
        "cloud/google/gemini",
        // flux's own Vault path.
        "secret/data/flux/plugin/slack/token",
        // The vendor's internal secret store's shape — the closest relative, and still not ours.
        "tenants/9f3a/credentials/abcd/extra",
        "tenants/9f3a",
        "",
        "/tenants/9f3a/com.acme.api/token",
    ] {
        assert!(
            TenantLayout.parse(foreign).is_err(),
            "path {foreign:?} must not parse as ours"
        );
    }
}

#[test]
fn the_tenant_validator_is_public_so_a_host_can_check_before_it_builds() {
    assert!(validate_tenant("9f3a4b2c-1d5e-4f60-8a7b-2c3d4e5f6071").is_ok());
    assert!(validate_tenant("../../etc").is_err());
    assert!(validate_tenant(&"a".repeat(MAX_TENANT)).is_ok());
    assert!(validate_tenant(&"a".repeat(MAX_TENANT + 1)).is_err());
    assert_eq!(TENANTS_ROOT, "tenants");
}

// ---------------------------------------------------------------------------------------------
// Derivation from a real connector
// ---------------------------------------------------------------------------------------------

fn shipped(name: &str) -> Connector {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../providers")
        .join(format!("{name}.toml"));
    let source = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    shipped_provider::load_definition(name, &source)
        .unwrap_or_else(|e| panic!("providers/{name}.toml must load:\n{e}"))
        .connector
}

/// One provider's paths, spelled out — the worked example behind the whole-catalogue assertion below.
///
/// Slack was chosen when it was the *only* provider with an authority; it stays because it is the one
/// whose rendered paths are quoted in `crates/connector-pack`'s own tests, so a change here shows up
/// as a mismatch there rather than as a silent divergence.
#[test]
fn slack_derives_a_path_for_each_of_its_credentials() {
    let connector = shipped("slack");

    let bot = connector
        .credential_ref_for("9f3a", "slack.bot_token", TenantInstances::sole())
        .expect("the tenant is valid")
        .expect("slack declares an authority");
    assert_eq!(
        TenantLayout.render(&bot),
        "tenants/9f3a/com.slack.api/bot_token",
        "the vendor prefix lives in the authority, so the leaf drops it"
    );

    let signing = connector
        .credential_ref_for("9f3a", "slack.signing_secret", TenantInstances::sole())
        .expect("valid")
        .expect("declared");
    assert_eq!(
        TenantLayout.render(&signing),
        "tenants/9f3a/com.slack.api/signing_secret"
    );

    // The API version is deliberately absent: slack declares `api_version = "v1"`, and a token must
    // survive the vendor's next version rather than forcing every tenant to re-provision.
    assert_eq!(connector.api_version.as_deref(), Some("v1"));
    assert!(
        !TenantLayout.render(&bot).contains("v1"),
        "a credential path carries no API version"
    );
}

/// The three outcomes are distinct because they have different owners: a bad tenant is the caller's,
/// a missing authority is the provider's, and a path is neither.
///
/// **The `Ok(None)` arm is built here rather than taken from `providers/`, and that is C-92's doing.**
/// It used to be `shipped("zendesk")`, which worked only while zendesk declared no authority; every
/// shipped provider declares one now
/// ([`every_shipped_provider_declares_an_authority_and_renders_a_credential_path`]), so no file in
/// that directory can stand for this case any more. The outcome itself has not gone away — `authority`
/// is still `Option`, a provider TOML may still omit it, and a host still has to tell "this connector
/// has no address" apart from "you asked for a credential it does not declare". So the case is spelled
/// out as the smallest connector that produces it.
#[test]
fn the_three_outcomes_are_distinguishable() {
    let connector = shipped("slack");

    assert!(
        connector
            .credential_ref_for("../etc", "slack.bot_token", TenantInstances::sole())
            .is_err(),
        "a bad tenant is an error naming what is wrong with it"
    );
    assert!(
        connector
            .credential_ref_for("9f3a", "slack.nonexistent", TenantInstances::sole())
            .is_err(),
        "an undeclared credential is an error"
    );

    const NO_AUTHORITY: &str = r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.test"
description = "A connector that declares no authority"
default_auth = [{ credentials = ["acme.token"] }]

[[auth]]
name = "acme.token"
scheme = "bearer"
env = ["ACME_TOKEN"]
description = "The token"

[[operations]]
id = "acme-ping"
method = "GET"
direction = "read"
path = "/ping"
description = "Ping"
risk = "low"
idempotency = "idempotent"
"#;
    let no_authority = provider::load("providers/acme.toml", NO_AUTHORITY)
        .expect("the fixture loads")
        .connector;
    assert!(
        no_authority.authority.is_none(),
        "the fixture exists to have no authority"
    );
    assert!(matches!(
        no_authority.credential_ref_for("9f3a", "acme.token", TenantInstances::sole()),
        Ok(None)
    ));
}

/// **Every shipped provider declares an authority, so every one of them has a credential path
/// (C-92).**
///
/// This replaces `only_providers_with_an_authority_have_credential_paths`, which existed to fail
/// here and said so: it asserted the two sets were *whatever the catalogue happened to make them*
/// and left `without` non-empty. The set that matters is now the empty one, and the reason is not
/// tidiness. Without an authority `Credentials::reference` refuses with `Error::NoCredentialAddress`
/// before any port is consulted, so a provider missing this one field cannot authenticate at all —
/// the credential path is the thing that makes the connector usable, not a label on it.
///
/// **The assertion is deliberately over the whole directory rather than a list.** A new provider
/// added without an authority has to fail here, on the day it is added, while its author is still
/// looking at the file — because an authority is published under the never-reused contract
/// `AGENTS.md` states, and the cheapest moment to choose one is before anything has been published
/// under it.
#[test]
fn every_shipped_provider_declares_an_authority_and_renders_a_credential_path() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../providers");
    let (mut without_authority, mut without_path, mut checked) = (Vec::new(), Vec::new(), 0);

    for entry in std::fs::read_dir(&dir).expect("providers/ is readable") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let name = path
            .file_stem()
            .expect("stem")
            .to_string_lossy()
            .to_string();
        let connector = shipped(&name);
        checked += 1;

        if connector.authority.is_none() {
            without_authority.push(name);
            continue;
        }

        // An authority is necessary *and sufficient*: a provider that declares one and still renders
        // no path means the derivation dropped it, which is a different defect from a missing field
        // and is reported as one. freshdesk declares no credential at all, deliberately, so it has
        // nothing to derive a path for and is skipped here rather than counted as a failure.
        let Some(method) = connector.auth.first() else {
            continue;
        };
        if connector
            .credential_ref_for("9f3a", &method.name, TenantInstances::sole())
            .expect("the tenant is valid")
            .is_none()
        {
            without_path.push(name);
        }
    }

    // Derived-set discipline (C-54): an empty providers/ must fail loudly, not pass vacuously.
    assert!(checked > 0, "no providers were checked");

    assert!(
        without_authority.is_empty(),
        "{} of {checked} providers declare no `authority`, so `Credentials::reference` refuses with \
         `NoCredentialAddress` and they cannot authenticate at all: {without_authority:?}",
        without_authority.len()
    );
    assert!(
        without_path.is_empty(),
        "these providers declare an authority but render no credential path — an authority is the \
         only thing a path needs, so the derivation dropped it: {without_path:?}"
    );
}

/// Every shipped credential is `<connector>.<local>`, which is what makes the leaf derivable at all.
#[test]
fn every_shipped_credential_is_prefixed_with_its_connector_id() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../providers");
    for entry in std::fs::read_dir(&dir).expect("providers/ is readable") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let name = path
            .file_stem()
            .expect("stem")
            .to_string_lossy()
            .to_string();
        let connector = shipped(&name);
        for method in &connector.auth {
            let leaf = connector
                .local_credential_name(&method.name)
                .unwrap_or_else(|e| panic!("providers/{name}.toml: {e}"));
            assert!(!leaf.is_empty());
            assert!(
                !leaf.contains('.'),
                "providers/{name}.toml: credential {:?} has a dotted local name {leaf:?}, which \
                 cannot be one path segment",
                method.name
            );
        }
    }
}
