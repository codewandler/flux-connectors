//! The connector Tool pack: this repository's connectors, as flux tools.
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # use flux_runtime::ToolRegistry;
//! let mut registry = ToolRegistry::new();
//! connector_pack::pack(&["zendesk"])(&mut registry)?;
//!
//! assert!(registry.get("zendesk.ticket.show").is_some());
//! # Ok(())
//! # }
//! ```
//!
//! or, from a host that uses flux's SDK, the same value straight into the builder:
//!
//! ```ignore
//! let client = flux_sdk::Client::builder()
//!     .try_register_pack(connector_pack::pack(&["zendesk", "slack"]))
//!     .build()?;
//! ```
//!
//! # Why a Tool pack, and not more `.flux`
//!
//! `connectors/<provider>.flux` keeps shipping and keeps being the human-readable contract. This is
//! an **additional** surface, and it exists because of a naming asymmetry that no amount of
//! composite Flux can get around: a **dotted name is not a legal Flux declaration**, which is why
//! this repository emits `zendesk-ticket-show`, and **every flux tool is dotted** — `http.request`,
//! `op.register`, `skill.load`. flux's reference flow calls `zendesk.ticket.show`. It was written
//! against a tool surface, and only a tool surface can spell it. See [`dotted_name`].
//!
//! The safety argument is the stronger one. As a composite, an operation inherits whatever gating
//! `http.request` happens to get. As a Tool, each operation is gated **individually** by flux's
//! permission and approval envelope, at the risk level the connector author declared — a capability
//! the composite path cannot have.
//!
//! # This crate still ships no runtime
//!
//! It links flux's runtime types and constructs none of it. `vision.md`'s non-goal stands: *this
//! repo compiles; flux executes.* A host builds the registry, binds the ports and runs the loop;
//! [`pack`] hands it declarations. The compiler crates — `connector-spec`, `connector-flux`,
//! `connector-cli` — link none of this, and `connector-catalog` stays dependency-free.
//!
//! # What is not here yet
//!
//! `Tool::execute` returns [`not_wired_yet`]. Building the request and delegating it to flux's own
//! `HttpRequestTool` is **C-115**, and the credential port is **C-116**. Registering this pack today
//! gives a host a complete, gated *catalogue* of what a connector offers and no way to call it —
//! which is the honest state, and a better one than a call that reaches a vendor without the
//! network gate C-115 must mirror.

mod name;
mod spec;
mod tool;

pub use name::{dotted_name, NameError};
pub use spec::project;
pub use tool::Operation;

use catalog::ProviderKey;
use flux_runtime::{Tool, ToolRegistry};
use std::sync::Arc;

/// Why a pack could not be installed.
///
/// Every variant refuses; none repairs. A pack that installed *most* of a provider would leave a
/// host holding a connector that resolves some of its operations and silently not others.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The pack named a connector this catalogue does not carry.
    ///
    /// Reported rather than skipped: registering nothing and returning success is how a host ends
    /// up with a client that cannot call anything and no diagnostic saying why. A typo in a
    /// provider name is the overwhelmingly likely cause.
    #[error(
        "no connector named `{provider}` is in this catalogue — `catalog::providers()` lists the \
         {available} this build carries"
    )]
    UnknownProvider {
        /// The name that was asked for.
        provider: String,
        /// How many connectors the catalogue does carry.
        available: usize,
    },

    /// The operation's id has no dotted tool name. See [`NameError`].
    #[error("`{operation}` cannot be projected onto a flux tool name: {source}")]
    Name {
        /// The operation id.
        operation: String,
        /// Which end of the seam refused it.
        #[source]
        source: NameError,
    },

    /// The Flux embedded for an operation does not parse.
    ///
    /// Unreachable for a catalogue this repository generated —
    /// `crates/catalog/tests/embedded_operations.rs` parses every rendering on its own as a
    /// standing gate. It is reported as the corrupt-input case it would be rather than unwrapped,
    /// because the alternative is a panic inside a host's registration call.
    #[error("the Flux embedded for `{operation}` does not parse: {message}")]
    Unparsable {
        /// The operation id.
        operation: String,
        /// What flux-lang said.
        message: String,
    },

    /// A catalogue rendering is exactly one `op` declaration, and this one is not.
    #[error(
        "the Flux embedded for `{operation}` declares {found} operations; one catalogue rendering \
         is exactly one declaration"
    )]
    NotOneOperation {
        /// The operation id.
        operation: String,
        /// How many declarations were found.
        found: usize,
    },

    /// The projected spec is one flux itself will not register.
    ///
    /// flux pairs `effects` with `access`: a declared effect with no host capability that could
    /// carry it is refused by `authority_requirements_from_declaration`, and therefore by
    /// `try_register_from`. Unreachable for a declaration this repository emits — every generated
    /// op declares `["network"]` alone, which pairs with network access — and reported here rather
    /// than left to surface at a host's startup with only the tool name to go on.
    ///
    /// The refusal is deliberate. Satisfying flux by claiming an access kind the connector does not
    /// have would register a tool that looks gated and is not.
    #[error("`{operation}` projects onto a spec flux will not register: {message}")]
    Unregistrable {
        /// The operation id.
        operation: String,
        /// What flux's authority checker said.
        message: String,
    },

    /// A rendering declaring a different operation than the entry it is filed under.
    ///
    /// Left alone this would register a tool under a name derived from one id carrying a contract
    /// taken from another — the two halves of an operation coming apart with nothing to notice it.
    #[error(
        "the entry for `{operation}` embeds a declaration of `{declared}`; the catalogue's index \
         and its renderings disagree"
    )]
    Mismatched {
        /// The operation id the entry is filed under.
        operation: String,
        /// The operation the embedded Flux actually declares.
        declared: String,
    },
}

impl From<Error> for flux_core::Error {
    fn from(error: Error) -> Self {
        flux_core::Error::Config(error.to_string())
    }
}

/// The error a host gets from an operation whose request path has not been built yet.
///
/// `execute` is **C-115**. Until it lands, reaching one of these tools has to produce something,
/// and the choice is between a panic and an error. A panic is worse: it crosses into a host process
/// that did nothing wrong, and `unimplemented!()` inside a `Tool` is reachable by any model that
/// picks the operation out of the catalogue this pack just advertised.
pub fn not_wired_yet(name: &str) -> flux_core::Error {
    flux_core::Error::Config(format!(
        "`{name}` is declared but not yet wired: this connector pack projects operations onto tool \
         specs, and request execution lands in C-115. Call the operation through the connector's \
         `.flux` module until then"
    ))
}

/// **The pack.** Install every operation of each named provider into a host's registry.
///
/// The returned value is `FnOnce(&mut ToolRegistry) -> flux_core::Result<()>` — exactly what
/// `flux_sdk::ClientBuilder::try_register_pack` takes, and equally callable against a bare
/// [`ToolRegistry`]. There is deliberately no `flux-sdk` dependency here: the SDK is the host's
/// choice, not this crate's, and a pack that required it could not be used by a host that
/// assembles its registry directly.
///
/// Each provider installs under **its own source label** (`connector-pack:<provider>`) through
/// [`ToolRegistry::try_register_all_from`], which is atomic: if any of a provider's operations is
/// invalid or collides with something already registered, none of that provider lands and flux's
/// own duplicate diagnostic names both contributors. A host composing this pack with its own
/// built-ins gets that diagnostic instead of a silent override.
///
/// The provider names are copied at call time, so the returned closure is independent of the slice
/// it was built from.
///
/// # Errors
///
/// The closure returns an error when a named provider is not in the catalogue, when an operation
/// cannot be projected, or when flux refuses the registration. Nothing is installed for a provider
/// whose install failed; providers listed before it are already in, because a partially-composed
/// registry is the caller's to discard.
pub fn pack(providers: &[&str]) -> impl FnOnce(&mut ToolRegistry) -> flux_core::Result<()> {
    let requested: Vec<String> = providers.iter().map(|name| (*name).to_string()).collect();

    move |registry: &mut ToolRegistry| {
        for provider in &requested {
            install(registry, provider)?;
        }
        Ok(())
    }
}

/// Install one provider's operations under one source label.
fn install(registry: &mut ToolRegistry, provider: &str) -> flux_core::Result<()> {
    let entry =
        catalog::provider(ProviderKey::id(provider)).ok_or_else(|| Error::UnknownProvider {
            provider: provider.to_owned(),
            available: catalog::providers().len(),
        })?;

    let tools = entry
        .operations
        .iter()
        .map(|operation| Ok(Arc::new(Operation::project(operation)?) as Arc<dyn Tool>))
        .collect::<Result<Vec<_>, Error>>()?;

    registry.try_register_all_from(source_label(entry.id), tools)
}

/// The auditable label a provider's operations are registered under.
///
/// Per-provider rather than one label for the whole pack, so a duplicate diagnostic names the
/// connector that contributed the colliding operation rather than "the connector pack".
fn source_label(provider: &str) -> String {
    format!("connector-pack:{provider}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_providers_operations_are_labelled_with_the_provider() {
        let mut registry = ToolRegistry::new();
        pack(&["zendesk"])(&mut registry).expect("zendesk installs");

        assert_eq!(
            registry.source("zendesk.ticket.show"),
            Some("connector-pack:zendesk")
        );
    }

    /// The closure must not borrow the slice it was built from, or a host could not build a pack
    /// from a temporary and hand it to a builder.
    #[test]
    fn the_pack_outlives_the_names_it_was_built_from() {
        let install = {
            let names = vec!["zendesk"];
            pack(&names)
        };

        let mut registry = ToolRegistry::new();
        install(&mut registry).expect("zendesk installs");
        assert!(registry.get("zendesk.ticket.show").is_some());
    }

    /// Two providers, one registry, one call — the shape the design's example uses.
    #[test]
    fn several_providers_install_together() {
        let mut registry = ToolRegistry::new();
        pack(&["zendesk", "slack"])(&mut registry).expect("both install");

        assert!(registry.get("zendesk.ticket.show").is_some());
        assert!(registry.get("slack.chat.post.message").is_some());
        assert_eq!(
            registry.source("slack.chat.post.message"),
            Some("connector-pack:slack")
        );
    }

    #[test]
    fn an_unknown_provider_names_itself() {
        let mut registry = ToolRegistry::new();
        let error = pack(&["salesforce"])(&mut registry).expect_err("no such connector");

        assert!(error.to_string().contains("salesforce"), "{error}");
        assert!(registry.names().is_empty());
    }
}
