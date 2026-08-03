//! The two seam functions, exercised end-to-end through `flux-connectors build` (C-27).
//!
//! `build_and_diff.rs` pins the *orchestration* — discovery, atomic writes, the byte-identical
//! no-op. This file pins the thing orchestration was built around: that
//! [`connector_cli::seam::load`] is `connector-spec`'s real provider loader and
//! [`connector_cli::seam::emit`] is `connector-flux`'s real Flux emitter, rather than the
//! placeholders C-13 stood in for them.
//!
//! Both are asserted through the artifacts a build writes, because that is the only thing a
//! consumer of this repo ever sees.
//!
//! # What is *not* asserted here, and why
//!
//! The story asks that a build produce artifacts that pass a parse-and-analyze check.
//! `flux_lang::program::Module::parse_str` is the natural way to say that, and it is **not
//! reachable from this crate**: `connector-cli` does not depend on `flux-lang`, and
//! `connector-flux` re-exports none of it, so asserting it directly would need a manifest edit
//! this story is fenced from making. `connector-flux`'s own
//! `emitted_text_is_a_fixed_point_of_the_flux_formatter` already proves every emitted `op` parses
//! and is canonical; what is left uncovered is the module *envelope* this file's
//! [`the_module_envelope_is_flux_comment_syntax`] pins by shape, and C-11's gate covers for real.

mod common;

use common::Fixture;

/// A complete hand-authored connector: one GET with a path parameter and an optional query
/// parameter — the shape C-8's emitter covers.
///
/// The operation id is kebab, not dotted: a dotted name cannot be a Flux `op` **declaration**
/// name, and the emitter refuses one rather than rewriting it (C-23 decides the public form).
const HAND_AUTHORED: &str = r#"
id = "acme"
vendor = "Acme"
base_url = "https://api.acme.example"
description = "The Acme support API."

[[operations]]
id = "acme-ticket-show"
method = "GET"
path = "/v2/tickets/{ticket_id}"
description = "Fetch one Acme ticket."
risk = "low"
idempotency = "idempotent"

[[operations.params.path]]
name = "ticket_id"
description = "The ticket to fetch."
required = true
schema = { type = "integer" }

[[operations.params.query]]
name = "include"
description = "Sideload related records."
required = false
schema = { type = "string" }
"#;

fn build(root: &str) -> anyhow::Result<String> {
    let invocation =
        connector_cli::cli::parse(["build", "--root", root].iter().map(|a| a.to_string()))?;
    let mut out = Vec::new();
    connector_cli::run(&invocation, &mut out)?;
    Ok(String::from_utf8(out).expect("CLI output is UTF-8"))
}

fn built_module(label: &str) -> String {
    let fixture = Fixture::new(label);
    fixture.write_provider("acme", HAND_AUTHORED);
    build(fixture.root().to_str().unwrap()).expect("build succeeds");
    fixture.read("connectors/acme.flux")
}

/// The emitter is wired: the declared operation reaches the module as a real `op` declaration,
/// carrying the metadata flux's approval gate reads and the `http.request` call it performs.
#[test]
fn build_emits_the_operations_a_provider_declares() {
    let module = built_module("wiring-module");

    for expected in [
        "op acme-ticket-show(ticket_id: Number, include: String) -> Any",
        r#"description "Fetch one Acme ticket.""#,
        r#"risk "low""#,
        r#"idempotency "idempotent""#,
        "expose true",
        r#"base = "https://api.acme.example""#,
        r#"url = fmt("{base}/v2/tickets/{ticket_id}")"#,
        // `$include` keeps its sigil where a bare record value would lose it: `include` is a Flux
        // keyword. C-30 keeps the value structural instead of guarding URL interpolation.
        "query: { include: $include }",
        r#"response = http.request(method: "GET", query: { include: $include }, url)"#,
        "return response",
    ] {
        assert!(
            module.contains(expected),
            "the generated module is missing `{expected}`:\n{module}"
        );
    }
}

/// Everything the module carries that is *not* an `op` must be Flux comment syntax, or the header
/// makes the artifact unparseable. Flux comments are `#`; `//` is not a comment in Flux and was
/// what the C-13 placeholder emitted.
#[test]
fn the_module_envelope_is_flux_comment_syntax() {
    let module = built_module("wiring-envelope");

    for (number, line) in module.lines().enumerate() {
        assert!(
            !line.trim_start().starts_with("//"),
            "line {} of the generated module is a `//` comment, which Flux does not parse:\n{line}",
            number + 1
        );
    }
    assert!(
        module.starts_with('#'),
        "the module should open with a generated-file header comment:\n{module}"
    );
}

/// The loader is wired: `connector-spec` validates, so a definition that parses as TOML but is not
/// a valid connector fails the build with the loader's own diagnosis. The placeholder loader
/// accepted anything that was not empty.
#[test]
fn build_rejects_a_definition_the_real_loader_finds_invalid() {
    let fixture = Fixture::new("wiring-invalid");
    // Well-formed TOML, an `id`, and nothing else: no `base_url`, and no operations to compile.
    fixture.write_provider("acme", "id = \"acme\"\n");

    let error = build(fixture.root().to_str().unwrap())
        .expect_err("an incomplete provider definition must not build");

    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("base_url"),
        "the loader's own diagnosis should reach the user, got: {rendered}"
    );
    assert!(
        !fixture.exists("connectors/acme.flux"),
        "a failed load wrote an artifact anyway"
    );
}

/// The manifest is derived from the loaded IR rather than from the bytes, so what a connector
/// declares — not merely that its file changed — is what reaches it.
#[test]
fn the_manifest_records_what_the_connector_declares() {
    let fixture = Fixture::new("wiring-manifest");
    fixture.write_provider("acme", HAND_AUTHORED);
    build(fixture.root().to_str().unwrap()).expect("build succeeds");

    let manifest = fixture.read("connectors/acme.connector.toml");
    for expected in [
        r#"connector = "acme""#,
        r#"vendor = "Acme""#,
        r#"base_url = "https://api.acme.example""#,
        r#""acme-ticket-show""#,
    ] {
        assert!(
            manifest.contains(expected),
            "the generated manifest is missing `{expected}`:\n{manifest}"
        );
    }
}
