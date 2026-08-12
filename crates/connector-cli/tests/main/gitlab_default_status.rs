//! GitLab's reviewed SaaS default is configuration, not an unbound destination.

use std::path::{Path, PathBuf};

use connector_cli::status::{self, UNBOUND_BASE_URL_TEMPLATE};
use connector_spec::provider;

use crate::common::Fixture;

const CONFIGURED_ORIGIN: &str = "https://configured-origin-sentinel.example:8443";
const CONNECTION_LABEL: &str = "company-connection-label-sentinel";
const CREDENTIAL: &str = "SENTINEL-NOT-A-REAL-GITLAB-CREDENTIAL";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

#[test]
fn gitlab_com_is_reported_as_zero_configuration() {
    let path = repo_root().join("providers/gitlab.toml");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let connector = provider::load(path.to_str().expect("UTF-8 provider path"), &source)
        .unwrap_or_else(|error| panic!("GitLab must load: {error}"))
        .connector;

    let field = connector
        .config_field("origin")
        .expect("GitLab origin field");
    assert_eq!(field.default.as_deref(), Some("https://gitlab.com"));

    for operation in &connector.operations {
        let status = status::of(&connector, operation);
        assert!(
            !status
                .issues
                .iter()
                .any(|issue| issue.code == UNBOUND_BASE_URL_TEMPLATE),
            "`{}` reports GitLab's declared default as unbound: {:?}",
            operation.id,
            status.issues
        );
    }
}

#[test]
fn runtime_values_do_not_enter_generated_or_public_gitlab_artifacts() {
    let provider_source = std::fs::read_to_string(repo_root().join("providers/gitlab.toml"))
        .expect("the shipped GitLab provider is readable");
    let fixture = Fixture::new("c508-gitlab-runtime-values");
    fixture.write_provider("gitlab", &provider_source);

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_flux-connectors"))
        .args(["build", "--root"])
        .arg(fixture.root())
        // These are deliberately subprocess-only. A generated/public consumer must not resolve a
        // connection value, its Exchange-owned mutable label, or the credential environment value.
        .env("GITLAB_ORIGIN", CONFIGURED_ORIGIN)
        .env("FLUX_CONNECTORS_CONNECTION_LABEL", CONNECTION_LABEL)
        .env("GITLAB_TOKEN", CREDENTIAL)
        .output()
        .expect("the compiler binary runs");
    assert!(
        output.status.success(),
        "GitLab fixture build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let snapshot = fixture.snapshot();
    assert!(
        snapshot
            .keys()
            .any(|path| path == Path::new("web/public/catalog.json")),
        "the fixture emitted no public catalogue, so the non-leak check would be vacuous"
    );
    assert!(
        snapshot.values().any(|bytes| bytes
            .windows("GITLAB_TOKEN".len())
            .any(|window| window == b"GITLAB_TOKEN")),
        "the credential address did not reach any artifact, so absence of its value proves nothing"
    );
    for (path, bytes) in snapshot {
        let rendered = String::from_utf8_lossy(&bytes);
        for (kind, sentinel) in [
            ("configured origin", CONFIGURED_ORIGIN),
            ("connection label", CONNECTION_LABEL),
            ("credential value", CREDENTIAL),
        ] {
            assert!(
                !rendered.contains(sentinel),
                "{kind} leaked into generated/public artifact {}",
                path.display()
            );
        }
    }
}
