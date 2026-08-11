//! `providers/github.toml` exists, emits analyzable Flux, and lets no caller-supplied query value
//! change the structure of a request (C-52, C-469, C-30, C-527).
//!
//! **The premise of this file changed, and the assertions changed with it.** It used to open with
//! *"nothing in this pipeline percent-encodes a query value: the emitter interpolates it verbatim"*,
//! and enforced the only rule that was safe under it — every query parameter is an integer, on four
//! frozen operation ids. C-30 landed Flux 0.54's structured `http.request(query: …)` map, and that
//! sentence stopped being true: a scalar value now travels as a record field encoded with RFC 3986
//! semantics, and the URL carries path data only. Verified rather than assumed — see
//! [`no_github_query_value_reaches_the_url`], which asserts it on the emitted text.
//!
//! So "integers only" is retired, because it was a **proxy** for the property that actually matters
//! and that proxy now excludes safe parameters while proving nothing extra. The two rules below are
//! what it was standing in for, and both are strictly stronger than what it checked:
//!
//! 1. **Every query parameter is a scalar.** An array or object has no declared wire shape, C-30
//!    refuses it with `UnencodableQueryValue`, and this asserts the connector never declares one.
//! 2. **No query value reaches the URL.** This is the injection vector itself, and it is now checked
//!    on every operation rather than on four exempted ids.
//!
//! This file names only GitHub; it never walks the catalogue, so another provider cannot change the
//! premise of a GitHub-specific assertion.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::Connector;

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

const PAGINATED_READS: [&str; 4] = [
    "github-issue-list",
    "github-pull-files-list",
    "github-workflow-run-list",
    "github-commit-list",
];

/// `<repo root>/providers/github.toml`, derived from this crate's manifest directory so the test is
/// independent of the working directory a runner happens to use.
fn provider_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
        .join("github.toml")
}

fn github() -> Connector {
    let path = provider_path();
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-52 ships the GitHub connector",
            path.display()
        )
    });
    shipped_provider::load_definition("github", &source)
        .expect("providers/github.toml does not load")
        .connector
}

/// The connector exists, loads through the real loader, and is the one C-52 describes.
#[test]
fn the_github_connector_loads() {
    let connector = github();

    assert_eq!(connector.id, "github");
    assert_eq!(connector.vendor, "GitHub");
    assert_eq!(
        connector.base_url, "https://api.github.com",
        "the host is `api.github.com` and is never widened"
    );
    assert!(
        !connector.operations.is_empty(),
        "a connector with no operations compiles to an empty module"
    );
}

/// Every operation emits Flux that parses, is canonical under flux's own formatter, and loads as
/// exactly one composite op — the C-11 gate, held against github specifically.
#[test]
fn every_github_operation_emits_an_analyzable_module() {
    let connector = github();
    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation).unwrap_or_else(|error| {
            panic!("operation `{}` is not emittable: {error}", operation.id)
        });

        let parsed = flux_lang::parser::parse_cst(&emitted);
        assert!(
            parsed.errors.is_empty(),
            "`{}` emits Flux that does not parse: {:?}\n{emitted}",
            operation.id,
            parsed.errors
        );
        assert_eq!(
            flux_lang::format_cst::format_module(&parsed).as_deref(),
            Some(emitted.as_str()),
            "the flux formatter would rewrite `{}`",
            operation.id
        );

        let module = flux_lang::program::Module::parse_str(&emitted)
            .unwrap_or_else(|error| panic!("`{}` does not load: {error}", operation.id));
        let program = module
            .program()
            .unwrap_or_else(|| panic!("`{}` is not a program", operation.id));
        assert_eq!(
            program.ops.len(),
            1,
            "one operation is one declaration; `{}` loaded {}",
            operation.id,
            program.ops.len()
        );
        assert_eq!(program.ops[0].name, operation.id);
    }
}

/// **The honesty assertion.** Every declared query parameter is a scalar.
///
/// A scalar is what C-30's structured `query` map can encode; an array or object has no declared
/// wire shape, and rather than guess a vendor's convention the emitter refuses one with
/// `UnencodableQueryValue`. Asserting it here means a widening that would only fail at emission
/// time fails at the connector's own contract instead, naming the parameter.
///
/// The four pre-C-30 reads are still checked more tightly than the rest — they are published bytes,
/// and `PAGINATED_READS` is what keeps a "while I'm here" widening of one of them visible.
#[test]
fn github_query_parameters_are_scalars() {
    const SCALARS: [&str; 4] = ["string", "integer", "number", "boolean"];
    let connector = github();
    for operation in &connector.operations {
        let names: Vec<&str> = operation
            .params
            .query
            .iter()
            .map(|param| {
                let declared = param.schema["type"].as_str().unwrap_or_else(|| {
                    panic!(
                        "`{}.{}` declares no query `type`, so nothing can say it is encodable",
                        operation.id, param.name
                    )
                });
                assert!(
                    SCALARS.contains(&declared),
                    "`{}.{}` is a {declared}, which has no declared query wire shape (C-30)",
                    operation.id,
                    param.name
                );
                param.name.as_str()
            })
            .collect();
        if PAGINATED_READS.contains(&operation.id.as_str()) {
            assert_eq!(names, ["per_page", "page"], "{} widened", operation.id);
        }
    }
}

/// **The injection assertion, and the one that replaced "integers only".**
///
/// No query value reaches the URL on any operation — the emitted `url` binding carries path data
/// and nothing else, so a caller's value cannot introduce a `?`, a `&` or a second parameter no
/// matter what it contains. Every declared query parameter appears instead in the structured
/// `query: { … }` record, which is where C-30's RFC 3986 encoding is applied.
///
/// This is checked on **every** GitHub operation rather than on four exempted ids, which is what
/// makes it stronger than the rule it replaced.
#[test]
fn no_github_query_value_reaches_the_url() {
    let connector = github();
    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation).expect("a shipped operation emits");

        let url_lines: Vec<&str> = emitted
            .lines()
            .map(str::trim_start)
            .filter(|line| line.starts_with("url = "))
            .collect();
        assert_eq!(
            url_lines.len(),
            1,
            "`{}` must bind exactly one $url:\n{emitted}",
            operation.id
        );
        assert!(
            !url_lines[0].contains('?') && !url_lines[0].contains('&'),
            "`{}` puts query data in the URL, which is the injection vector C-30 closed:\n{emitted}",
            operation.id
        );

        if operation.params.query.is_empty() {
            assert!(
                !emitted.contains("query: {"),
                "`{}` emits a query record it declares no parameters for:\n{emitted}",
                operation.id
            );
            continue;
        }

        // The emitter sorts the record's fields, so the expectation is built sorted too rather than
        // in declaration order — a mismatch here would otherwise read as a missing parameter.
        let mut declared: Vec<&str> = operation
            .params
            .query
            .iter()
            .map(|param| param.name.as_str())
            .collect();
        declared.sort_unstable();
        let expected = format!("query: {{ {} }}", declared.join(", "));
        assert!(
            emitted.contains(&expected),
            "`{}` does not carry every declared parameter structurally; expected `{expected}`:\n{emitted}",
            operation.id
        );
    }
}
