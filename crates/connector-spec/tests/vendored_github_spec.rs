//! C-469: fixed-byte provenance and fail-closed scrub evidence for GitHub's REST OpenAPI document.

use std::path::{Path, PathBuf};
use std::process::Command;

use connector_spec::sha256_hex;
use serde_json::Value;

const SOURCE_COMMIT: &str = "5e28810649ba41b5483753ba74f976f83856a504";
const UPSTREAM_SHA256: &str = "281dc9e4ab24860c4010cea1bc90232175a6c92aa73dc569e1ccea6a5018cae9";
const SELECTED: [&str; 4] = [
    "issues/list-for-repo",
    "pulls/list-files",
    "actions/list-workflow-runs-for-repo",
    "repos/list-commits",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn provenance() -> toml::Table {
    let path = root().join("specs/github.provenance.toml");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
        .parse()
        .unwrap_or_else(|error| panic!("{} is not valid TOML: {error}", path.display()))
}

fn field<'a>(table: &'a toml::Table, name: &str) -> &'a str {
    table
        .get(name)
        .and_then(toml::Value::as_str)
        .unwrap_or_else(|| panic!("GitHub provenance has no string {name:?}"))
}

#[test]
fn provenance_pins_the_first_party_source_and_both_hashes() {
    let provenance = provenance();
    assert_eq!(field(&provenance, "source_commit"), SOURCE_COMMIT);
    assert_eq!(field(&provenance, "upstream_sha256"), UPSTREAM_SHA256);
    assert_eq!(field(&provenance, "upstream_version"), "1.1.4");
    assert_eq!(field(&provenance, "openapi_version"), "3.0.3");
    assert_eq!(
        field(&provenance, "source_url"),
        format!(
            "https://raw.githubusercontent.com/github/rest-api-description/{SOURCE_COMMIT}/descriptions/api.github.com/api.github.com.2022-11-28.json"
        )
    );

    let path = root().join(field(&provenance, "path"));
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    assert_eq!(sha256_hex(&bytes), field(&provenance, "sha256"));
}

#[test]
fn the_shared_scrubber_mutation_fixture_preserves_all_schema_declaration_maps_and_refuses_id_drift()
{
    let script = root().join("scripts/openapi_example_scrub.py");
    let output = Command::new("python3")
        .arg(&script)
        .arg("--self-test")
        .output()
        .unwrap_or_else(|error| panic!("cannot run {}: {error}", script.display()));
    assert!(
        output.status.success(),
        "{} --self-test failed: {}",
        script.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn the_vendored_document_keeps_the_contract_but_no_example_values() {
    let provenance = provenance();
    let path = root().join(field(&provenance, "path"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let document: Value = serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("{} is not JSON: {error}", path.display()));

    assert_eq!(document["openapi"], "3.0.3");
    assert_eq!(document["info"]["version"], "1.1.4");
    assert_eq!(document["info"]["license"]["name"], "MIT");
    assert_eq!(
        document["paths"].as_object().map(|paths| paths.len()),
        Some(805)
    );

    #[derive(Clone, Copy)]
    enum Mode {
        Regular,
        ComponentSections,
        Declarations,
        ExampleDeclarations,
        ExampleObject,
    }

    fn assert_no_example_values(value: &Value, at: &str, mode: Mode) {
        match value {
            Value::Object(object) => {
                for (key, value) in object {
                    assert!(
                        !matches!(mode, Mode::Regular)
                            || !matches!(key.as_str(), "example" | "examples"),
                        "example keyword value survived at {at}/{key}"
                    );
                    assert!(
                        !matches!(mode, Mode::ExampleObject)
                            || !matches!(key.as_str(), "value" | "externalValue"),
                        "component example value survived at {at}/{key}"
                    );
                    let child_mode = match mode {
                        Mode::ComponentSections if key == "examples" => Mode::ExampleDeclarations,
                        Mode::ComponentSections
                            if matches!(
                                key.as_str(),
                                "callbacks"
                                    | "headers"
                                    | "links"
                                    | "parameters"
                                    | "pathItems"
                                    | "requestBodies"
                                    | "responses"
                                    | "schemas"
                                    | "securitySchemes"
                            ) =>
                        {
                            Mode::Declarations
                        }
                        Mode::ComponentSections => Mode::Regular,
                        Mode::ExampleDeclarations => Mode::ExampleObject,
                        Mode::Declarations | Mode::ExampleObject => Mode::Regular,
                        Mode::Regular if key == "components" => Mode::ComponentSections,
                        Mode::Regular
                            if matches!(
                                key.as_str(),
                                "$defs"
                                    | "callbacks"
                                    | "dependentSchemas"
                                    | "definitions"
                                    | "encoding"
                                    | "headers"
                                    | "links"
                                    | "parameters"
                                    | "pathItems"
                                    | "patternProperties"
                                    | "properties"
                                    | "requestBodies"
                                    | "responses"
                                    | "schemas"
                                    | "securitySchemes"
                                    | "webhooks"
                            ) =>
                        {
                            Mode::Declarations
                        }
                        Mode::Regular => Mode::Regular,
                    };
                    assert_no_example_values(value, &format!("{at}/{key}"), child_mode);
                }
            }
            Value::Array(array) => {
                for (index, value) in array.iter().enumerate() {
                    assert_no_example_values(value, &format!("{at}/{index}"), Mode::Regular);
                }
            }
            _ => {}
        }
    }
    assert_no_example_values(&document, "", Mode::Regular);
    assert_eq!(
        document["components"]["examples"]
            .as_object()
            .map(|examples| examples.len()),
        Some(535),
        "the scrub must preserve named Example Object declarations while removing their values"
    );

    let mut operation_ids = Vec::new();
    for path_item in document["paths"]
        .as_object()
        .expect("paths is an object")
        .values()
    {
        for operation in path_item
            .as_object()
            .expect("a path item is an object")
            .values()
        {
            if let Some(operation_id) = operation.get("operationId").and_then(Value::as_str) {
                operation_ids.push(operation_id);
            }
        }
    }
    operation_ids.sort_unstable();
    operation_ids.dedup();
    for selected in SELECTED {
        assert!(
            operation_ids.binary_search(&selected).is_ok(),
            "selected operationId {selected:?} is absent"
        );
    }
}
