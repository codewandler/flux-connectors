//! The emitted unit is the **service** (C-49), and for a `default`-only provider that changes nothing.
//!
//! Two claims, and they pull in opposite directions, which is why they are tested together:
//!
//! 1. **Nothing moved.** Every provider this repository ships is single-service, so every committed
//!    `.flux`, `.connector.toml`, per-operation rendering and generated catalogue table is a fixed
//!    point of a rebuild *after* the reshape. This is the regression proof for the whole story: an
//!    IR-level change that leaves 59 artifacts byte-identical is meaning-preserving for the existing
//!    catalogue by construction rather than by inspection.
//! 2. **Services really are the unit.** A two-service provider emits one module and one manifest per
//!    service, each carrying only its own operations and its own base URL, and one whole service can
//!    be selected.
//!
//! Everything here goes through [`connector_cli::run`] or [`connector_cli::pipeline::plan`], so what
//! is exercised is what the binary does.

mod common;

use std::path::{Path, PathBuf};

use common::Fixture;
use connector_cli::{pipeline, workspace::Workspace};

/// Run the CLI the way `main` does, returning whatever it printed.
fn run(args: &[&str]) -> anyhow::Result<String> {
    let invocation = connector_cli::cli::parse(args.iter().map(|a| a.to_string()))?;
    let mut out = Vec::new();
    connector_cli::run(&invocation, &mut out)?;
    Ok(String::from_utf8(out).expect("CLI output is UTF-8"))
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The provider names this repository ships, read from the directory rather than listed here.
fn shipped() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(repo_root().join("providers"))
        .expect("the providers directory must exist")
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
        .map(|path| {
            path.file_stem()
                .expect("a provider file has a stem")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no shipped providers found");
    names
}

/// **The byte-identity item, stated as a test.** Every installable and catalogue artifact this
/// repository commits is unchanged by a rebuild under services.
///
/// `web/public/catalog.json` is deliberately excluded: the published catalogue *gains* the service
/// fields C-42's consumers group by, which is an additive schema change and the one file this story
/// is allowed to move.
#[test]
fn the_shipped_artifacts_are_byte_identical() {
    let workspace = Workspace::new(repo_root());
    let plan = pipeline::plan(&workspace, None).expect("every shipped provider compiles");

    let moved: Vec<String> = plan
        .artifacts
        .iter()
        .filter(|artifact| artifact.change.is_change())
        .map(|artifact| workspace.display_path(&artifact.path).display().to_string())
        .filter(|path| !path.ends_with("catalog.json"))
        .collect();

    assert!(
        moved.is_empty(),
        "these committed artifacts are not byte-identical under services: {moved:?}"
    );
}

/// A `default`-only provider emits `<provider>.flux`, exactly as before — never a
/// `<provider>-default.flux`. The reserved service is elided from the file name for the same reason
/// it is elided from an address.
#[test]
fn every_shipped_provider_emits_its_unsuffixed_pair() {
    let workspace = Workspace::new(repo_root());
    let plan = pipeline::plan(&workspace, None).expect("every shipped provider compiles");

    let paths: Vec<String> = plan
        .artifacts
        .iter()
        .map(|artifact| workspace.display_path(&artifact.path).display().to_string())
        .collect();

    for provider in shipped() {
        for expected in [
            format!("connectors/{provider}.flux"),
            format!("connectors/{provider}.connector.toml"),
        ] {
            assert!(
                paths.contains(&expected),
                "a build must still plan {expected}; it planned {paths:?}"
            );
        }
    }
    let suffixed: Vec<&String> = paths
        .iter()
        .filter(|path| path.contains("-default."))
        .collect();
    assert!(
        suffixed.is_empty(),
        "the reserved `default` service must not reach a file name: {suffixed:?}"
    );
}

/// A two-service provider, AWS-shaped: one authority, one host and one API date per service.
const AWS: &str = r#"
id = "aws"
vendor = "Amazon Web Services"
authority = "com.amazonaws"
base_url = "https://amazonaws.com"
description = "Amazon Web Services."

[[services]]
name = "s3"
description = "Object storage."
base_url = "https://s3.amazonaws.com"
api_version = "2006-03-01"

[[services]]
name = "bedrock-runtime"
description = "Model inference."
base_url = "https://bedrock-runtime.us-east-1.amazonaws.com"
api_version = "2023-09-30"

[[operations]]
id = "aws-object-get"
service = "s3"
method = "GET"
path = "/objects/{key}"
description = "Fetch one object."
risk = "low"
idempotency = "idempotent"

[[operations.params.path]]
name = "key"
required = true
schema = { type = "string" }

[[operations]]
id = "aws-model-invoke"
service = "bedrock-runtime"
method = "POST"
path = "/model/{model_id}/invoke"
description = "Invoke a model."
risk = "medium"
idempotency = "non_idempotent"

[[operations.params.path]]
name = "model_id"
required = true
schema = { type = "string" }
"#;

fn aws_fixture(label: &str) -> Fixture {
    let fixture = Fixture::new(label);
    fixture.write_provider("aws", AWS);
    fixture
}

#[test]
fn a_two_service_provider_emits_one_pair_per_service() {
    let fixture = aws_fixture("two-service");
    run(&["build", "--root", fixture.root().to_str().unwrap()]).expect("build succeeds");

    for expected in [
        "connectors/aws-s3.flux",
        "connectors/aws-s3.connector.toml",
        "connectors/aws-bedrock-runtime.flux",
        "connectors/aws-bedrock-runtime.connector.toml",
    ] {
        assert!(fixture.exists(expected), "{expected} was not written");
    }
    assert!(
        !fixture.exists("connectors/aws.flux"),
        "a multi-service provider must not also emit a whole-provider module — it would publish an \
         installable unit no service owns"
    );

    let s3 = fixture.read("connectors/aws-s3.flux");
    assert!(s3.contains("op aws-object-get"), "{s3}");
    assert!(
        !s3.contains("op aws-model-invoke"),
        "the s3 module carries bedrock's operation:\n{s3}"
    );

    // `http_hosts` is C-10's and does not exist yet; the base URL is the value it will derive from,
    // and it is the service's own rather than the provider's.
    let manifest = fixture.read("connectors/aws-s3.connector.toml");
    assert!(
        manifest.contains("base_url = \"https://s3.amazonaws.com\""),
        "{manifest}"
    );
    assert!(
        !manifest.contains("bedrock"),
        "a service's manifest must not describe another service's surface:\n{manifest}"
    );
}

/// Acceptance: "Building can select one whole service — every operation belonging to it and nothing
/// else."
#[test]
fn selecting_a_service_builds_that_service_and_no_other() {
    for selector in ["s3", "com.amazonaws/s3:2006-03-01"] {
        let fixture = aws_fixture("select-service");
        run(&[
            "build",
            "--root",
            fixture.root().to_str().unwrap(),
            "--service",
            selector,
        ])
        .unwrap_or_else(|error| panic!("`--service {selector}` must build: {error:#}"));

        assert!(
            fixture.exists("connectors/aws-s3.flux"),
            "`--service {selector}` wrote no s3 module"
        );
        assert!(
            !fixture.exists("connectors/aws-bedrock-runtime.flux"),
            "`--service {selector}` also built bedrock-runtime"
        );

        let s3 = fixture.read("connectors/aws-s3.flux");
        assert!(!s3.contains("aws-model-invoke"), "{s3}");

        // A scoped run is not a function of the whole catalogue, so it must leave the
        // repository-wide document alone rather than truncating it — the rule `--provider` follows.
        assert!(
            !fixture.exists("web/public/catalog.json"),
            "a service-scoped run rewrote the whole-catalogue document"
        );
    }
}

#[test]
fn an_unknown_service_is_an_error_that_names_the_available_ones() {
    let fixture = aws_fixture("unknown-service");
    let error = run(&[
        "build",
        "--root",
        fixture.root().to_str().unwrap(),
        "--service",
        "s4",
    ])
    .expect_err("an unknown service must not silently build everything");

    let rendered = format!("{error:#}");
    assert!(rendered.contains("s4"), "{rendered}");
    assert!(rendered.contains("s3"), "{rendered}");
    assert!(rendered.contains("bedrock-runtime"), "{rendered}");
    // The gid is the other spelling a selector may use, so the error offers both.
    assert!(
        rendered.contains("com.amazonaws/s3:2006-03-01"),
        "{rendered}"
    );
}

/// The published catalogue carries the service, so a consumer can group by it — C-42's schema, written
/// back to as the story requires. `default` appears here, unlike in an address: this is data about the
/// catalogue, not a published identifier.
#[test]
fn the_published_catalogue_carries_the_service() {
    let document: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("web/public/catalog.json"))
            .expect("the committed catalogue must exist"),
    )
    .expect("the catalogue is JSON");

    let providers = document["providers"]
        .as_array()
        .expect("the catalogue lists providers");
    assert!(!providers.is_empty());

    for provider in providers {
        let services = provider["services"]
            .as_array()
            .expect("every provider publishes its services");
        assert_eq!(
            services.len(),
            1,
            "provider `{}` is single-service today",
            provider["id"]
        );
        assert_eq!(services[0]["name"], serde_json::json!("default"));
        // No authority is declared yet, so no address renders — stated as `null`, never invented.
        assert_eq!(services[0]["gid"], serde_json::Value::Null);
        assert_eq!(provider["authority"], serde_json::Value::Null);

        let operations = provider["operations"]
            .as_array()
            .expect("every provider publishes its operations");
        let counted: u64 = services
            .iter()
            .map(|service| service["operation_count"].as_u64().unwrap_or_default())
            .sum();
        assert_eq!(
            counted as usize,
            operations.len(),
            "the per-service counts must sum to the provider's, because services partition the \
             operation set"
        );
        for operation in operations {
            assert_eq!(operation["service"], serde_json::json!("default"));
        }
    }
}
