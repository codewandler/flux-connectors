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

use connector_spec::credential::{validate_tenant, MAX_TENANT, TENANTS_ROOT};
use connector_spec::{provider, Connector, CredentialRef, Layout, TenantLayout, DEFAULT_SERVICE};

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

/// **The law.** Over the whole mixed corpus: every reference the constructor admits round-trips, and
/// every one it rejects is genuinely unrepresentable — never silently repaired into a different
/// address.
#[test]
fn every_admissible_reference_round_trips_and_no_rejected_one_renders() {
    let mut admitted = 0;
    let mut rejected = 0;

    // The corpus is mostly hostile by design, so roughly one draw in thirty is admissible — the seed
    // count is what makes both sides of the assertion meaningful.
    for seed in 0..3000u64 {
        let mut rng = Rng(seed.wrapping_mul(2_654_435_761).wrapping_add(1));
        let tenant = rng.pick(TENANTS);
        let authority = rng.pick(AUTHORITIES);
        let service = rng.pick(SERVICES);
        let credential = rng.pick(CREDENTIALS);

        match CredentialRef::new(tenant, authority, service, credential) {
            Ok(reference) => {
                admitted += 1;
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
        // sbf/secrets' shape — the closest relative, and still not ours.
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
    provider::load(&format!("providers/{name}.toml"), &source)
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
        .credential_ref_for("9f3a", "slack.bot_token")
        .expect("the tenant is valid")
        .expect("slack declares an authority");
    assert_eq!(
        TenantLayout.render(&bot),
        "tenants/9f3a/com.slack.api/bot_token",
        "the vendor prefix lives in the authority, so the leaf drops it"
    );

    let signing = connector
        .credential_ref_for("9f3a", "slack.signing_secret")
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
            .credential_ref_for("../etc", "slack.bot_token")
            .is_err(),
        "a bad tenant is an error naming what is wrong with it"
    );
    assert!(
        connector
            .credential_ref_for("9f3a", "slack.nonexistent")
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
        no_authority.credential_ref_for("9f3a", "acme.token"),
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
            .credential_ref_for("9f3a", &method.name)
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
