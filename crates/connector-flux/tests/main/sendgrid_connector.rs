//! `providers/sendgrid.toml` exists, and **it does not send mail** (C-168).
//!
//! SendGrid's `POST /v3/mail/send` takes `personalizations: [{"to": [{"email": "…"}]}]` — an array
//! of objects containing a further array of objects. This file's central claim used to be that the
//! body-nesting mechanism *could not* place a value inside an array at any depth, so the
//! decomposition was unwritable rather than merely unwritten.
//!
//! **C-185 changed that half, and this file records the change rather than restating a claim that
//! has stopped being true.** A `wire` segment now takes a bracketed index, so the envelope is
//! expressible — [`the_excluded_envelope_shape_is_now_expressible`] builds SendGrid's exact shape
//! from a synthetic fixture, and [`a_bare_numeric_wire_segment_is_refused`] shows the spelling that
//! used to be silently wrong is now a refusal. What has *not* changed is that this connector still
//! ships four reads and no send; [`the_curated_operation_set_still_excludes_the_send`] carries the
//! two reasons that outlive C-185.
//!
//! Every test here except the two synthetic ones depends on the provider file loading, which is what
//! gives this file its failing-first shape.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{provider, Connector, HttpMethod};

/// `<repo root>/providers/sendgrid.toml`, derived from this crate's manifest directory so the test is
/// independent of the working directory a runner happens to use.
fn provider_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
        .join("sendgrid.toml")
}

fn sendgrid() -> Connector {
    let path = provider_path();
    let source = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}) — C-168 ships the SendGrid connector",
            path.display()
        )
    });
    provider::load("providers/sendgrid.toml", &source)
        .expect("providers/sendgrid.toml does not load")
        .connector
}

/// The connector exists, loads through the real loader, and is the one C-168 describes.
#[test]
fn the_sendgrid_connector_loads() {
    let connector = sendgrid();

    assert_eq!(connector.id, "sendgrid");
    assert_eq!(connector.vendor, "SendGrid");
    assert_eq!(
        connector.base_url, "https://api.sendgrid.com",
        "one tenant-independent host, never widened"
    );
    assert!(
        !connector.operations.is_empty(),
        "a connector with no operations compiles to an empty module"
    );
}

/// **The curated set is exactly the reads.** Named rather than counted, so adding
/// `sendgrid-mail-send` back in is a deliberate edit here rather than a silent regression.
///
/// **The reason for the exclusion moved with C-185 and is now two things, neither of them the
/// emitter.** The `wire` mechanism can build the envelope
/// ([`the_excluded_envelope_shape_is_now_expressible`]), so what remains is: the host cannot yet
/// *compose a request* from a body containing an array — `connector_pack`'s evaluator has arms for
/// `Lit`, `Var`, `Fmt`, `Obj` and `Parse` and refuses everything else with *"its body computes a
/// list, which this pack does not evaluate"* (`crates/connector-pack/src/request.rs:1297-1332`,
/// `kind` at `:1550`) — and the operation itself has never been authored. An operation that
/// emitted, catalogued and could not be called is the C-110 shape this repository has shipped once
/// already; the send waits for the pack.
#[test]
fn the_curated_operation_set_still_excludes_the_send() {
    let connector = sendgrid();
    let mut ids: Vec<&str> = connector
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .collect();
    ids.sort_unstable();

    assert_eq!(
        ids,
        [
            "sendgrid-email-validate",
            "sendgrid-suppression-bounce-list",
            "sendgrid-template-get",
            "sendgrid-template-list",
        ],
        "the curated set changed. Sending mail needs an array-of-objects body envelope: the \
         emitter builds one since C-185, and `connector-pack` cannot yet compose a request from \
         one — see this test's documentation"
    );
}

/// No operation targets SendGrid's mail-send endpoint or any path under `/v3/mail`, under any
/// method — a second, coarser guard than the id list above, so a reintroduction under a different
/// name still fails here.
#[test]
fn no_operation_reaches_the_mail_send_endpoint() {
    let connector = sendgrid();
    for operation in &connector.operations {
        assert!(
            !operation.path.contains("/mail"),
            "`{}` reaches {:?}, under SendGrid's mail-sending surface. That surface was excluded \
             deliberately, and since C-185 it is excluded for the host's reason rather than the \
             emitter's — see `the_curated_operation_set_still_excludes_the_send`",
            operation.id,
            operation.path
        );
    }
}

/// One synthetic stand-in for SendGrid's mail-send envelope, spelled `{wire}` at the recipient.
///
/// Synthetic rather than `providers/sendgrid.toml` because the claim being tested is about the
/// pipeline's body-assembly mechanism in general, not about one provider's authoring choice — and
/// because no recipient address, real or shaped like one, belongs in this repository.
fn envelope_fixture(wire: &str) -> Connector {
    let fixture = format!(
        r#"
id = "acme-mail"
vendor = "Acme"
base_url = "https://api.acme.test"
description = "A fixture standing in for SendGrid's mail-send envelope"

[[operations]]
id = "acme-mail-send"
method = "POST"
direction = "write"
path = "/v3/mail/send"
description = "A synthetic stand-in for SendGrid's mail-send envelope"
risk = "medium"
idempotency = "non_idempotent"

[[operations.params.body]]
name = "to_address"
wire = "{wire}"
description = "The one recipient this fixture addresses"
required = true
schema = {{ type = "string" }}
"#
    );
    provider::load("providers/acme-mail.toml", &fixture)
        .expect("the fixture must load")
        .connector
}

/// **The claim this file used to make, now inverted: the envelope is expressible.** A bracketed
/// index in a `wire` path is an array element (C-185), so `personalizations[0].to[0].email` reaches
/// the vendor as the two nested arrays of objects it requires, rather than as the nested objects
/// that earned SendGrid a 400.
///
/// This is what moves the `sendgrid-mail-send` exclusion off the emitter. It does not by itself
/// ship the operation — see [`the_curated_operation_set_still_excludes_the_send`] for what does.
#[test]
fn the_excluded_envelope_shape_is_now_expressible() {
    let connector = envelope_fixture("personalizations[0].to[0].email");
    let emitted = emit_operation(&connector, &connector.operations[0])
        .unwrap_or_else(|error| panic!("the envelope must emit since C-185: {error}"));

    let payload_line = emitted
        .lines()
        .find(|line| line.trim_start().starts_with("payload ="))
        .unwrap_or_else(|| panic!("no `payload` binding in the emitted op:\n{emitted}"));
    assert_eq!(
        payload_line.trim(),
        "payload = { personalizations: [{ to: [{ email: to_address }] }] }",
        "the assembled payload must carry SendGrid's two arrays, not objects keyed by \
         digits:\n{payload_line}"
    );
}

/// **The trap that used to sit here, now a refusal.** `personalizations.0.to.0.email` built
/// `{"personalizations": {"0": {"to": {"0": {"email": …}}}}}` — a request that parsed, formatted,
/// loaded as one composite op, and that SendGrid answers 400 to, because an object keyed `"0"` is
/// not an array. It was the quietest failure available.
///
/// With `[0]` now meaning the array, two spellings one character apart would mean two different
/// requests, so the ambiguous one is refused and the refusal names the one that works.
#[test]
fn a_bare_numeric_wire_segment_is_refused() {
    let connector = envelope_fixture("personalizations.0.to.0.email");
    let error = match emit_operation(&connector, &connector.operations[0]) {
        Ok(emitted) => panic!("a bare numeric segment must be refused, and emitted:\n{emitted}"),
        Err(error) => error,
    };

    let rendered = error.to_string();
    assert!(
        rendered.contains("all digits") && rendered.contains("[0]"),
        "the refusal must name the segment and show the bracket spelling: {rendered}"
    );
}

/// Every shipped operation emits Flux that parses, is canonical under flux's own formatter, and
/// loads as exactly one composite op — the C-11 gate, held against sendgrid specifically.
#[test]
fn every_sendgrid_operation_emits_an_analyzable_module() {
    let connector = sendgrid();
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

/// Auth is one bearer API key, and the `verify` operation is a genuine, argument-free read.
#[test]
fn the_connector_verifies_with_a_read_over_a_bearer_key() {
    let connector = sendgrid();

    let credentials: Vec<&str> = connector
        .auth
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(
        credentials,
        ["sendgrid.api_key"],
        "one credential covers the whole selection"
    );

    assert_eq!(
        connector.verify.as_deref(),
        Some("sendgrid-template-list"),
        "the `verify` operation is the Test-connection button and must be a read"
    );
    let verify = connector
        .operations
        .iter()
        .find(|operation| Some(operation.id.as_str()) == connector.verify.as_deref())
        .expect("`verify` names a declared operation");
    assert_eq!(
        verify.method,
        HttpMethod::Get,
        "`verify` runs unattended whenever a settings page opens, so it is a GET"
    );
}

/// The API key is configurable and carries no example — a token-shaped placeholder is exactly what
/// tripped GitHub's push protection on a prior release (`providers/shopify.toml` records it).
#[test]
fn the_api_key_is_configurable_and_carries_no_example_value() {
    let connector = sendgrid();

    let field = connector
        .config
        .iter()
        .find(|field| field.name == "api_key")
        .expect("the API key is the one thing a human must supply");

    assert!(field.secret, "an API key is a secret");
    assert_eq!(field.binds, "credential.sendgrid.api_key");
    assert!(
        !field.label.is_empty() && !field.help.is_empty(),
        "a field must be renderable: `label` and `help` are what a settings page shows"
    );
    assert_eq!(
        field.example, None,
        "a secret field carries no example — a token-shaped placeholder trips secret scanning"
    );
}

/// No operation declares a body-nested field without `wire`, and nothing shipped here needs an
/// array: the four curated operations are three bodiless reads and one flat write. That was a
/// consequence of the emitter's limit and is now a consequence of the curation — C-185 made the
/// array expressible without adding one here.
#[test]
fn no_shipped_operation_declares_a_body_field() {
    let connector = sendgrid();
    for operation in &connector.operations {
        assert!(
            operation.params.body.is_empty() || operation.id == "sendgrid-email-validate",
            "`{}` declares a body field. Every read here is bodiless; the one write \
             (`sendgrid-email-validate`) declares flat, unnested fields only — a body-nesting write \
             is exactly what this connector could not ship (see the module doc)",
            operation.id
        );
    }

    let validate = connector
        .operations
        .iter()
        .find(|operation| operation.id == "sendgrid-email-validate")
        .expect("sendgrid-email-validate is part of the curated set");
    for param in &validate.params.body {
        assert!(
            param.wire.is_none(),
            "`sendgrid-email-validate`'s `{}` declares a `wire` path, but its fields are flat by \
             design — a nested `wire` here would be the one place this connector could quietly grow \
             back toward the unexpressible envelope",
            param.name
        );
    }
}

/// No sendgrid operation declares a query parameter carrying free text — every declared query value
/// is a closed enum or a plain integer, so none of them can reach C-30's unencoded-query gap
/// (`zendesk-ticket-search` is the standing demonstration in `AGENTS.md`).
#[test]
fn no_query_parameter_carries_free_text() {
    let connector = sendgrid();
    for operation in &connector.operations {
        for query in &operation.params.query {
            let ty = query.schema.get("type").and_then(|value| value.as_str());
            assert!(
                matches!(ty, Some("integer") | Some("number")) || query.schema.get("enum").is_some(),
                "`{}`'s `{}` query parameter is neither a closed enum nor numeric ({:?}) — nothing \
                 in this pipeline percent-encodes a query value, so free text can inject a parameter",
                operation.id,
                query.name,
                query.schema
            );
        }
    }
}
