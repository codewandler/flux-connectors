//! `providers/github.toml` exists, emits analyzable Flux, and exposes only integer pagination in
//! query strings (C-52, C-469).
//!
//! Nothing in this pipeline percent-encodes a query value: the emitter interpolates it verbatim.
//! C-469 therefore widens C-52's zero-query rule only for `per_page` and `page`, whose integer
//! schemas cannot carry `&`, `#` or another query pair. This test closes that exception over the
//! four frozen operation ids; every existing operation remains query-free and every string, array
//! or boolean filter from GitHub's document remains omitted.
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

/// **The honesty assertion.** Only the four reviewed collection reads carry query parameters, and
/// their whole surface is the two integer pagination fields.
#[test]
fn github_query_parameters_are_closed_over_integer_pagination() {
    let connector = github();
    for operation in &connector.operations {
        let names: Vec<&str> = operation
            .params
            .query
            .iter()
            .map(|param| {
                assert_eq!(
                    param.schema["type"],
                    serde_json::json!("integer"),
                    "`{}.{}` is not injection-safe integer pagination",
                    operation.id,
                    param.name
                );
                param.name.as_str()
            })
            .collect();
        if PAGINATED_READS.contains(&operation.id.as_str()) {
            assert_eq!(names, ["per_page", "page"], "{} widened", operation.id);
        } else {
            assert!(
                names.is_empty(),
                "the existing operation `{}` gained a query surface",
                operation.id
            );
        }
    }
}

/// The emitted request agrees: the reviewed reads carry exactly `page` and `per_page` as structured
/// values, and every other GitHub operation carries no query record.
#[test]
fn github_emits_only_the_reviewed_pagination_query() {
    let connector = github();
    for operation in &connector.operations {
        let emitted = emit_operation(&connector, operation).expect("a shipped operation emits");

        let url_lines: Vec<&str> = emitted
            .lines()
            .map(str::trim_start)
            .filter(|line| line.starts_with("url = "))
            .collect();
        assert!(
            !url_lines.is_empty(),
            "`{}` binds no $url:\n{emitted}",
            operation.id
        );
        if PAGINATED_READS.contains(&operation.id.as_str()) {
            assert_eq!(
                url_lines.len(),
                1,
                "{} query data entered the URL",
                operation.id
            );
            assert!(
                emitted.contains("query: { page, per_page }"),
                "{} emits a query other than per_page/page:\n{emitted}",
                operation.id,
            );
        } else {
            assert_eq!(url_lines.len(), 1, "{} gained a query", operation.id);
            assert!(!emitted.contains("query: {"));
        }
    }
}
