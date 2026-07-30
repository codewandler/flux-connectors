//! **Every embedded operation is valid Flux on its own** — the C-11 gate applied per operation
//! rather than per provider module.
//!
//! `connector-flux`'s `shipped_modules.rs` makes the same three assertions, and they are not the
//! same claim. That file tests what the *emitter produces right now*; this one tests the text that
//! is actually **compiled into this crate**, which is what a consumer receives. Between the two
//! sits a generated, committed artifact that can be stale, hand-edited or truncated, and every one
//! of those failures is invisible to a test that re-emits.
//!
//! Per operation rather than per module also matters on its own terms. The catalog's unit is one
//! `op`, so a consumer takes one declaration and hands it to flux; a declaration that only parses
//! in the company of its siblings would be one nobody could use that way. Each rendering is
//! therefore parsed, formatter-checked and *loaded* by itself.
//!
//! The staleness half of the story lives next door in
//! `crates/connector-cli/tests/catalog_artifacts.rs`, which recomputes these bytes from
//! `providers/*.toml`; this file needs no filesystem access at all except for the one test that
//! checks the hand-written provider list.

use catalog::{Operation, OperationKey, ProviderKey};

/// Every operation in the catalog, as a flat list.
fn all() -> Vec<&'static Operation> {
    catalog::providers()
        .iter()
        .flat_map(|provider| provider.operations.iter())
        .collect()
}

/// An empty catalog would satisfy every `for` loop below without saying a word.
#[test]
fn the_catalog_is_not_empty() {
    assert_eq!(
        catalog::providers().len(),
        6,
        "C-17's three providers, plus github (C-52), openai (C-51) and slack (C-53)"
    );
    assert_eq!(
        all().len(),
        38,
        "38 operations ship today — C-17's 25, github's 5, openai's 4 and slack's 4; if \
         this changed deliberately, change the number"
    );
}

/// **The gate.** Each embedded rendering parses on its own, with no diagnostics.
#[test]
fn every_embedded_operation_parses() {
    for operation in all() {
        let parsed = flux_lang::parser::parse_cst(operation.flux);
        assert!(
            parsed.errors.is_empty(),
            "`{}` embeds Flux that does not parse: {:?}\n{}",
            operation.id,
            parsed.errors,
            operation.flux
        );
    }
}

/// And each is already canonical: flux's own formatter would not rewrite a byte of it.
///
/// This is the half that keeps the artifact reviewable. A generated file that reformats the first
/// time a human opens it in an editor produces a diff nobody can read.
#[test]
fn every_embedded_operation_is_a_fixed_point_of_the_flux_formatter() {
    for operation in all() {
        let parsed = flux_lang::parser::parse_cst(operation.flux);
        assert_eq!(
            flux_lang::format_cst::format_module(&parsed).as_deref(),
            Some(operation.flux),
            "the flux formatter would rewrite `{}`",
            operation.id
        );
    }
}

/// **The analyze half**: each rendering loads through flux-lang's module loader as exactly one
/// composite op, carrying the name and the exposure flux's approval gate and tool registry read.
///
/// Parsing is not enough — a module that parsed but did not load would publish no ops at all, and
/// a consumer handing it to flux would get silence rather than an error.
#[test]
fn every_embedded_operation_loads_as_a_composite_op() {
    for operation in all() {
        let module = flux_lang::program::Module::parse_str(operation.flux)
            .unwrap_or_else(|error| panic!("`{}` does not load: {error}", operation.id));
        let program = module
            .program()
            .unwrap_or_else(|| panic!("`{}` is not a program", operation.id));

        assert_eq!(
            program.ops.len(),
            1,
            "one rendering is one operation; `{}` loaded {}",
            operation.id,
            program.ops.len()
        );
        assert_eq!(program.ops[0].name, operation.id);
        assert!(
            program.ops[0].meta.expose,
            "`{}` must be exposed to the model as a tool",
            operation.id
        );
    }
}

/// **The metadata describes the Flux it is attached to.** `risk`, `idempotency` and the host are
/// carried twice — once as a queryable field and once inside the declaration flux reads — and a
/// caller deciding whether to run an operation must not be able to get a different answer from the
/// two.
///
/// This is also what pins the catalog's mirrored [`catalog::Risk`] and [`catalog::Idempotency`]
/// vocabularies against flux's own spelling, without this crate depending on either.
#[test]
fn metadata_agrees_with_the_embedded_flux() {
    for operation in all() {
        assert!(
            operation
                .flux
                .contains(&format!("risk \"{}\"", operation.risk.as_str())),
            "`{}` is catalogued as {:?} but does not declare it:\n{}",
            operation.id,
            operation.risk,
            operation.flux
        );
        assert!(
            operation.flux.contains(&format!(
                "idempotency \"{}\"",
                operation.idempotency.as_str()
            )),
            "`{}` is catalogued as {:?} but does not declare it:\n{}",
            operation.id,
            operation.idempotency,
            operation.flux
        );
        for host in operation.hosts {
            assert!(
                operation.flux.contains(host),
                "`{}` is catalogued as reaching `{host}`, which its request never mentions:\n{}",
                operation.id,
                operation.flux
            );
        }
        assert!(
            operation.flux.contains(&format!("op {} ", operation.id))
                || operation.flux.contains(&format!("op {}(", operation.id)),
            "`{}` embeds a declaration of something else:\n{}",
            operation.id,
            operation.flux
        );
    }
}

/// **No credential ever enters a generated artifact** (AGENTS.md). The catalog carries credential
/// *references* so a caller can see what an operation needs; the Flux carries not even that yet
/// (auth injection is C-10). Neither carries an environment variable's value, and neither may.
#[test]
fn no_operation_names_a_credential_it_does_not_reference() {
    for operation in all() {
        for mechanism in operation.credentials {
            assert!(
                !mechanism.is_empty(),
                "`{}` declares an empty mechanism — \"no auth\" is an empty alternatives list, not \
                 a list holding an empty one",
                operation.id
            );
            for credential in *mechanism {
                assert!(
                    !credential.is_empty(),
                    "`{}` names an empty credential",
                    operation.id
                );
            }
        }
    }
}

/// The two directions of the middle level: every operation belongs to a provider that lists it, and
/// every provider's listing holds only its own.
#[test]
fn listing_by_provider_round_trips() {
    for provider in catalog::providers() {
        let listed = catalog::operations_of(ProviderKey::id(provider.id));
        assert!(!listed.is_empty(), "`{}` publishes nothing", provider.id);
        for operation in listed {
            assert_eq!(operation.provider, provider.id);
            assert_eq!(
                catalog::operation(OperationKey::id(operation.id)),
                Some(operation),
                "`{}` is listed under `{}` but not findable by key",
                operation.id,
                provider.id
            );
        }
    }
}

/// **The hand-written module list matches what ships.** `src/generated.rs` names one module per
/// provider by hand — see its docs for why — so a provider added to `providers/` without that line
/// would be compiled, written into `ops/` and `src/generated/`, and then silently left out of every
/// query.
#[test]
fn the_provider_list_matches_the_repository() {
    let providers_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../providers");
    let mut shipped: Vec<String> = std::fs::read_dir(&providers_dir)
        .expect("the repository's providers/ directory is readable")
        .map(|entry| entry.expect("readable directory entry").file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .filter_map(|name| name.strip_suffix(".toml").map(str::to_string))
        .collect();
    shipped.sort();

    let catalogued: Vec<String> = catalog::providers()
        .iter()
        .map(|provider| provider.id.to_string())
        .collect();

    assert_eq!(
        catalogued, shipped,
        "`crates/catalog/src/generated.rs` and `providers/` disagree — add or remove the `mod` \
         line and the `PROVIDERS` entry"
    );
}
