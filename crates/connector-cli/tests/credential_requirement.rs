//! **The embedded catalogue can say why an operation names no credential** (C-235).
//!
//! `catalog::Operation::credentials` is an empty slice in two opposite situations, and until this
//! story a host linking `connector-catalog` could not tell which one it was holding:
//!
//! - **positively public** — the connector *declares* `auth = []` on the operation, C-206's
//!   `no-credential-required`. The unauthenticated call is the correct call.
//! - **withheld** — nothing is declared anywhere, C-206's `no-credential`. A credential exists and
//!   this repository cannot hold it safely yet, so the call goes out unauthenticated and 401s.
//!   Freshdesk is the shipped case.
//!
//! C-206 taught the *published* catalogue (`web/public/catalog.json`) to separate them, through
//! `status.notes`. The **embedded** catalogue — the Rust table a host links — carried only the
//! flattened mechanism list, so the distinction stopped at the crate boundary and
//! `connectors-api` had to infer the state from an absence.
//!
//! # Why the assertion is over the emitted table and not over `providers/`
//!
//! Neither state has to be waited for here: both are written down as provider TOML and pushed
//! through the real loader and the real emitter, which is the whole path a host's answer comes
//! from. Nothing in this file walks `providers/` or counts the catalogue, so a fifty-fourth
//! connector cannot turn it red merely by existing (`AGENTS.md`, "A per-provider test asserts about
//! its provider, never about the catalogue").
//!
//! The genuinely-public case still ships in no provider file — C-133 and C-157 are the candidates
//! — which is exactly why it is written as a fixture rather than looked up.

use connector_cli::catalog::{render, OperationRendering};
use connector_cli::seam::{load, ProviderInputs};

/// A connector declaring one credential and one operation. `{auth}` is substituted with the
/// operation's own `auth` declaration, which is the one thing that moves between the two cases.
const CONNECTOR: &str = r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.example"
description = "The Acme support API."

[[auth]]
name = "acme.api_token"
scheme = "bearer"
env = ["ACME_API_TOKEN"]

[[operations]]
id = "acme-ping"
method = "GET"
direction = "read"
path = "/v2/ping"
description = "Check that the Acme API is reachable."
risk = "low"
idempotency = "idempotent"
{auth}
"#;

/// Render one connector's generated catalogue table.
fn table(auth: &str) -> String {
    let inputs = ProviderInputs {
        name: "acme".to_owned(),
        definition: CONNECTOR.replace("{auth}", auth),
        specs: Vec::new(),
    };
    let connector = load(&inputs).expect("the fixture connector loads");
    let renderings = vec![OperationRendering {
        id: "acme-ping".to_owned(),
        source: "op acme-ping() -> Any\n".to_owned(),
    }];
    render(&connector, &renderings).expect("the fixture connector renders")
}

/// The one line the table says about the operation's credentials, whatever else moves around it.
fn credential_lines(table: &str) -> Vec<&str> {
    table
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("credential"))
        .collect()
}

/// **The failing-first assertion.** A positively-public operation and a withheld one must not be
/// emitted identically into the table a host links against.
///
/// Before C-235 both rendered `credentials: &[],` and nothing else, so the two lists below were
/// equal and a host reading the embedded catalogue had one value for two opposite facts.
///
/// The comparison is on what the table says *about credentials*, not on the whole file: an
/// assertion over the whole rendering would also pass if some unrelated field happened to differ,
/// which is not the property this story is about.
#[test]
fn the_embedded_catalogue_tells_a_public_operation_from_a_withheld_one() {
    // C-206's positive declaration: the vendor needs nothing for this operation.
    let public = table("auth = []");
    // Nothing declared at all — the operation inherits the connector's default, which the fixture
    // also leaves unset. Freshdesk's shape.
    let withheld = table("");

    assert_ne!(
        credential_lines(&public),
        credential_lines(&withheld),
        "a positively-public operation and a withheld one are emitted identically into the \
         embedded catalogue, so no host linking `connector-catalog` can tell them apart:\n\
         public:   {:?}\nwithheld: {:?}",
        credential_lines(&public),
        credential_lines(&withheld)
    );

    // Both still name no credential — the mechanism list is unchanged, and the distinction is
    // carried beside it rather than by giving one of them a credential it does not have.
    for rendered in [&public, &withheld] {
        assert!(
            rendered.contains("credentials: &[],"),
            "neither state may invent a credential: {rendered}"
        );
    }

    // And each says which state it is, positively.
    assert!(
        public.contains("credential_requirement: crate::CredentialRequirement::NoneRequired,"),
        "{public}"
    );
    assert!(
        withheld.contains("credential_requirement: crate::CredentialRequirement::Withheld,"),
        "{withheld}"
    );
}

/// **The declaration is read off the operation, never inferred from an absence.**
///
/// C-206's rule, restated where C-235 could most easily have broken it. A rule that *guessed*
/// "public" from "no credential field anywhere" would be the original bug with more steps, and it
/// would guess wrong on freshdesk — the one connector already shipping in that shape, and the one
/// this story's answer for the withheld state is measured against.
#[test]
fn a_missing_credential_is_never_read_as_a_public_operation() {
    let withheld = table("");
    assert!(
        !withheld.contains("CredentialRequirement::NoneRequired"),
        "a connector that declares nothing must not be read as declaring that nothing is needed: \
         {withheld}"
    );
}

/// **An authenticated operation is unaffected**, which is what keeps the 665 shipped operations
/// that declare a credential saying exactly what they said before.
#[test]
fn an_authenticated_operation_declares_its_requirement() {
    let authenticated = table("auth = [{ credentials = [\"acme.api_token\"] }]");
    assert!(
        authenticated.contains(r#"credentials: &[&["acme.api_token"]],"#),
        "{authenticated}"
    );
    assert!(
        authenticated.contains("credential_requirement: crate::CredentialRequirement::Declared,"),
        "{authenticated}"
    );
}
