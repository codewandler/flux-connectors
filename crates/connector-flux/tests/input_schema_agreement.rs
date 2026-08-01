//! **One operation, two surfaces, and the proof they describe the same input.**
//!
//! Two things now say what an operation receives, and they were derived independently:
//!
//! - [`connector_spec::Operation::input_schema`] composes the IR's declared parameters into one
//!   JSON Schema. That is the catalogue's answer, and it is what `web/public/catalog.json` carries.
//! - `connector-pack`'s `ToolSpec` projection parses the operation's **emitted Flux** and lowers the
//!   declaration through flux's own `OpSpec::lower`. That is the host's answer, and it is
//!   deliberately taken from the shipped artifact so the pack's answer *is* the module's answer.
//!
//! Two derivations of one schema is exactly the drift `AGENTS.md` warns about for the C-12/C-95
//! lowering, so this file is the resolution: it holds them together over every shipped operation.
//! Neither derivation could simply consume the other, and the reasons are structural rather than
//! stylistic:
//!
//! - **A Flux composite op cannot declare the IR's names.** babelforce's `time.start` is not a
//!   spellable symbol — `$time.start` reparses as field access — so the declaration says
//!   `time_start`. `connector-spec` cannot compute that name: the mapping lives in this crate, one
//!   dependency edge *downstream* of the IR. So the composed schema keys by the caller-facing name
//!   and this test carries the correspondence, through [`parameter_symbols`], which is the same
//!   allocation the emitter itself used.
//! - **A Flux composite op has no optional parameter.** The pack's `required` is therefore
//!   necessarily every parameter — its own request builder refuses a call that omits one
//!   (`connector-pack/src/request.rs`, "every declared parameter must be supplied") — while the
//!   composed schema states what the *vendor* requires. Those are two different true statements, so
//!   the second test below pins the relationship between them instead of pretending it away.
//!
//! What is asserted is therefore the strongest thing that is true: the two describe the **same
//! parameter set**, modulo the symbol mapping and the one documented exception, and the composed
//! `required` is always a subset of it. A provider that broke either would fail here rather than in
//! a host, where the symptom is a model passing an argument the tool does not have.

use std::path::{Path, PathBuf};

use connector_flux::{emit_operation, parameter_symbols};
use connector_spec::{Connector, Operation};

#[path = "../../connector-spec/tests/support/shipped_provider.rs"]
mod shipped_provider;

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

/// Every provider this repository ships, read from `providers/` rather than listed here (C-54).
fn shipped() -> Vec<String> {
    let dir = providers_dir();
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()))
        .map(|entry| entry.expect("readable directory entry").file_name())
        .filter_map(|name| {
            name.to_string_lossy()
                .strip_suffix(".toml")
                .map(str::to_string)
        })
        .collect();
    names.sort();

    assert!(
        !names.is_empty(),
        "{} holds no provider definitions, so every gate in this file would pass vacuously",
        dir.display()
    );
    names
}

fn load(name: &str) -> Connector {
    let path = providers_dir().join(format!("{name}.toml"));
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    shipped_provider::load_definition(name, &source)
        .unwrap_or_else(|error| panic!("providers/{name}.toml does not load: {error}"))
        .connector
}

/// The parameter names the emitted `op` declares, read back from the Flux the same way
/// `connector-pack` reads it: parse the rendering, take its single composite op.
fn declared_parameters(connector: &Connector, operation: &Operation) -> Vec<String> {
    let emitted = emit_operation(connector, operation)
        .unwrap_or_else(|error| panic!("`{}` does not emit: {error}", operation.id));
    let module = flux_lang::program::Module::parse_str(&emitted)
        .unwrap_or_else(|error| panic!("`{}` does not parse: {error}", operation.id));
    let program = module
        .program()
        .unwrap_or_else(|| panic!("`{}` is not a program", operation.id));

    program.ops[0]
        .params
        .iter()
        .map(|param| param.name.0.clone())
        .collect()
}

/// A body field pinned with a JSON Schema `const` is **sent but never declared**: the schema already
/// fixes its value, so asking a model to supply it would be asking it to guess a decided answer
/// (`connector-flux`'s `constant`). It is the one property of the composed schema with no Flux
/// parameter behind it, and naming the exception here is what keeps it from quietly growing.
fn is_pinned_constant(operation: &Operation, property: &str) -> bool {
    operation
        .params
        .body
        .iter()
        .any(|param| param.name == property && param.schema.get("const").is_some())
}

/// **The two derivations describe the same parameters.**
///
/// Read in both directions on purpose. Left to right catches a parameter the catalogue publishes
/// that a host cannot pass; right to left catches a parameter a host must pass that the catalogue
/// never mentions. Only one of the two would have caught babelforce's dotted names.
#[test]
fn the_composed_input_schema_and_the_emitted_declaration_describe_the_same_parameters() {
    for name in shipped() {
        let connector = load(&name);
        for operation in &connector.operations {
            let id = &operation.id;
            let schema = operation.input_schema();
            let properties = schema["properties"]
                .as_object()
                .unwrap_or_else(|| panic!("`{id}` composes a properties object"));

            // The correspondence itself: caller-facing name → the symbol the emitter allocated.
            let symbols =
                parameter_symbols(operation).unwrap_or_else(|error| panic!("`{id}`: {error}"));

            let mut declared = declared_parameters(&connector, operation);
            declared.sort();
            let mut mapped: Vec<String> = symbols.values().cloned().collect();
            mapped.sort();
            assert_eq!(
                declared, mapped,
                "providers/{name}.toml: `{id}` declares {declared:?} in Flux, which is not the \
                 symbol image of what its composed input schema names"
            );

            for caller_facing in symbols.keys() {
                assert!(
                    properties.contains_key(caller_facing),
                    "providers/{name}.toml: `{id}` takes `{caller_facing}` but its composed input \
                     schema does not name it, so a caller reading the catalogue cannot supply it"
                );
            }
            for property in properties.keys() {
                assert!(
                    symbols.contains_key(property) || is_pinned_constant(operation, property),
                    "providers/{name}.toml: `{id}` publishes `{property}` in its composed input \
                     schema, but the emitted op takes no such parameter and it is not a `const`-\
                     pinned body field"
                );
            }
        }
    }
}

/// **`required` is the one place the two answers differ, and the difference is stated here.**
///
/// The composed schema says what the *vendor* requires. Anything reading the emitted declaration
/// must say *everything*, because flux has no optional composite-op parameter and the pack's
/// request builder refuses a call that omits one. So the composed `required` is a subset of the
/// properties, and — the assertion that makes this a test rather than a comment — a **proper**
/// subset somewhere in the shipped catalogue. If it ever became equality everywhere, the two
/// statements would have collapsed into one and this file would be asserting nothing.
#[test]
fn required_is_the_vendors_answer_and_is_a_proper_subset_somewhere() {
    let mut somewhere_optional = None;

    for name in shipped() {
        let connector = load(&name);
        for operation in &connector.operations {
            let id = &operation.id;
            let schema = operation.input_schema();
            let properties = schema["properties"]
                .as_object()
                .unwrap_or_else(|| panic!("`{id}` composes a properties object"));
            let required = schema["required"]
                .as_array()
                .unwrap_or_else(|| panic!("`{id}` composes a required array"));

            for entry in required {
                let entry = entry.as_str().expect("a required entry is a name");
                assert!(
                    properties.contains_key(entry),
                    "providers/{name}.toml: `{id}` requires `{entry}`, which it does not declare"
                );
            }
            if required.len() < properties.len() && somewhere_optional.is_none() {
                somewhere_optional = Some(id.clone());
            }
        }
    }

    assert!(
        somewhere_optional.is_some(),
        "no shipped operation declares an optional parameter, so the composed `required` and the \
         emitted declaration's \"everything is required\" cannot be told apart — the divergence \
         this test documents would be untested"
    );
}
