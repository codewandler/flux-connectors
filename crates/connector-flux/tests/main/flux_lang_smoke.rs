//! Smoke test for the flux-lang dependency.
//!
//! This crate emits Flux by building `flux_lang` AST nodes, never string templates, so the whole
//! emitter rests on that dependency resolving and its parser being usable from here. This test is
//! the cheapest possible proof of both: it parses a trivial `.flux` source through the public entry
//! point and asserts the result. If the pin in the root `Cargo.toml` ever stops resolving, or
//! flux-lang moves `Module::parse_str`, this fails before any codegen story does.

use flux_lang::program::Module;

#[test]
fn parses_trivial_flux_module() {
    let module = Module::parse_str("flow ping\n  return null")
        .expect("a trivial flux source must parse through flux_lang");

    match module {
        Module::Flow(flow) => assert_eq!(flow.name.as_deref(), Some("ping")),
        Module::Program(_) => panic!("a lone flow header must not sniff as a program"),
    }
}
