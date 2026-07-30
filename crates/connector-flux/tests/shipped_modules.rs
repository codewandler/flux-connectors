//! Every operation of every **shipped** provider emits Flux that parses and is canonical.
//!
//! `op_emitter.rs` pins hand-written fixtures; this file pins the 25 real operations in
//! `providers/*.toml`. Those are different claims. A fixture proves the emitter handles a shape; only
//! this proves the shapes the repository actually ships are among them — and it is the shipped set
//! that a `flux-connectors build` turns into `connectors/*.flux` for flux to load.
//!
//! It reads the provider files from the repository root, like `connector-spec`'s own
//! `shipped_providers.rs`, because a copy embedded here would be the thing under test drifting away
//! from the thing that ships.
//!
//! The fixed-point assertion is the load-bearing half. flux's own CST formatter is what a human
//! editing a generated module would run, so text that is not already a fixed point of it would be
//! rewritten the first time anyone touched the file — and a generated artifact that reformats on
//! sight is one nobody can review a diff of.

use std::path::{Path, PathBuf};

use connector_flux::emit_operation;
use connector_spec::{provider, Connector};

/// The providers this repository ships: the three C-17 names, in its order, then each one
/// added since — `github` by C-52, `openai` by C-51.
const SHIPPED: &[&str] = &["zendesk", "freshdesk", "babelforce", "github", "openai"];

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("providers")
}

fn load(name: &str) -> Connector {
    let path = providers_dir().join(format!("{name}.toml"));
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    provider::load(&format!("providers/{name}.toml"), &source)
        .unwrap_or_else(|error| panic!("providers/{name}.toml does not load: {error}"))
        .connector
}

/// Every shipped operation is emittable. A single refusal aborts a whole `build`, so this is the
/// difference between three generated modules and none — which is exactly the state C-29 found the
/// repository in, over one `presence.name`.
#[test]
fn every_shipped_operation_emits() {
    for name in SHIPPED {
        let connector = load(name);
        for operation in &connector.operations {
            emit_operation(&connector, operation).unwrap_or_else(|error| {
                panic!(
                    "providers/{name}.toml: operation `{}`: {error}",
                    operation.id
                )
            });
        }
    }
}

/// And every shipped operation's text parses and is already canonical — the property `op_emitter.rs`
/// asserts for its fixtures, held against the real inventory.
#[test]
fn every_shipped_operation_is_a_fixed_point_of_the_flux_formatter() {
    for name in SHIPPED {
        let connector = load(name);
        for operation in &connector.operations {
            let emitted = emit_operation(&connector, operation).expect("shipped operations emit");
            let parsed = flux_lang::parser::parse_cst(&emitted);
            assert!(
                parsed.errors.is_empty(),
                "providers/{name}.toml: `{}` emits Flux that does not parse: {:?}\n{emitted}",
                operation.id,
                parsed.errors
            );
            assert_eq!(
                flux_lang::format_cst::format_module(&parsed).as_deref(),
                Some(emitted.as_str()),
                "providers/{name}.toml: the flux formatter would rewrite `{}`",
                operation.id
            );
        }
    }
}

/// Each emitted declaration loads back through flux-lang's own module loader carrying the metadata
/// flux's approval gate reads. A module that parsed but did not *load* would publish no ops at all.
#[test]
fn every_shipped_operation_reloads_as_a_composite_op() {
    for name in SHIPPED {
        let connector = load(name);
        for operation in &connector.operations {
            let emitted = emit_operation(&connector, operation).expect("shipped operations emit");
            let module = flux_lang::program::Module::parse_str(&emitted)
                .unwrap_or_else(|error| panic!("`{}` does not load: {error}", operation.id));
            let program = module
                .program()
                .unwrap_or_else(|| panic!("`{}` is not a program", operation.id));
            assert_eq!(program.ops.len(), 1);
            assert_eq!(program.ops[0].name, operation.id);
            assert!(
                program.ops[0].meta.expose,
                "`{}` must be exposed to the model as a tool",
                operation.id
            );
        }
    }
}
