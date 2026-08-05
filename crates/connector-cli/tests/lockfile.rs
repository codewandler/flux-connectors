//! `connectors.lock` is **written**, and it is an artifact like every other one — C-189.
//!
//! `crates/connector-spec/tests/lockfile.rs` owns the hash domain: what a row records, that the file
//! is a pure function of its entries, and that no timestamp reaches it. None of that was worth
//! anything, because nothing produced the file. Three CHANGELOG entries asserted byte-identity of an
//! artifact no build emitted, and every one of them was vacuously true.
//!
//! So this file asserts the half that was missing, at the layer that owns it:
//!
//! 1. **A full build writes it**, and a rebuild over unchanged inputs rewrites nothing.
//! 2. **Every recorded hash is the hash of a committed byte sequence** — the provider TOML on disk,
//!    each vendored document, each generated artifact. A lockfile of plausible-looking hex that
//!    matches nothing would pass a shape test and detect no drift at all.
//! 3. **`diff` reports a stale one**, which is what makes it a checked artifact rather than a file
//!    the build happens to drop.
//! 4. **It is whole-catalogue** (`AGENTS.md`): a `--provider` or `--service` run compiled a subset,
//!    so it must leave the committed file alone rather than truncating it to what it saw.
//!
//! The committed-tree fixed point is asserted here too. It is a whole-catalogue staleness check, so
//! it is red in a provider story's worktree for exactly the reason the other four are — see
//! `AGENTS.md`, *Whole-catalogue artifacts are coordinator-owned*.

use std::path::{Path, PathBuf};

use connector_cli::pipeline::{self, Change};
use connector_cli::workspace::Workspace;
use connector_spec::{sha256_hex, Lockfile, LOCKFILE_NAME};

mod common;

use common::Fixture;

/// Run the CLI the way `main` does, returning whatever it printed.
fn run(args: &[&str]) -> anyhow::Result<String> {
    let invocation = connector_cli::cli::parse(args.iter().map(|arg| arg.to_string()))?;
    let mut out = Vec::new();
    connector_cli::run(&invocation, &mut out)?;
    Ok(String::from_utf8(out).expect("CLI output is UTF-8"))
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root exists")
}

/// The lockfile a fixture build wrote, parsed.
fn lockfile_of(fixture: &Fixture) -> Lockfile {
    assert!(
        fixture.exists(LOCKFILE_NAME),
        "a full build must write {LOCKFILE_NAME}"
    );
    Lockfile::parse(&fixture.read(LOCKFILE_NAME)).expect("the written lockfile parses")
}

// ---------------------------------------------------------------------------------------------
// The file exists, and is a fixed point
// ---------------------------------------------------------------------------------------------

/// **The story's headline.** A full build emits the lockfile, and building again writes nothing.
#[test]
fn a_full_build_writes_the_lockfile_and_a_rebuild_is_a_no_op() {
    let fixture = Fixture::with_provider("lock-written", "zendesk");
    fixture.write_provider("freshdesk", &common::definition("freshdesk"));
    let root = fixture.root().to_str().unwrap().to_string();

    run(&["build", "--root", &root]).expect("the first build succeeds");
    let after_first = fixture.snapshot();

    let lockfile = lockfile_of(&fixture);
    let ids: Vec<&str> = lockfile
        .entries()
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(
        ids,
        ["freshdesk", "zendesk"],
        "the lockfile records one row per provider, ordered by id"
    );

    let second = run(&["build", "--root", &root]).expect("the second build succeeds");
    assert_eq!(
        after_first,
        fixture.snapshot(),
        "a rebuild over unchanged inputs rewrote the lockfile"
    );
    assert!(
        second.contains("up to date"),
        "the second build should report itself a no-op, got:\n{second}"
    );
}

/// **The committed `connectors.lock` follows from the committed tree.**
///
/// A whole-catalogue staleness check, and therefore red in a provider story's worktree for the same
/// reason the other four are: the implementor correctly did not write a whole-catalogue file. The
/// coordinator's full build at integration resolves it.
#[test]
fn the_committed_lockfile_is_a_fixed_point_of_a_build() {
    let workspace = Workspace::new(repo_root());
    let plan = pipeline::plan(&workspace, None).expect("every shipped provider compiles");

    let planned = plan
        .artifacts
        .iter()
        .find(|artifact| artifact.path == workspace.lockfile_path())
        .unwrap_or_else(|| panic!("a full build must plan {LOCKFILE_NAME}"));

    assert_eq!(
        planned.change,
        Change::Unchanged,
        "the committed {LOCKFILE_NAME} is stale — run `cargo run -p connector-cli -- build`"
    );
}

// ---------------------------------------------------------------------------------------------
// Every hash is the hash of something committed
// ---------------------------------------------------------------------------------------------

/// The recorded hashes are checkable, one input at a time — which is the whole point of recording
/// them separately. `flux-connectors check` (C-14) recomputes exactly these.
///
/// Asserted against the bytes **on disk** rather than against the plan, because that is the question
/// `check` asks: does the committed tree still follow from the committed inputs.
#[test]
fn every_recorded_hash_is_the_hash_of_a_committed_input() {
    let fixture = Fixture::with_provider("lock-hashes", "zendesk");
    let root = fixture.root().to_str().unwrap().to_string();
    run(&["build", "--root", &root]).expect("the build succeeds");

    let entry = lockfile_of(&fixture)
        .entry("zendesk")
        .cloned()
        .expect("the lockfile records the provider that was built");

    assert_eq!(
        entry.toml_sha256.as_deref(),
        Some(sha256_hex(fixture.read("providers/zendesk.toml").as_bytes()).as_str()),
        "toml_sha256 must be the hash of the provider file as committed"
    );
    assert_eq!(
        entry.generator,
        connector_cli::seam::generator(),
        "the row records the generator that produced it"
    );

    // The generated artifacts, keyed by repository-relative path so a row names a file a reader can
    // open rather than a stem they have to reconstruct.
    assert!(
        entry.artifacts.contains_key("connectors/zendesk.flux"),
        "the row must cover the shipped module; it covers {:?}",
        entry.artifacts.keys().collect::<Vec<_>>()
    );
    assert!(
        entry
            .artifacts
            .contains_key("connectors/zendesk.connector.toml"),
        "the row must cover the shipped manifest"
    );
    for (path, recorded) in &entry.artifacts {
        assert!(
            fixture.exists(path),
            "{LOCKFILE_NAME} records a hash for {path}, which the build did not write"
        );
        assert_eq!(
            recorded,
            &sha256_hex(fixture.read(path).as_bytes()),
            "the recorded hash for {path} is not the hash of its committed bytes"
        );
    }
}

/// **A comment-only edit to a provider file moves the lockfile, deliberately** — the story's own
/// note.
///
/// `toml_sha256` hashes the file's bytes, comments included, because a comment can change what an
/// operator believes the connector does. The generated artifacts do not move, and the lockfile has to
/// say both things at once: the input was edited, the output was not.
#[test]
fn a_comment_only_provider_edit_moves_the_lockfile_and_nothing_else() {
    let fixture = Fixture::with_provider("lock-comment", "zendesk");
    let root = fixture.root().to_str().unwrap().to_string();
    run(&["build", "--root", &root]).expect("the first build succeeds");

    let before = lockfile_of(&fixture)
        .entry("zendesk")
        .cloned()
        .expect("the first build recorded the provider");
    let module_before = fixture.read("connectors/zendesk.flux");

    fixture.write_provider(
        "zendesk",
        &format!(
            "# a note for the next reader\n{}",
            common::definition("zendesk")
        ),
    );
    run(&["build", "--root", &root]).expect("the second build succeeds");

    let after = lockfile_of(&fixture)
        .entry("zendesk")
        .cloned()
        .expect("the second build recorded the provider");

    assert_ne!(
        before.toml_sha256, after.toml_sha256,
        "the provider file's bytes changed, so its hash must have"
    );
    assert_eq!(
        before.ir_sha256, after.ir_sha256,
        "a comment compiles to nothing, so the IR hash must not move"
    );
    assert_eq!(
        before.artifacts, after.artifacts,
        "no generated byte moved, so no artifact hash may"
    );
    assert_eq!(
        module_before,
        fixture.read("connectors/zendesk.flux"),
        "and the module itself is untouched"
    );
}

/// **Every vendored document a connector compiles gets its own row** — C-410's shape, carried
/// through to the record `check` reads.
///
/// A connector built from several documents has no single `spec_sha256`: filling one from the first
/// document would record one document's provenance as the whole connector's, so the scalar is `None`
/// and the per-document list is where the hashes live. Without this, the first connector to compile
/// from more than one document would be recorded with **no spec hash at all** — a row that looks
/// complete and detects nothing.
#[test]
fn a_connector_compiled_from_several_documents_records_a_hash_for_each() {
    const MANAGER: &str = r#"{
  "openapi": "3.0.3",
  "info": { "title": "Acme Manager", "version": "0.0.0-dev" },
  "servers": [{ "url": "https://services.acme.example" }],
  "paths": {
    "/api/v2/users/{user_id}": {
      "get": {
        "operationId": "getUser",
        "summary": "Fetch one managed user.",
        "parameters": [
          { "name": "user_id", "in": "path", "required": true, "schema": { "type": "string" } }
        ]
      }
    }
  }
}
"#;
    const USER: &str = r#"{
  "openapi": "3.0.3",
  "info": { "title": "Acme User", "version": "0.0.0-dev" },
  "servers": [{ "url": "https://services.acme.example" }],
  "paths": {
    "/api/v3/me": { "get": { "operationId": "getUser", "summary": "Fetch the calling user." } }
  }
}
"#;

    let manager_path = "specs/acme/manager-2026-07-10.json";
    let user_path = "specs/acme/user-2026-06-25.json";
    let definition = format!(
        r#"id = "acme"
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
path = "{manager_path}"
service = "manager"
sha256 = "{}"

[[spec]]
path = "{user_path}"
service = "user"
sha256 = "{}"

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
"#,
        sha256_hex(MANAGER.as_bytes()),
        sha256_hex(USER.as_bytes()),
    );

    let fixture = Fixture::new("lock-many-documents");
    fixture.write_provider("acme", &definition);
    fixture.write(manager_path, MANAGER);
    fixture.write(user_path, USER);

    run(&["build", "--root", fixture.root().to_str().unwrap()]).expect("the build succeeds");

    let entry = lockfile_of(&fixture)
        .entry("acme")
        .cloned()
        .expect("the lockfile records the provider");

    assert_eq!(
        entry.spec_sha256, None,
        "no single hash is true of a connector built from two documents"
    );

    let recorded: Vec<(&str, Option<&str>)> = entry
        .specs
        .iter()
        .map(|spec| (spec.path.as_str(), spec.sha256.as_deref()))
        .collect();
    assert_eq!(
        recorded,
        vec![
            (manager_path, Some(sha256_hex(MANAGER.as_bytes()).as_str())),
            (user_path, Some(sha256_hex(USER.as_bytes()).as_str())),
        ],
        "each vendored document must be recorded with its own hash, so `check` can name the one \
         that moved"
    );
    assert_eq!(
        entry.specs.iter().map(|s| s.service()).collect::<Vec<_>>(),
        ["manager", "user"],
        "and with the service it joins, because an operation id is only unambiguous inside one \
         document's service"
    );
}

// ---------------------------------------------------------------------------------------------
// It is a checked artifact
// ---------------------------------------------------------------------------------------------

/// A hand-edited lockfile is drift, and `diff` says so rather than silently regenerating it.
#[test]
fn diff_reports_a_stale_lockfile_and_build_repairs_it() {
    let fixture = Fixture::with_provider("lock-stale", "zendesk");
    let root = fixture.root().to_str().unwrap().to_string();
    run(&["build", "--root", &root]).expect("the build succeeds");

    // Flip the recorded IR hash to a value no input produces — the shape a hand-edit or a bad merge
    // leaves behind. Targeted at a value read out of the file rather than at a literal, so the
    // tamper cannot silently become a no-op when a hash changes.
    let current = fixture.read(LOCKFILE_NAME);
    let recorded = lockfile_of(&fixture)
        .entry("zendesk")
        .expect("the build recorded the provider")
        .ir_sha256
        .clone();
    fixture.write(LOCKFILE_NAME, &current.replace(&recorded, &"f".repeat(64)));
    assert_ne!(
        current,
        fixture.read(LOCKFILE_NAME),
        "the fixture edit must change the file"
    );

    let output = run(&["diff", "--root", &root]).expect("diff succeeds");
    assert!(
        output.contains(LOCKFILE_NAME),
        "diff must name the stale lockfile, got:\n{output}"
    );
    assert!(
        output.contains("would change"),
        "diff must report the tree as not up to date, got:\n{output}"
    );

    run(&["build", "--root", &root]).expect("the repairing build succeeds");
    assert_eq!(
        current,
        fixture.read(LOCKFILE_NAME),
        "a build must restore the lockfile the inputs imply"
    );
}

// ---------------------------------------------------------------------------------------------
// Whole-catalogue: a scoped run must not truncate it
// ---------------------------------------------------------------------------------------------

/// **A `--provider` run leaves the lockfile alone.** It compiled one provider, so a lockfile written
/// from it would drop every other row — successfully, and with the file still looking well formed.
/// That is precisely the failure the whole-catalogue rule exists to prevent (`AGENTS.md`).
#[test]
fn a_scoped_build_leaves_the_lockfile_byte_identical() {
    let fixture = Fixture::with_provider("lock-scoped", "acme");
    for provider in ["beacon", "cinder"] {
        fixture.write_provider(provider, &common::definition(provider));
    }
    let root = fixture.root().to_str().unwrap().to_string();

    run(&["build", "--root", &root]).expect("the full build succeeds");
    let before = fixture.read(LOCKFILE_NAME);
    assert_eq!(
        lockfile_of(&fixture).entries().len(),
        3,
        "the full build must record all three providers"
    );

    run(&["build", "--provider", "acme", "--root", &root]).expect("the scoped build succeeds");
    assert_eq!(
        before,
        fixture.read(LOCKFILE_NAME),
        "`build --provider acme` rewrote the lockfile from one provider's data"
    );

    run(&["build", "--service", "default", "--root", &root]).expect("the scoped build succeeds");
    assert_eq!(
        before,
        fixture.read(LOCKFILE_NAME),
        "`build --service default` rewrote the lockfile from a narrowed connector"
    );
}
