//! `providers/github.toml` exists, emits analyzable Flux, and **declares no query parameter at all**
//! (C-52).
//!
//! The third claim is the one this file exists for, and it is a safety assertion rather than a style
//! one. Nothing in this pipeline percent-encodes a query value: the emitter interpolates it verbatim
//! into a `fmt` template (`crates/connector-flux/src/op.rs`, *"Query values are interpolated
//! verbatim"*), and flux registers no URL-encoding op. `zendesk-ticket-search` is the standing
//! demonstration — AGENTS.md lists it under *Intentional gaps* as non-functional for exactly this
//! reason, where a value carrying `&` injects a parameter rather than filtering by one.
//!
//! GitHub's most-wanted operations are precisely the ones that would trip it: `GET /issues` filters
//! on `state`, `labels` and `assignee`, and the search endpoints take a free-form `q`. So C-52 ships
//! the path-and-body surface only, and this file is what stops a later well-meaning edit from adding
//! `?state=open` back before C-30 lands. It asserts the strong form — **zero** query parameters, not
//! merely zero string-ish ones — because that is a property a reader can check at a glance and a
//! future author cannot satisfy by picking a narrower type for an injectable value.
//!
//! The two structural claims deliberately duplicate what `shipped_modules.rs` asserts across every
//! provider. That file iterates whatever `providers/` holds (C-54, which replaced the hand-listed set
//! it used to share with the sibling connector stories); this one names only github, so C-52's gate
//! stays a claim about github rather than one whose subject moves when the shipped set does.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{provider, Connector};

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
    provider::load("providers/github.toml", &source)
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

/// **The honesty assertion.** No github operation declares a query parameter, so nothing this
/// connector emits can carry an unencoded value into a query string. See the module documentation
/// for why this is the strong form rather than a check on the parameter's type.
#[test]
fn no_github_operation_declares_a_query_parameter() {
    let connector = github();
    for operation in &connector.operations {
        assert!(
            operation.params.query.is_empty(),
            "operation `{}` declares query parameters {:?}. Nothing percent-encodes a query value \
             (C-30 is unimplemented), so a value carrying `&` or `#` corrupts the request or injects \
             a parameter — the `zendesk-ticket-search` failure AGENTS.md records. C-52 ships the \
             path-and-body surface only; if C-30 has landed, change this test deliberately",
            operation.id,
            operation
                .params
                .query
                .iter()
                .map(|param| param.name.as_str())
                .collect::<Vec<_>>()
        );
    }
}

/// The generated request URL carries no query string either — the assertion above stated against the
/// emitted text rather than the IR, so a future emitter that synthesised a query parameter from
/// somewhere other than `params.query` could not slip past.
///
/// **Every `url = ` line is checked, not just the first, and that is the whole substance of this
/// test.** The emitter binds `$url` once for the path and required query parameters, then re-binds it
/// once more per *optional* query parameter inside a `when` guard
/// (`crates/connector-flux/src/op.rs`, the `optional` loop) — `connectors/zendesk.flux` shows the
/// shape:
///
/// ```flux
/// url = fmt("{base}/api/v2/tickets/{ticket_id}/comments.json")
/// sep = "?"
/// when $page
///   url = fmt("{url}{sep}page={page}")
/// ```
///
/// The `?` lives on `sep`, and the parameter name lives on a *later* `$url` line than the first. So
/// inspecting only the first binding would pass while an operation quietly appended optional filters,
/// which is exactly the regression this file exists to prevent.
#[test]
fn no_github_operation_emits_a_query_string() {
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
        // One binding and one only: a second is the emitter appending an optional query parameter.
        assert_eq!(
            url_lines.len(),
            1,
            "`{}` re-binds $url {} times; the emitter does that once per optional query parameter, \
             so this operation is appending a query string:\n{emitted}",
            operation.id,
            url_lines.len()
        );
        for line in &url_lines {
            assert!(
                !line.contains('?'),
                "`{}` emits a query string: {line}",
                operation.id
            );
        }
        // `sep` exists only to carry the `?`/`&` between optional query parameters, so an operation
        // that binds it is building a query string even if no single line spells the `?`.
        assert!(
            !emitted
                .lines()
                .any(|line| line.trim_start().starts_with("sep = ")),
            "`{}` binds `sep`, which the emitter emits only to separate query parameters:\n{emitted}",
            operation.id
        );
    }
}
