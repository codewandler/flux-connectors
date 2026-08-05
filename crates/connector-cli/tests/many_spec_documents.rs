//! One connector, several vendored documents — a document per service (C-410).
//!
//! `openapi_ingest.rs` covers one document and `spec_backed_provider.rs` covers the join between one
//! document and one patch set. This file covers the property neither can: that a provider's spec
//! **cache** is plural, that the provider file decides how many of its documents are compiled, and
//! that both of them reach the emitted artifacts.
//!
//! It goes through [`connector_cli::run`] rather than the loader, because the defect this story
//! exists to close lived in the layer between them. Discovery's `Provider::spec()` returned
//! `self.specs.last()` — the highest file stem — so for babelforce it selected the four-operation
//! `user` document over the 356-operation `manager` one. A loader test cannot see that: it is handed
//! its documents. Only a run that starts from a directory can.

mod common;

use common::Fixture;

/// Run the CLI the way `main` does, returning whatever it printed.
fn run(args: &[&str]) -> anyhow::Result<String> {
    let invocation = connector_cli::cli::parse(args.iter().map(|a| a.to_string()))?;
    let mut out = Vec::new();
    connector_cli::run(&invocation, &mut out)?;
    Ok(String::from_utf8(out).expect("CLI output is UTF-8"))
}

/// The `manager` half of the fixture vendor: root security, one operation, `/api/v2`.
const MANAGER: &str = r#"{
  "openapi": "3.0.3",
  "info": { "title": "Acme Manager", "version": "0.0.0-dev" },
  "servers": [{ "url": "https://services.acme.example" }],
  "security": [{ "oauth2": [] }],
  "paths": {
    "/api/v2/users/{user_id}": {
      "get": {
        "operationId": "getUser",
        "summary": "Fetch one managed user.",
        "parameters": [
          {
            "name": "user_id",
            "in": "path",
            "required": true,
            "schema": { "type": "string" }
          }
        ]
      }
    }
  }
}
"#;

/// The `user` half: a **different** `/api/v3` request published under the **same** `operationId`.
///
/// This is babelforce's real collision, reduced: `getUser` exists in both `manager-2026-07-10` and
/// `user-2026-06-25`, and they are not the same call. A build that resolved a patch's `select`
/// against whichever document it happened to read last would compile the wrong request, exit 0 and
/// emit plausible Flux.
const USER: &str = r#"{
  "openapi": "3.0.3",
  "info": { "title": "Acme User", "version": "0.0.0-dev" },
  "servers": [{ "url": "https://services.acme.example" }],
  "paths": {
    "/api/v3/me": {
      "get": {
        "operationId": "getUser",
        "summary": "Fetch the calling user.",
        "security": [{ "bearerAuth": [] }, { "oauth2": [] }]
      }
    }
  }
}
"#;

/// A provider declaring both documents, each joining its own service.
const PROVIDER: &str = r#"id = "acme"
vendor = "Acme"
base_url = "https://services.acme.example"
description = "Two vendor documents, one connector."

[[services]]
name = "manager"
description = "The management API."

[[services]]
name = "user"
description = "The user API."

[[spec]]
path = "specs/acme/manager-2026-07-10.json"
service = "manager"

[[spec]]
path = "specs/acme/user-2026-06-25.json"
service = "user"

[patch.directions.manager]
getUser = "read"

[patch.directions.user]
getUser = "read"

[[patch.operations]]
service = "manager"
select = "getUser"
rename = "acme-manager-user-get"
risk = "low"
idempotency = "idempotent"

[[patch.operations]]
service = "user"
select = "getUser"
rename = "acme-user-me-get"
risk = "low"
idempotency = "idempotent"
"#;

fn fixture(label: &str) -> Fixture {
    let fixture = Fixture::new(label);
    fixture.write_provider("acme", PROVIDER);
    fixture.write("specs/acme/manager-2026-07-10.json", MANAGER);
    fixture.write("specs/acme/user-2026-06-25.json", USER);
    fixture
}

/// **The property this story rests on.** Two documents sit in one provider's spec directory and
/// **both** reach the IR — one service each, one operation each, each carrying its own document's
/// request.
///
/// Before C-410 a provider compiled from exactly one document: `[[spec]]` did not parse, and
/// discovery's `Provider::spec()` picked the last file by stem. Either half alone silently drops
/// 356 of babelforce's 398 operations.
#[test]
fn two_documents_in_one_provider_both_reach_the_ir() {
    let fixture = fixture("many-specs-both-reach");

    run(&["build", "--root", fixture.root().to_str().unwrap()]).expect("the build succeeds");

    let manager = fixture.read("connectors/acme-manager.flux");
    let user = fixture.read("connectors/acme-user.flux");

    assert!(
        manager.contains("acme-manager-user-get"),
        "the manager document's operation is missing from its service module:\n{manager}"
    );
    assert!(
        manager.contains("/api/v2/users/{user_id}"),
        "the manager operation must carry the manager document's request:\n{manager}"
    );

    assert!(
        user.contains("acme-user-me-get"),
        "the user document's operation is missing from its service module:\n{user}"
    );
    assert!(
        user.contains("/api/v3/me"),
        "the user operation must carry the user document's request:\n{user}"
    );

    // The same `operationId` in two documents is two operations, not one won by file order.
    assert!(
        !manager.contains("/api/v3/me"),
        "a patch's `select` was resolved against the wrong document:\n{manager}"
    );
    assert!(
        !user.contains("/api/v2/users/{user_id}"),
        "a patch's `select` was resolved against the wrong document:\n{user}"
    );
}
