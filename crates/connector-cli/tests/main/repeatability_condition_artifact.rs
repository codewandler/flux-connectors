//! **The reason an authored conditional-idempotency claim is allowed reaches an artifact.**
//!
//! C-186's declaration would be worth very little if it stopped at the IR. `AGENTS.md` records six
//! declarable surfaces that the loader validates and no artifact carries, and calls that the largest
//! real gap in the repository: a field a host cannot read is prose with a schema attached. The
//! justification is the *evidence* for a claim that machines act on — `idempotency` reaches flux's
//! `ToolSpec` and a host decides from it whether a retry is safe — so publishing the claim while
//! keeping the evidence in a TOML comment would reproduce exactly the split C-186 exists to close,
//! one level up.
//!
//! So `web/public/catalog.json` carries it, and this asserts that from the **planned document**
//! rather than the committed one. The committed file is a whole-catalogue artifact the coordinator
//! regenerates at integration (`AGENTS.md`, "Whole-catalogue artifacts are coordinator-owned"), and
//! planning it here needs nothing written to the repository tree.

use connector_cli::pipeline;
use connector_cli::workspace::Workspace;
use serde_json::Value;

use crate::common::Fixture;

/// The document a build would write for this fixture, parsed.
const CATALOG_JSON: &str = "web/public/catalog.json";

/// A one-operation connector whose `POST` is idempotent by the vendor's behaviour and says so —
/// `cloudflare-cache-purge` reduced to the two fields this test is about.
fn purge_connector(justification: Option<&str>) -> String {
    let because = justification
        .map(|reason| format!("repeatable_because = {reason:?}\n"))
        .unwrap_or_default();
    format!(
        r#"id = "acme"
vendor = "Acme Inc."
base_url = "https://api.acme.example"
description = "A hand-authored fixture connector."

[[operations]]
id = "acme-cache-purge"
method = "POST"
direction = "write"
path = "/cache/purge"
description = "Empty the whole cache for this account."
risk = "high"
idempotency = "{idempotency}"
{because}"#,
        idempotency = if justification.is_some() {
            "conditional"
        } else {
            "non_idempotent"
        }
    )
}

/// Plan a build over a fixture holding exactly `provider`, and return the planned `catalog.json`.
fn planned_catalog(label: &str, provider: &str) -> Value {
    let fixture = Fixture::new(label);
    fixture.write_provider("acme", provider);

    let workspace = Workspace::new(fixture.root().to_path_buf());
    let plan = pipeline::plan(&workspace, None).expect("the fixture connector compiles");

    let document = plan
        .artifacts
        .iter()
        .find(|artifact| artifact.path.ends_with(CATALOG_JSON))
        .unwrap_or_else(|| panic!("a full build plans {CATALOG_JSON}"));

    serde_json::from_str(&document.contents).expect("the planned catalogue is valid JSON")
}

/// The single operation of the fixture document.
fn only_operation(document: &Value) -> &Value {
    document["providers"][0]["operations"][0]
        .as_object()
        .map(|_| &document["providers"][0]["operations"][0])
        .expect("the fixture connector publishes one operation")
}

/// **The claim and its evidence travel together.** A consumer reading authored conditional
/// idempotency can read why in the same object.
#[test]
fn a_conditional_post_publishes_its_condition_beside_the_claim() {
    let reason = "purging an already-purged cache is a no-op";
    let document = planned_catalog("conditional-post", &purge_connector(Some(reason)));
    let operation = only_operation(&document);

    assert_eq!(
        operation["idempotency"],
        Value::String("conditional".to_owned()),
        "the published claim must be the one the author declared"
    );
    assert_eq!(
        operation["repeatable_because"],
        Value::String(reason.to_owned()),
        "the justification must reach `{CATALOG_JSON}` verbatim — a reason that stops at the IR \
         leaves a consumer reading a conditional retry-safety claim with no way to audit it, \
         which is the gap this story closes rather than moves"
    );
}

/// **And the field is absent-shaped for everything else.** Publishing a reason on an operation
/// carrying no conditional-idempotency claim would turn deliberate evidence into ambient noise,
/// and a consumer could no longer tell the two apart by reading.
#[test]
fn a_non_idempotent_operation_publishes_no_condition() {
    let document = planned_catalog("unconditional-post", &purge_connector(None));
    let operation = only_operation(&document);

    assert_eq!(
        operation["idempotency"],
        Value::String("non_idempotent".to_owned())
    );
    assert_eq!(
        operation["repeatable_because"],
        Value::Null,
        "an operation stating no justification must publish none; the key is present and null \
         rather than missing, so a consumer reads one shape for every operation"
    );
}
