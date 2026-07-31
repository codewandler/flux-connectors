//! `providers/sendgrid.toml` exists, and **it does not — and structurally cannot — send mail**
//! through this pipeline's named-body-parameter mechanism (C-168).
//!
//! SendGrid's `POST /v3/mail/send` takes `personalizations: [{"to": [{"email": "…"}]}]` — an array
//! of objects containing a further array of objects. This file's central claim is not "the connector
//! parses" but "the body-nesting mechanism this pipeline has cannot place a value inside an array,
//! at any depth, so a per-field decomposition of that envelope is not merely unwritten — it is
//! unwritable." The mechanical proof is [`a_wire_path_that_looks_like_an_array_index_still_builds_an_object`]
//! below, which is deliberately independent of `providers/sendgrid.toml` — it demonstrates the claim
//! about the *pipeline*, not about this one provider file — while every other test here does depend
//! on the provider file loading, which is what gives this file its failing-first shape.

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
/// `sendgrid-mail-send` back in is a deliberate edit here rather than a silent regression — the
/// exclusion is not "not yet written", it is "not expressible with this pipeline's `wire` mechanism"
/// (see the module doc and `providers/sendgrid.toml`'s header comment).
#[test]
fn the_curated_operation_set_excludes_the_unexpressible_send() {
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
        "the curated set changed. Sending mail needs an array-of-objects body envelope, which \
         `BodyNode` (crates/connector-flux/src/op.rs) cannot build — see the module doc"
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
            "`{}` reaches {:?}, under SendGrid's mail-sending surface. That surface's request body \
             is an array-of-objects envelope this pipeline's `wire` mechanism cannot build \
             (see providers/sendgrid.toml's header comment) and was excluded deliberately",
            operation.id,
            operation.path
        );
    }
}

/// **The mechanical proof.** `wire`'s only nesting primitive is a dot-separated object path
/// (`BodyNode::Branch(BTreeMap<String, BodyNode>)`, `crates/connector-flux/src/op.rs`), so a segment
/// that reads like an array index — `"0"` — is not special-cased into "the next array element": it
/// becomes an ordinary object key, indistinguishable from any other. This is checked against a
/// synthetic fixture rather than `providers/sendgrid.toml` because the claim is about the pipeline's
/// body-assembly mechanism in general, not about one provider's authoring choice.
///
/// The emitter does **not** refuse this — refusing it would at least be loud. Instead it succeeds and
/// assembles nested objects, which is the quieter and more dangerous failure: a caller who mistook a
/// numeric `wire` segment for "build me an array here" gets a request that parses, formats, and loads
/// as one composite op, and that SendGrid answers 400 to, because `{"personalizations": {"0": {"to":
/// {"0": {"email": …}}}}}` is not the array `[{"to": [{"email": …}]}]` it required.
#[test]
fn a_wire_path_that_looks_like_an_array_index_still_builds_an_object() {
    let fixture = r#"
id = "acme-mail"
vendor = "Acme"
base_url = "https://api.acme.test"
description = "A fixture proving wire paths cannot build an array"

[[operations]]
id = "acme-mail-send"
method = "POST"
path = "/v3/mail/send"
description = "A synthetic stand-in for SendGrid's mail-send envelope"
risk = "medium"
idempotency = "non_idempotent"

[[operations.params.body]]
name = "to_address"
wire = "personalizations.0.to.0.email"
description = "The one recipient this fixture attempts to address, by a wire path shaped like an array index"
required = true
schema = { type = "string" }
"#;
    let connector = provider::load("providers/acme-mail.toml", fixture)
        .expect("the fixture must load")
        .connector;

    let emitted = emit_operation(&connector, &connector.operations[0]).unwrap_or_else(|error| {
        panic!(
            "an array-shaped wire path is not refused at all — it is silently wrong, which is the \
             point this test makes. It should not fail to emit, and yet it errored: {error}"
        )
    });

    // No array survives: the assembled payload nests only `Node::Obj` records, so no `[` from an
    // array literal appears anywhere the payload is built.
    let payload_line = emitted
        .lines()
        .find(|line| line.trim_start().starts_with("payload ="))
        .unwrap_or_else(|| panic!("no `payload` binding in the emitted op:\n{emitted}"));
    assert!(
        !payload_line.contains('['),
        "the assembled body must contain no JSON array — `wire` can only build nested objects, so \
         `personalizations` and `to` both come out as objects rather than arrays:\n{payload_line}"
    );
    // The numeric segment is placed as a literal object key, exactly like any other segment — proof
    // that "0" was never read as "index zero of a list" rather than "the object key `0`".
    // The exact shape measured: nested objects, each numeric segment surviving as a *quoted string
    // key* (`"0"`), never as an array index or an array literal. This is the mechanical proof that
    // `wire` has no array primitive — a segment that looks like an index is just an object key that
    // happens to be all digits.
    assert_eq!(
        payload_line.trim(),
        r#"payload = { personalizations: { "0": { to: { "0": { email: to_address } } } } }"#,
        "the assembled payload must be nested objects with quoted numeric keys, not arrays:\n{payload_line}"
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

/// No operation declares a body-nested field without `wire`, and — the point of this file — no
/// operation's `wire` path, if one is declared, ever needs an array to express: everything shipped
/// here nests objects at most, never arrays. Read operations carry no body at all.
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
