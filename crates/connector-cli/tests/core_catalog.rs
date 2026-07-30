use std::path::{Path, PathBuf};

use connector_cli::pipeline;
use connector_cli::workspace::Workspace;
use serde_json::Value;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../specs/flux/core-v1.json"
));

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

fn changed(mut change: impl FnMut(&mut Value)) -> String {
    let mut value: Value = serde_json::from_str(SOURCE).unwrap();
    change(&mut value);
    serde_json::to_string_pretty(&value).unwrap() + "\n"
}

#[test]
fn the_vendored_flux_catalog_validates_and_preserves_the_foundational_distinctions() {
    let catalog = connector_cli::core_catalog::parse(SOURCE).expect("valid core catalogue");
    assert_eq!(catalog.operations.len(), 29);
    assert_eq!(catalog.nodes.len(), 43);
    assert_eq!(catalog.capabilities.len(), 5);
    assert!(catalog
        .operations
        .iter()
        .any(|entry| entry.name == "http.request"));
    assert!(catalog.operations.iter().any(|entry| entry.name == "map"));
    assert!(catalog
        .operations
        .iter()
        .any(|entry| entry.name == "filter"));
    assert!(catalog.nodes.iter().any(|entry| entry.name == "return"));
    assert!(!catalog.operations.iter().any(|entry| entry.name == "noop"));
    assert!(!catalog.nodes.iter().any(|entry| entry.name == "noop"));

    for name in ["dns", "tcp", "udp", "icmp"] {
        let capability = catalog
            .capabilities
            .iter()
            .find(|entry| entry.name == name)
            .unwrap_or_else(|| panic!("{name} capability"));
        assert_eq!(capability.availability.as_str(), "planned");
        assert!(!capability.callable);
        assert!(capability.operation_ids.is_empty());
    }
}

#[test]
fn malformed_core_catalogues_fail_closed() {
    let cases = [
        (
            "schema version",
            changed(|value| value["schema_version"] = 2.into()),
            "schema_version",
        ),
        (
            "canonical id",
            changed(|value| value["operations"][0]["$id"] = "https://example.test/all.json".into()),
            "canonical",
        ),
        (
            "duplicate id",
            changed(|value| value["operations"][1]["$id"] = value["operations"][0]["$id"].clone()),
            "duplicate",
        ),
        (
            "planned callable",
            changed(|value| {
                let capabilities = value["capabilities"].as_array_mut().unwrap();
                let dns = capabilities
                    .iter_mut()
                    .find(|entry| entry["name"] == "dns")
                    .unwrap();
                dns["callable"] = true.into();
            }),
            "planned",
        ),
        (
            "dangling node schema",
            changed(|value| {
                let nodes = value["nodes"].as_array_mut().unwrap();
                let return_node = nodes
                    .iter_mut()
                    .find(|entry| entry["name"] == "return")
                    .unwrap();
                return_node["schema_ref"] =
                    "https://flux.codewandler.org/v1/schema/flux-ast.schema.json#node-vanished"
                        .into();
            }),
            "anchor",
        ),
    ];

    for (label, source, expected) in cases {
        let error = connector_cli::core_catalog::parse(&source)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(expected),
            "{label} failed for the wrong reason: {error}"
        );
    }
}

#[test]
fn a_full_build_publishes_every_core_id_and_schema_once() {
    let workspace = Workspace::new(repo_root());
    let plan = pipeline::plan(&workspace, None).expect("full plan");
    let public = workspace.root().join("web/public");

    let catalog_artifact = plan
        .artifacts
        .iter()
        .find(|artifact| artifact.path == public.join("catalog.json"))
        .expect("public catalogue");
    let document: Value = serde_json::from_str(&catalog_artifact.contents).unwrap();
    assert_eq!(document["core"]["schema_version"], 1);

    let entries = document["core"]["operations"]
        .as_array()
        .unwrap()
        .iter()
        .chain(document["core"]["nodes"].as_array().unwrap())
        .chain(document["core"]["capabilities"].as_array().unwrap());
    for entry in entries {
        let id = entry["$id"].as_str().expect("entry id");
        let relative = id
            .strip_prefix("https://flux.codewandler.org/")
            .expect("canonical host");
        let expected = public.join(relative);
        assert_eq!(
            plan.artifacts
                .iter()
                .filter(|artifact| artifact.path == expected)
                .count(),
            1,
            "{id} maps to one public artifact"
        );
    }

    for name in [
        "core-catalog.schema.json",
        "core-entry.schema.json",
        "flux-ast.schema.json",
    ] {
        assert!(plan
            .artifacts
            .iter()
            .any(|artifact| artifact.path == public.join("v1/schema").join(name)));
    }
}
