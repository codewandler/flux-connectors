//! `flux-connectors scaffold` — the helper that writes the patch set (C-419).
//!
//! Every test here runs against the **real repository**, not a fixture, because the thing under
//! test is whether a real vendored document produces provider TOML a human can paste. A synthetic
//! three-operation document would prove the emitter is syntactically well-formed and nothing about
//! the property that matters: that 356 operations of a document nobody wrote by hand come out as a
//! file the loader accepts.
//!
//! # The two rules that are one rule
//!
//! [`scaffolding_a_mixed_get_set_carries_exact_reviewed_direction_and_loads`] is the round trip:
//! generated text in, a compiled connector out, no hand-editing in between.
//! [`a_mutating_selector_emits_a_hole_the_loader_refuses`] is its inverse and is the more important
//! of the two — a scaffold that silently declared 54 DELETEs `low` would pass the first test and
//! would have manufactured 54 unreviewed safety claims.

use std::path::{Path, PathBuf};

use crate::common::Fixture;

/// A document with one read and one write and **nothing that has ever been reviewed** — no provider
/// file, so no `risk` exists anywhere for the helper to carry.
const UNCLAIMED_DOCUMENT: &str = r#"openapi: "3.0.3"
info:
  title: The Acme widget API
  version: "1.4.0"
servers:
  - url: https://api.acme.example
paths:
  /v1/widgets:
    get:
      operationId: listWidgets
      summary: List every widget.
      responses:
        "200":
          description: ok
          content:
            application/json:
              schema:
                type: object
                properties:
                  total: { type: integer }
  /v1/widgets/{widgetId}:
    delete:
      operationId: deleteWidget
      summary: Destroy one widget, permanently.
      parameters:
        - name: widgetId
          in: path
          required: true
          schema: { type: string }
      responses:
        "200":
          description: ok
          content:
            application/json:
              schema:
                type: object
                properties:
                  deleted: { type: boolean }
"#;

/// Two transport-adversarial operations whose direction has already been reviewed: a mutating GET
/// and a read-only POST. Scaffold must carry those facts, not reverse them back to method classes.
const REVIEWED_DOCUMENT: &str = r#"openapi: "3.0.3"
info:
  title: The Acme widget API
  version: "1.4.0"
servers:
  - url: https://api.acme.example
paths:
  /v1/widgets/flush:
    get:
      operationId: flushWidgets
      summary: Flush queued widget work.
      responses:
        "200": { description: ok }
  /v1/widgets/lookup:
    post:
      operationId: lookupWidgets
      summary: Look up widgets without changing them.
      responses:
        "200": { description: ok }
"#;

fn reviewed_provider() -> String {
    format!(
        r#"id = "acme"
vendor = "Acme"
base_url = "https://api.acme.example"

[[spec]]
path = "specs/acme/v1.yaml"
sha256 = "{}"

[patch.naming]
rule = "kebab"

[[patch.operations]]
select = "flushWidgets"
direction = "write"
risk = "high"
idempotency = "non_idempotent"

[[patch.operations]]
select = "lookupWidgets"
direction = "read"
risk = "low"
idempotency = "idempotent"
"#,
        connector_spec::sha256_hex(REVIEWED_DOCUMENT.as_bytes())
    )
}

/// The repository root, derived from this crate's manifest directory so the test is independent of
/// the working directory a runner happens to use.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root exists")
}

/// Run `flux-connectors scaffold` against the real repository and capture stdout.
fn scaffold(args: &[&str]) -> anyhow::Result<String> {
    scaffold_in(&root(), args)
}

/// The same, rooted anywhere — the fixture tests point it at a tree holding a document and no
/// provider file at all, which is the case that has nothing to carry forward.
fn scaffold_in(root: &Path, args: &[&str]) -> anyhow::Result<String> {
    let mut argv: Vec<String> = vec!["scaffold".to_owned()];
    argv.extend(args.iter().map(|arg| (*arg).to_owned()));
    argv.push("--root".to_owned());
    argv.push(root.to_string_lossy().into_owned());

    let invocation = connector_cli::cli::parse(argv)?;
    let mut out = Vec::new();
    connector_cli::run(&invocation, &mut out)?;
    Ok(String::from_utf8(out).expect("scaffold output is UTF-8"))
}

/// The vendored documents for one provider, as the spec cache holds them.
fn documents(provider: &str) -> Vec<(String, String)> {
    let dir = root().join("specs").join(provider);
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()))
        .map(|entry| entry.expect("readable directory entry").path())
        .filter(|path| path.is_file())
        .collect();
    entries.sort();
    entries
        .into_iter()
        .map(|path| {
            let name = path.file_name().expect("a file name").to_string_lossy();
            (
                format!("specs/{provider}/{name}"),
                std::fs::read_to_string(&path).expect("readable document"),
            )
        })
        .collect()
}

/// Compile scaffolded TOML exactly as a build would, with no hand-editing in between.
///
/// `load_with_spec` rather than the `provider::load` the story names: C-421 landed after this story
/// was written and made the pure entry point *refuse* a file that pins a `[spec]`, so a spec-backed
/// file can only be compiled through the cache-taking one. This is the same call
/// `connector_cli::seam::load` makes for every shipped provider.
fn load(provider: &str, toml: &str) -> connector_spec::Result<connector_spec::LoadedProvider> {
    let owned = documents(provider);
    let cache: Vec<connector_spec::SpecDocument<'_>> = owned
        .iter()
        .map(|(path, document)| connector_spec::SpecDocument {
            path,
            document: document.as_str(),
        })
        .collect();
    connector_spec::provider::load_with_spec(&format!("scaffolded/{provider}.toml"), toml, &cache)
}

/// Acceptance: "`connector-cli scaffold <provider>` reads the vendored document(s) and writes
/// provider TOML to **stdout** … A failing-first test scaffolds the babelforce manager document and
/// asserts the output loads through `provider::load` without hand-editing."
///
/// The reads of the manager document, because they are the half of it that carries no safety claim:
/// a scaffold of the writes is *supposed* not to load until a human has stated `risk`, which is
/// [`a_mutating_selector_emits_a_hole_the_loader_refuses`]'s subject.
#[test]
fn scaffolding_a_mixed_get_set_carries_exact_reviewed_direction_and_loads() {
    let toml = scaffold(&["babelforce", "--select", "manager:/api/v2:GET"])
        .expect("scaffold runs against the vendored babelforce documents");

    let loaded = load("babelforce", &toml).unwrap_or_else(|error| {
        panic!("scaffolded TOML does not load:\n{error}\n\n--- emitted ---\n{toml}")
    });

    assert!(
        loaded.connector.operations.len() > 100,
        "scaffolding the manager document's reads published {} operations; the document declares \
         well over a hundred GETs under `/api/v2`, so this is a selector that silently matched \
         almost nothing",
        loaded.connector.operations.len()
    );
    assert!(
        loaded
            .connector
            .operations
            .iter()
            .all(|operation| operation.method == connector_spec::HttpMethod::Get),
        "a `GET`-only selector published a non-GET operation"
    );
    let flush = loaded
        .connector
        .operation("babelforce-flush-dialer")
        .expect("the reviewed mutating GET remains selected");
    assert_eq!(flush.direction, connector_spec::OperationDirection::Write);
    assert_eq!(flush.method, connector_spec::HttpMethod::Get);
}

/// Acceptance: "**Everything the document cannot state is emitted as a hole, not a guess.** `risk`
/// and `idempotency` come out as an explicit `TODO` the loader refuses (C-414) rather than a
/// plausible default."
///
/// **The purest form of the rule**, because there is nothing here to carry forward: a document with
/// a `DELETE` in it and no `providers/acme.toml` at all. Every claim a scaffold could make about
/// this operation would be one it invented, so it must make none.
///
/// This is the test that has to fail if the helper ever becomes convenient. A `DELETE` that came out
/// `risk = "low"` would load, and every other assertion in this file would still pass.
#[test]
fn a_document_nobody_has_claimed_emits_a_hole_the_loader_refuses() {
    let fixture = Fixture::new("scaffold-hole");
    fixture.write("specs/acme/v1.yaml", UNCLAIMED_DOCUMENT);

    let toml = scaffold_in(fixture.root(), &["acme"]).expect("scaffold runs with no provider file");

    assert!(
        toml.contains("TODO(direction)"),
        "a selector over unclaimed operations emitted no direction `TODO`:\n{toml}"
    );
    // Declarations only: the `TODO` above deliberately quotes the menu of legal values, and a
    // comment is not a claim.
    let claims: Vec<&str> = toml
        .lines()
        .map(str::trim)
        .filter(|line| {
            line.starts_with("direction = ")
                || line.starts_with("risk = ")
                || line.starts_with("idempotency = ")
        })
        .collect();
    assert!(
        claims.is_empty(),
        "the scaffold stated {claims:?} for operations nobody has claimed. No OpenAPI document \
         publishes either field, so every one of those would be a safety claim this helper \
         invented:\n{toml}"
    );

    let cache = [connector_spec::SpecDocument {
        path: "specs/acme/v1.yaml",
        document: UNCLAIMED_DOCUMENT,
    }];
    let error = connector_spec::provider::load_with_spec("scaffolded/acme.toml", &toml, &cache)
        .expect_err("an unclaimed DELETE must not compile")
        .to_string();
    assert!(
        error.contains("direction"),
        "the refusal does not name the fields a human has to state: {error}"
    );
}

#[test]
fn scaffold_carries_reviewed_mutating_get_and_read_post_without_method_inference() {
    let fixture = Fixture::new("scaffold-reviewed-direction");
    fixture.write("specs/acme/v1.yaml", REVIEWED_DOCUMENT);
    fixture.write("providers/acme.toml", &reviewed_provider());

    let toml = scaffold_in(fixture.root(), &["acme"]).expect("scaffold reads reviewed truth");
    assert!(!toml.contains("TODO(direction"), "{toml}");

    let get_selector = toml
        .split("[[patch.select]]")
        .find(|selector| selector.contains("methods = [\"GET\","))
        .expect("a GET selector");
    assert!(!get_selector.contains("direction = "), "{get_selector}");

    let post_selector = toml
        .split("[[patch.select]]")
        .find(|selector| selector.contains("methods = [\"POST\","))
        .expect("a POST selector");
    assert!(!post_selector.contains("direction = "), "{post_selector}");
    let direction_map = toml
        .split("[patch.directions.default]")
        .nth(1)
        .and_then(|rest| rest.split("\n\n").next())
        .expect("a stable identity-keyed direction map");
    assert!(
        direction_map.contains("\"flushWidgets\" = \"write\""),
        "{direction_map}"
    );
    assert!(
        direction_map.contains("\"lookupWidgets\" = \"read\""),
        "{direction_map}"
    );

    let cache = [connector_spec::SpecDocument {
        path: "specs/acme/v1.yaml",
        document: REVIEWED_DOCUMENT,
    }];
    let loaded = connector_spec::provider::load_with_spec("scaffolded/acme.toml", &toml, &cache)
        .unwrap_or_else(|error| panic!("reviewed scaffold does not load: {error}\n{toml}"));
    let flush_id = loaded
        .connector
        .provenance
        .operation_specs
        .iter()
        .find(|(_, source)| source.operation_id == "flushWidgets")
        .map(|(id, _)| id)
        .expect("the stable flushWidgets identity remains published");
    let lookup_id = loaded
        .connector
        .provenance
        .operation_specs
        .iter()
        .find(|(_, source)| source.operation_id == "lookupWidgets")
        .map(|(id, _)| id)
        .expect("the stable lookupWidgets identity remains published");
    let flush = loaded.connector.operation(flush_id).unwrap();
    let lookup = loaded.connector.operation(lookup_id).unwrap();
    assert_eq!(flush.method, connector_spec::HttpMethod::Get);
    assert_eq!(flush.direction, connector_spec::OperationDirection::Write);
    assert_eq!(lookup.method, connector_spec::HttpMethod::Post);
    assert_eq!(lookup.direction, connector_spec::OperationDirection::Read);
}

/// The same rule where it is hardest to hold: **one** unclaimed operation among nine that a human
/// has already reviewed.
///
/// `task-automation` declares `POST /api/v1/webhook/zendesk`, a receiver Zendesk calls *into*
/// babelforce, which `providers/babelforce.toml` deliberately excludes. A selector over the whole
/// document's POSTs therefore reaches nine operations carrying a reviewed `high`/`non_idempotent`
/// and one carrying nothing at all — and the honest output is both: the nine keep their claim in
/// their own blocks, and the selector states nothing, so the loader refuses over the one gap rather
/// than over all ten.
#[test]
fn one_unclaimed_operation_makes_the_selector_a_hole_and_loses_no_reviewed_claim() {
    let toml = scaffold(&["babelforce", "--select", "task-automation::POST"])
        .expect("scaffold runs against the vendored babelforce documents");

    let selector = toml
        .split("[[patch.select]]")
        .nth(1)
        .and_then(|rest| rest.split("\n\n").next())
        .expect("the emitted file states a selector");
    assert!(
        !selector.contains("risk"),
        "the selector claimed a `risk` over a set holding an operation nobody has claimed:\n{selector}"
    );
    assert!(
        toml.contains("TODO(direction)"),
        "no `TODO` names the gap:\n{toml}"
    );
    assert_eq!(
        toml.matches("risk = \"high\"").count(),
        9,
        "the nine reviewed claims were not all carried into their own blocks:\n{toml}"
    );

    let error = load("babelforce", &toml)
        .expect_err("one unclaimed POST must not compile")
        .to_string();
    assert!(
        error.contains("risk") && error.contains("idempotency"),
        "the refusal does not name the fields a human has to state: {error}"
    );
}

/// Acceptance: "The output is deterministic and canonically formatted: scaffolding twice gives
/// byte-identical text."
#[test]
fn scaffolding_twice_is_byte_identical() {
    let first = scaffold(&["babelforce"]).expect("scaffold runs");
    let second = scaffold(&["babelforce"]).expect("scaffold runs");
    assert_eq!(
        first, second,
        "two scaffold runs over the same committed bytes disagreed"
    );
}

/// Acceptance: "It reports what it could not carry, per operation and by count — a body encoding
/// the IR cannot express, a parameter position that is dropped, an operation with no description."
///
/// The five `multipart/form-data` uploads are the measured case
/// (`providers/babelforce.toml:17-27`): ingest cannot express them, so they are absent from the
/// emitted selection, and a dropped operation that produces no output reads as "the vendor does not
/// offer that".
#[test]
fn it_reports_what_it_could_not_carry() {
    let toml = scaffold(&["babelforce"]).expect("scaffold runs");

    assert!(
        toml.contains("could not carry"),
        "the scaffold emitted no report of what it dropped:\n{toml}"
    );
    assert!(
        toml.contains("multipart/form-data"),
        "the five manager upload operations are skipped by ingest and were not reported:\n{toml}"
    );
    assert!(
        toml.contains("no description"),
        "operations the document leaves nameless were not reported:\n{toml}"
    );
}

/// Acceptance: "`--diff` compares the document against the connector as it stands and reports what
/// upstream added, removed or changed."
#[test]
fn diff_reports_the_document_against_the_connector_as_it_stands() {
    let report = scaffold(&["babelforce", "--diff"]).expect("scaffold --diff runs");

    for heading in ["added", "removed", "changed", "unchanged"] {
        assert!(
            report.contains(heading),
            "`--diff` reported no `{heading}` section:\n{report}"
        );
    }
    assert!(
        !report.contains("[[patch.select]]"),
        "`--diff` emitted provider TOML; it reports, it does not scaffold:\n{report}"
    );
}

/// `AGENTS.md`, owner-stated 2026-08-01: **"An authentication endpoint is never a connector
/// operation… ingest selecting them is a selection error rather than a coverage win."**
///
/// The same section rules out the workaround in advance — `expose = false` withholds the *tool* and
/// not the *call*, because `connector_pack::resolve` admits any named operation regardless of
/// exposure — so the test is that the operation is **absent from the selection**, not that it is
/// unexposed. babelforce's `auth` document is three such endpoints and nothing else, so a scaffold
/// that proposes it at all has proposed exactly what the rule forbids.
#[test]
fn an_authentication_endpoint_is_never_selected() {
    let toml = scaffold(&["babelforce"]).expect("scaffold runs");

    for endpoint in ["/oauth/token", "/oauth/authorize", "/oauth/revoke"] {
        assert!(
            !toml.contains(&format!("path_prefix = \"{endpoint}\"")),
            "a selector reaches {endpoint}, which is authentication-flow material:\n{toml}"
        );
    }
    for operation_id in ["\"token\"", "\"authorize\"", "\"revoke\""] {
        assert!(
            !toml.contains(&format!("select = {operation_id}")),
            "a `[[patch.operations]]` block selects {operation_id}, which is an authentication \
             endpoint:\n{toml}"
        );
    }
    // The whole `auth` document is authentication flow, so nothing should reference it — a
    // `[[spec]]` entry for a document no selector reaches would declare a surface the file
    // publishes nothing into.
    assert!(
        !toml.contains("auth-2026-06-25.openapi.yaml\""),
        "the emitted file pins the `auth` document, whose every operation is withheld:\n{toml}"
    );

    // Withheld is not the same as silently dropped. Every exclusion carries the rule that made it.
    assert!(
        toml.contains("an authentication endpoint is never a connector operation"),
        "the three withheld endpoints are not reported, so the exclusion reads as a coverage \
         gap:\n{toml}"
    );
}

/// "never over a file in place — the author diffs and pastes, so a bad run costs nothing".
#[test]
fn scaffold_writes_no_file() {
    let definition = root().join("providers/babelforce.toml");
    let before = std::fs::read(&definition).expect("the shipped definition is readable");
    let modified = std::fs::metadata(&definition)
        .and_then(|meta| meta.modified())
        .expect("a modification time");

    scaffold(&["babelforce"]).expect("scaffold runs");

    assert_eq!(
        before,
        std::fs::read(&definition).expect("readable"),
        "`scaffold` rewrote providers/babelforce.toml"
    );
    assert_eq!(
        modified,
        std::fs::metadata(&definition)
            .and_then(|meta| meta.modified())
            .expect("a modification time"),
        "`scaffold` touched providers/babelforce.toml"
    );
}
