//! A request body that contains a **JSON array** — C-185.
//!
//! The body mechanism assembles a nested record from each field's dot-separated
//! [`connector_spec::Param::wire`] path (`crates/connector-flux/src/op.rs`, `body_tree`). Every
//! segment was an object key and nothing else, so a vendor whose write surface is an *envelope* —
//! SendGrid's `{"personalizations": [{"to": [{"email": …}]}]}` — could not be addressed at all, and
//! `providers/sendgrid.toml`'s header records the resulting exclusion.
//!
//! This file is the mechanism's contract, held against synthetic fixtures rather than a provider
//! file: the claim is about what a `wire` path can express, not about one vendor's authoring
//! choice. What was fixed is stated as narrowly as it holds — [`an_indexed_wire_path_builds_a_json_array`]
//! is what is now expressible, [`a_caller_supplied_list_of_objects_is_still_not_decomposable`] is
//! what is not, and the refusals below are the shapes that stay refused because the alternative to
//! each is a request a vendor answers.

use connector_flux::{emit_operation, Error};
use connector_spec::{provider, Connector};

/// Load a synthetic provider, so a fixture goes through exactly the loader a real file does.
fn fixture(id: &str, toml: &str) -> Connector {
    provider::load(&format!("providers/{id}.toml"), toml)
        .unwrap_or_else(|error| panic!("the fixture must load: {error}"))
        .connector
}

/// The `payload = …` line of an emitted operation — the whole request body, on one line.
fn payload_line(emitted: &str) -> String {
    emitted
        .lines()
        .find(|line| line.trim_start().starts_with("payload ="))
        .unwrap_or_else(|| panic!("no `payload` binding in the emitted op:\n{emitted}"))
        .trim()
        .to_string()
}

/// One body field on a `POST`, spelled however the caller of this helper wants to spell it.
fn one_body_field(id: &str, name: &str, wire: &str, extra: &str) -> String {
    format!(
        r#"
id = "{id}"
vendor = "Acme"
base_url = "https://api.acme.test"
description = "A fixture for one body field's wire path"

[[operations]]
id = "{id}-write"
method = "POST"
direction = "write"
path = "/v1/write"
description = "Write one field"
risk = "medium"
idempotency = "non_idempotent"
{extra}

[[operations.params.body]]
name = "{name}"
wire = "{wire}"
description = "The one field this fixture declares"
required = true
schema = {{ type = "string" }}
"#
    )
}

/// SendGrid's envelope, with the vendor's own names kept: an array of objects, each holding a
/// further array of objects, beside a second sibling array at the body root.
const ENVELOPE: &str = r#"
id = "acme-mail"
vendor = "Acme"
base_url = "https://api.acme.test"
description = "A fixture standing in for an envelope-shaped mail vendor"

[[operations]]
id = "acme-mail-send"
method = "POST"
direction = "write"
path = "/v3/mail/send"
description = "Send one message to one recipient"
risk = "medium"
idempotency = "non_idempotent"

[[operations.params.body]]
name = "to_address"
wire = "personalizations[0].to[0].email"
description = "The recipient address"
required = true
schema = { type = "string", format = "email" }

[[operations.params.body]]
name = "from_address"
wire = "from.email"
description = "The verified sender address"
required = true
schema = { type = "string", format = "email" }

[[operations.params.body]]
name = "subject"
description = "The message subject"
required = true
schema = { type = "string" }

[[operations.params.body]]
name = "content_type"
wire = "content[0].type"
description = "Constant `text/plain` — not caller-supplied"
required = true
schema = { type = "string", const = "text/plain" }

[[operations.params.body]]
name = "body_text"
wire = "content[0].value"
description = "The message body"
required = true
schema = { type = "string" }
"#;

/// **The capability.** A bracketed index in a `wire` path addresses an element of an array, so a
/// per-field decomposition of an envelope reaches the vendor as the arrays it requires.
///
/// Asserted on the emitted text rather than only on the AST because the shape of the request is the
/// whole claim: `personalizations`, `to` and `content` are JSON *arrays*, and a bare object in any
/// of those positions is a 400 the vendor answers, not a shorthand it accepts.
///
/// `content_type_2` is not a typo. The emitter binds its own `content_type` for the media type
/// before any parameter is allocated, so a body field the vendor calls `type` and a provider file
/// calls `content_type` gets the next free symbol — the allocator's collision rule (`names.rs`),
/// visible here because a constant body field is bound and never declared.
#[test]
fn an_indexed_wire_path_builds_a_json_array() {
    let connector = fixture("acme-mail", ENVELOPE);
    let emitted = emit_operation(&connector, &connector.operations[0])
        .unwrap_or_else(|error| panic!("an envelope body must emit: {error}"));

    assert_eq!(
        payload_line(&emitted),
        "payload = { content: [{ type: content_type_2, value: body_text }], \
         from: { email: from_address }, \
         personalizations: [{ to: [{ email: to_address }] }], \
         subject }",
        "the assembled body must carry real JSON arrays at `personalizations`, `to` and \
         `content`:\n{emitted}"
    );
}

/// The emitted module is still Flux the engine reads: it parses, it is a fixed point of flux's own
/// formatter, and it loads as exactly one composite op. An array literal is `flux_lang`'s
/// `Node::List`, so this is what makes the new shape a projection of Flux rather than text this
/// crate invented.
#[test]
fn an_envelope_body_parses_analyzes_and_is_canonical() {
    let connector = fixture("acme-mail", ENVELOPE);
    let emitted = emit_operation(&connector, &connector.operations[0]).expect("emittable");

    let parsed = flux_lang::parser::parse_cst(&emitted);
    assert!(
        parsed.errors.is_empty(),
        "an envelope body emits Flux that does not parse: {:?}\n{emitted}",
        parsed.errors
    );
    assert_eq!(
        flux_lang::format_cst::format_module(&parsed).as_deref(),
        Some(emitted.as_str()),
        "the flux formatter would rewrite the emitted module"
    );

    let module = flux_lang::program::Module::parse_str(&emitted)
        .unwrap_or_else(|error| panic!("the emitted module does not load: {error}"));
    let program = module.program().expect("a program");
    assert_eq!(program.ops.len(), 1, "one operation is one declaration");
    assert_eq!(program.ops[0].name, "acme-mail-send");
}

/// One body field, one declared parameter — an index adds no signature of its own. A model calling
/// this op supplies scalars and never sees a bracket.
#[test]
fn an_indexed_field_declares_the_caller_facing_parameter_and_nothing_else() {
    let connector = fixture("acme-mail", ENVELOPE);
    let emitted = emit_operation(&connector, &connector.operations[0]).expect("emittable");
    let signature = emitted
        .lines()
        .next()
        .expect("a declaration line")
        .to_string();

    assert!(
        signature.contains("to_address: String") && signature.contains("body_text: String"),
        "an indexed body field is declared by its caller-facing name: {signature}"
    );
    assert!(
        !signature.contains('['),
        "no part of a wire path reaches the signature: {signature}"
    );
    assert!(
        !signature.contains("content_type"),
        "a constant field is sent and never declared: {signature}"
    );
}

/// **What this does not solve, written as a test so it cannot be quietly assumed.** Every index is
/// declared, so an array's length is a property of the provider file. A caller-supplied *list* of
/// objects — a batch send, a bulk write — would need one element built per value the caller passed,
/// which is a computation over caller data rather than a declaration, and this repository generates
/// the Flux expression precisely so no author ever writes one (AGENTS.md, *Flow graph contract*).
///
/// The older bargain is unchanged and still available: a whole array declared as one caller-supplied
/// value, as `providers/notion.toml`'s rich-text title is. Nothing here checks that shape, which is
/// exactly why the decomposed spelling was worth adding beside it.
#[test]
fn a_caller_supplied_list_of_objects_is_still_not_decomposable() {
    let connector = fixture("acme-mail", ENVELOPE);
    let emitted = emit_operation(&connector, &connector.operations[0]).expect("emittable");

    assert!(
        !emitted.contains("each ") && !emitted.contains("repeat "),
        "nothing here iterates: an envelope is assembled from declared fields, so its length is a \
         property of the provider file and never of a caller's argument:\n{emitted}"
    );
}

/// An index may address a **whole** element, not only a field inside one: the caller supplies the
/// element and this repository supplies the array wrapper. That is the middle of the two bargains —
/// the vendor's envelope is still assembled here, and only the element's own shape is the caller's.
#[test]
fn an_index_may_address_a_whole_caller_supplied_element() {
    let toml = r#"
id = "acme-element"
vendor = "Acme"
base_url = "https://api.acme.test"
description = "A fixture whose array element is one caller value"

[[operations]]
id = "acme-element-write"
method = "POST"
direction = "write"
path = "/v1/write"
description = "Write one element"
risk = "medium"
idempotency = "non_idempotent"

[[operations.params.body]]
name = "run"
wire = "properties.title[0]"
description = "One rich-text run, supplied whole"
required = true
schema = { type = "object" }
"#;
    let connector = fixture("acme-element", toml);
    let emitted = emit_operation(&connector, &connector.operations[0]).expect("emittable");

    assert_eq!(
        payload_line(&emitted),
        "payload = { properties: { title: [run] } }",
        "the array wrapper is this repository's; the element is the caller's:\n{emitted}"
    );
}

/// A hole in an array is not a shape JSON has. Declaring `[0]` and `[2]` and no `[1]` would either
/// drop the gap — shifting the third element into the second position, silently sending the wrong
/// element — or invent a `null` the author never wrote. Both are requests a vendor answers.
#[test]
fn a_sparse_array_declaration_is_refused() {
    let toml = r#"
id = "acme-gap"
vendor = "Acme"
base_url = "https://api.acme.test"
description = "A fixture declaring an array with a hole in it"

[[operations]]
id = "acme-gap-write"
method = "POST"
direction = "write"
path = "/v1/write"
description = "Write two of three elements"
risk = "medium"
idempotency = "non_idempotent"

[[operations.params.body]]
name = "first"
wire = "items[0].value"
description = "Element zero"
required = true
schema = { type = "string" }

[[operations.params.body]]
name = "third"
wire = "items[2].value"
description = "Element two, with element one never declared"
required = true
schema = { type = "string" }
"#;
    let connector = fixture("acme-gap", toml);
    let error = emit_operation(&connector, &connector.operations[0])
        .expect_err("a sparse array is not emittable");

    assert!(
        matches!(
            &error,
            Error::SparseBodyArray { path, missing, .. } if path == "items" && *missing == 1
        ),
        "the refusal must name the array and the missing index, got: {error}"
    );
    let rendered = error.to_string();
    assert!(
        rendered.contains("[0], [2]"),
        "the refusal must show what was declared: {rendered}"
    );
}

/// Every spelling a bracket can be part of that is not an index. Each one would otherwise be
/// absorbed into an object key and reach the vendor verbatim — `{"items[": …}` is accepted and
/// ignored, the failure mode every refusal in this emitter exists to avoid.
#[test]
fn a_malformed_array_index_is_refused() {
    for wire in [
        "items[].value",     // no index at all
        "items[a].value",    // not a number
        "items[-1].value",   // not a position
        "items[01].value",   // two spellings of one index
        "items[0.value",     // never closed
        "items0].value",     // never opened
        "items[0]x.value",   // text after the index
        "items[0][1].value", // an array directly inside an array
    ] {
        let toml = one_body_field("acme-bad", "field", wire, "");
        let connector = fixture("acme-bad", &toml);
        let error = match emit_operation(&connector, &connector.operations[0]) {
            Ok(emitted) => panic!("`{wire}` must be refused, and instead emitted:\n{emitted}"),
            Err(error) => error,
        };
        assert!(
            matches!(error, Error::BadArrayIndex { .. }),
            "`{wire}` must be refused as a malformed index, got: {error}"
        );
    }
}

/// A bare numeric segment is the trap this story had to close rather than leave sitting beside a
/// working spelling. `items.0.value` built `{"items": {"0": {"value": …}}}` — an object keyed
/// `"0"`, which every vendor that wanted an array answers 400 to, and which
/// `providers/sendgrid.toml` records an author reaching for. With `items[0].value` now meaning the
/// array, two spellings one character apart would mean two different requests.
///
/// The cost is stated in [`Error::NumericWirePathSegment`]: a vendor whose object key is genuinely
/// a number has no spelling here. None of the 53 provider files has one.
#[test]
fn a_bare_numeric_segment_is_refused_and_names_the_bracket_spelling() {
    let toml = one_body_field("acme-numeric", "field", "items.0.value", "");
    let connector = fixture("acme-numeric", &toml);
    let error = emit_operation(&connector, &connector.operations[0])
        .expect_err("a bare numeric segment is not emittable");

    assert!(
        matches!(&error, Error::NumericWirePathSegment { segment, .. } if segment == "0"),
        "the refusal must name the numeric segment, got: {error}"
    );
    assert!(
        error.to_string().contains("[0]"),
        "the refusal must show the spelling that builds an array: {error}"
    );
}

/// C-144's `form` encoding refuses nesting outright, and an array is nesting. A form body is
/// assembled by `fmt` as `key=value` text, so an indexed key would reach the vendor as the literal
/// `items[0]=…`. Some form parsers do read that as an array; none of the vendors described in this
/// repository has been checked for it, which is exactly the guess this refuses.
#[test]
fn a_form_body_refuses_an_indexed_path() {
    let toml = one_body_field(
        "acme-form",
        "field",
        "items[0]",
        "[operations.params]\nbody_encoding = \"form\"",
    );
    let connector = fixture("acme-form", &toml);
    let error = emit_operation(&connector, &connector.operations[0])
        .expect_err("a form body cannot carry an array");

    assert!(
        matches!(&error, Error::UnencodableFormField { name, .. } if name == "field"),
        "the refusal must name the field, got: {error}"
    );
}

/// An array at the **root** of the body — a batch write, `providers/postmark.toml`'s batch send —
/// stays unexpressible, and deliberately: a root array is a caller-supplied list in every vendor
/// this repository has looked at, so a fixed-length spelling for it would buy nothing. It is
/// refused by the rule that has always refused `.a`, because it needs an empty first segment.
#[test]
fn an_array_at_the_body_root_is_refused() {
    let toml = one_body_field("acme-batch", "subject", "[0].Subject", "");
    let connector = fixture("acme-batch", &toml);
    let error = emit_operation(&connector, &connector.operations[0])
        .expect_err("a root array is not expressible");

    assert!(
        matches!(&error, Error::BadWirePath { .. }),
        "the refusal must be the empty-segment one, got: {error}"
    );
}

/// A body field whose **name** carries a bracket and declares no `wire` is refused for the reason a
/// dotted one already was: `items[0]` is either a field the vendor spells with brackets or element
/// zero of `items`, and nothing can decide which. Adding an array spelling made this ambiguity
/// reachable, so the existing refusal grew a bracket.
#[test]
fn a_bracketed_name_with_no_wire_path_is_refused() {
    let toml = r#"
id = "acme-unwired"
vendor = "Acme"
base_url = "https://api.acme.test"
description = "A fixture naming a body field with a bracket and no wire"

[[operations]]
id = "acme-unwired-write"
method = "POST"
direction = "write"
path = "/v1/write"
description = "Write a bracketed name"
risk = "medium"
idempotency = "non_idempotent"

[[operations.params.body]]
name = "items[0]"
description = "A field whose name is bracketed and whose path is undeclared"
required = true
schema = { type = "string" }
"#;
    let connector = fixture("acme-unwired", toml);
    let error = emit_operation(&connector, &connector.operations[0])
        .expect_err("a bracketed name with no wire is undecidable");

    assert!(
        matches!(&error, Error::NestedBodyField { name, .. } if name == "items[0]"),
        "the refusal must name the field, got: {error}"
    );
}

/// A path that needs one position to be both an array and an object is the array-side twin of the
/// conflict `ticket.comment` and `ticket.comment.body` already produce. Either resolution drops a
/// field the author declared.
#[test]
fn a_path_that_is_an_array_and_an_object_at_once_is_refused() {
    let toml = r#"
id = "acme-clash"
vendor = "Acme"
base_url = "https://api.acme.test"
description = "A fixture claiming one path as two shapes"

[[operations]]
id = "acme-clash-write"
method = "POST"
direction = "write"
path = "/v1/write"
description = "Write a contested path"
risk = "medium"
idempotency = "non_idempotent"

[[operations.params.body]]
name = "indexed"
wire = "items[0].value"
description = "`items` as an array"
required = true
schema = { type = "string" }

[[operations.params.body]]
name = "keyed"
wire = "items.value"
description = "`items` as an object"
required = true
schema = { type = "string" }
"#;
    let connector = fixture("acme-clash", toml);
    let error = emit_operation(&connector, &connector.operations[0])
        .expect_err("one path cannot be two shapes");

    assert!(
        matches!(&error, Error::BodyPathConflict { path, .. } if path == "items"),
        "the refusal must name the contested path, got: {error}"
    );
    let rendered = error.to_string();
    assert!(
        rendered.contains("indexed") && rendered.contains("keyed"),
        "both sides are named, so an author does not have to find the other one: {rendered}"
    );
}
