//! The C-505 ratchet: every native Flux adapter stays accounted for until a published,
//! Exchange-conformant replacement makes its removal safe.

use std::path::{Path, PathBuf};

use crate::common::Fixture;
use connector_cli::migration::{
    check, conformance_verdict, load_conformance, load_inventory, Verdict,
};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("connector-cli sits below the repository root")
        .to_path_buf()
}

#[test]
fn the_checked_inventory_maps_all_eighteen_adapters_once_in_fixed_wave_order() {
    let inventory = load_inventory(&repo_root()).expect("the committed inventory is valid");

    let adapters = inventory
        .adapters
        .iter()
        .map(|adapter| {
            (
                adapter.id.as_str(),
                adapter.connector.as_str(),
                adapter.wave.as_str(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        adapters,
        vec![
            ("confluence", "confluence", "C-499"),
            ("gitlab", "gitlab", "C-499"),
            ("jira", "jira", "C-499"),
            ("slack", "slack", "C-499"),
            ("docker", "docker", "C-500"),
            ("kubernetes", "kubernetes", "C-500"),
            ("alertmanager", "alertmanager", "C-501"),
            ("grafana", "grafana", "C-501"),
            ("loki", "loki", "C-501"),
            ("opsgenie", "opsgenie", "C-501"),
            ("prometheus", "prometheus", "C-501"),
            ("onepassword", "onepassword", "C-502"),
            ("sql", "sql", "C-502"),
            ("vault", "vault", "C-502"),
            ("aws", "aws", "C-503"),
            ("homer", "homer", "C-503"),
            ("huggingface", "huggingface", "C-503"),
            ("websearch", "websearch", "C-503"),
        ]
    );
    assert_eq!(
        inventory
            .support
            .iter()
            .map(|support| support.id.as_str())
            .collect::<Vec<_>>(),
        vec!["host-kit", "pack-index"]
    );
}

#[test]
fn equivalent_legacy_and_exchange_observations_are_conformant() {
    let document =
        load_conformance(&repo_root().join("migration/fixtures/equivalent-one-shot.json"))
            .expect("the equivalent fixture follows the shared format");

    assert_eq!(conformance_verdict(&document), Verdict::Conformant);
}

#[test]
fn the_shared_format_covers_every_observable_without_a_skip_escape_hatch() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("migration/conformance-v1.schema.json"))
            .expect("read the shared schema"),
    )
    .expect("the shared schema is JSON");
    let validator = jsonschema::validator_for(&schema).expect("the shared schema compiles");

    for fixture in ["equivalent-one-shot.json", "runtime-refused.json"] {
        let document: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(repo_root().join("migration/fixtures").join(fixture))
                .expect("read conformance fixture"),
        )
        .expect("fixture is JSON");
        assert!(
            validator.is_valid(&document),
            "{fixture} must validate against the published format"
        );
    }

    let mut escape: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("migration/fixtures/equivalent-one-shot.json"))
            .expect("read equivalent fixture"),
    )
    .expect("fixture is JSON");
    escape["cases"][0]["skip"] = true.into();
    assert!(
        !validator.is_valid(&escape),
        "a case cannot turn missing or divergent evidence into a skipped pass"
    );
    assert!(
        connector_cli::migration::parse_conformance(&escape.to_string()).is_err(),
        "the executable parser and the published schema must both reject the escape hatch"
    );

    let mut omitted_nullable: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("migration/fixtures/equivalent-one-shot.json"))
            .expect("read equivalent fixture"),
    )
    .expect("fixture is JSON");
    omitted_nullable["surface"]["operations"][0]
        .as_object_mut()
        .expect("operation is an object")
        .remove("output_schema");
    assert!(!validator.is_valid(&omitted_nullable));
    assert!(
        connector_cli::migration::parse_conformance(&omitted_nullable.to_string()).is_err(),
        "nullable fields remain required so a capture cannot silently omit its contract"
    );

    let mut boolean_sent: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("migration/fixtures/runtime-refused.json"))
            .expect("read refusal fixture"),
    )
    .expect("fixture is JSON");
    boolean_sent["cases"][0]["exchange"]["transcript"][0]["sent"] = false.into();
    assert!(!validator.is_valid(&boolean_sent));
    assert!(
        connector_cli::migration::parse_conformance(&boolean_sent.to_string()).is_err(),
        "dispatch state preserves Exchange's stable no/maybe wire vocabulary"
    );
}

#[test]
fn an_unsupported_runtime_is_an_explicit_refusal_and_never_conformant() {
    let document = load_conformance(&repo_root().join("migration/fixtures/runtime-refused.json"))
        .expect("the refusal fixture follows the shared format");

    assert!(matches!(
        conformance_verdict(&document),
        Verdict::Unsupported { ref code, .. } if code == "runtime_refused"
    ));
}

#[test]
fn missing_side_evidence_is_a_failure_not_a_skip() {
    let mut value: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("migration/fixtures/equivalent-one-shot.json"))
            .expect("read equivalent fixture"),
    )
    .expect("fixture is JSON");
    value["evidence"]["exchange"] = serde_json::Value::Null;
    value["cases"][0]["exchange"] = serde_json::Value::Null;

    let document = connector_cli::migration::parse_conformance(&value.to_string())
        .expect("missing evidence is a representable pending document");
    assert!(matches!(
        conformance_verdict(&document),
        Verdict::MissingEvidence { .. }
    ));
}

#[test]
fn removing_a_flux_adapter_requires_both_conformance_and_publication() {
    let fixture = Fixture::new("native-plugin-removal");
    write_single_adapter_inventory(&fixture);
    fixture.write("plugins/Cargo.toml", "[workspace]\nmembers = []\n");

    let missing = check(fixture.root(), fixture.root()).expect_err(
        "a disappeared legacy crate with neither evidence record must fail the release ratchet",
    );
    assert!(format!("{missing:#}").contains("acme"));

    fixture.write(
        "migration/conformance/acme.json",
        &equivalent_document("acme", "acme"),
    );
    let no_publication = check(fixture.root(), fixture.root())
        .expect_err("conformance alone must not claim a replacement was published");
    assert!(format!("{no_publication:#}").contains("publication"));

    fixture.write(
        "migration/publications/acme.json",
        &publication_receipt("acme", "acme"),
    );
    let report = check(fixture.root(), fixture.root())
        .expect("paired conformance and publication evidence permits exactly this removal");
    assert_eq!(report.inventoried, 1);
    assert_eq!(report.legacy_present, 0);
    assert_eq!(report.retired_with_evidence, 1);
}

#[test]
fn a_new_uninventoried_flux_plugin_fails_the_cross_repository_check() {
    let fixture = Fixture::new("native-plugin-unknown");
    write_single_adapter_inventory(&fixture);
    fixture.write(
        "plugins/Cargo.toml",
        "[workspace]\nmembers = [\"acme\", \"surprise\"]\n",
    );
    write_flux_plugin(&fixture, "acme");
    write_flux_plugin(&fixture, "surprise");

    let error = check(fixture.root(), fixture.root())
        .expect_err("an adapter absent from the checked inventory must fail closed");
    assert!(format!("{error:#}").contains("surprise"));
}

#[test]
fn an_implicit_cargo_binary_is_still_an_integration_not_support() {
    let fixture = Fixture::new("native-plugin-implicit-bin");
    write_single_adapter_inventory(&fixture);
    fixture.write("plugins/Cargo.toml", "[workspace]\nmembers = [\"acme\"]\n");
    fixture.write(
        "plugins/acme/Cargo.toml",
        "[package]\nname = \"flux-plugin-acme\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    fixture.write("plugins/acme/src/main.rs", "fn main() {}\n");

    let report = check(fixture.root(), fixture.root())
        .expect("Cargo's implicit src/main.rs target remains an official integration binary");
    assert_eq!(report.legacy_present, 1);
    assert_eq!(report.support_present, 0);
}

#[test]
fn the_shipped_command_runs_the_same_offline_release_check() {
    let fixture = Fixture::new("native-plugin-cli");
    write_single_adapter_inventory(&fixture);
    fixture.write("plugins/Cargo.toml", "[workspace]\nmembers = [\"acme\"]\n");
    write_flux_plugin(&fixture, "acme");

    let root = fixture.root().display().to_string();
    let invocation = connector_cli::cli::parse(
        [
            "migration-check",
            "--root",
            root.as_str(),
            "--flux-root",
            root.as_str(),
        ]
        .into_iter()
        .map(str::to_owned),
    )
    .expect("the migration command parses");
    let mut output = Vec::new();
    connector_cli::run(&invocation, &mut output).expect("the migration command passes");
    assert_eq!(
        String::from_utf8(output).expect("command output is UTF-8"),
        "native plugin migration check: 1 inventoried; 1 legacy present; 0 retired with evidence; 0 support crates\n"
    );
}

fn write_single_adapter_inventory(fixture: &Fixture) {
    fixture.write(
        "migration/native-plugins.toml",
        r#"schema = 1
waves = ["C-499", "C-500", "C-501", "C-502", "C-503"]

[[adapters]]
id = "acme"
flux_manifest = "plugins/acme/Cargo.toml"
flux_binary = "flux-plugin-acme"
legacy_contract = "plugins/acme/src/main.rs"
connector = "acme"
wave = "C-499"
conformance = "migration/conformance/acme.json"
publication = "migration/publications/acme.json"
"#,
    );
}

fn write_flux_plugin(fixture: &Fixture, id: &str) {
    fixture.write(
        &format!("plugins/{id}/Cargo.toml"),
        &format!(
            "[package]\nname = \"{id}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[[bin]]\nname = \"flux-plugin-{id}\"\npath = \"src/main.rs\"\n"
        ),
    );
    fixture.write(&format!("plugins/{id}/src/main.rs"), "fn main() {}\n");
}

fn equivalent_document(adapter: &str, connector: &str) -> String {
    let mut value: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("migration/fixtures/equivalent-one-shot.json"))
            .expect("read equivalent fixture"),
    )
    .expect("fixture is JSON");
    value["adapter"] = adapter.into();
    value["connector"] = connector.into();
    serde_json::to_string_pretty(&value).expect("render fixture") + "\n"
}

fn publication_receipt(adapter: &str, connector: &str) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "format": "flux-connectors-publication/v1",
        "adapter": adapter,
        "connector": connector,
        "release": "v1.0.0",
        "connector_commit": "0123456789abcdef0123456789abcdef01234567",
        "artifact": {
            "identity": "registry.example/connectors/acme@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        },
        "replacement_addresses": ["com.acme.api:v1#acme-get"],
        "migration_notes": "docs/migrations/acme.md"
    }))
    .expect("render receipt")
        + "\n"
}
