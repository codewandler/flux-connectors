//! **The plan-deriving core**: a connector's canonical document in, its HTTP request plan out.
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use std::collections::BTreeMap;
//!
//! let zendesk = connector_resolve::document::provider("zendesk").expect("a shipped connector");
//! let operation = zendesk.operation("zendesk-ticket-show").expect("a shipped operation");
//! let base = zendesk.base_url(&operation.service).expect("its service");
//!
//! let plan = connector_resolve::resolve(
//!     operation,
//!     base,
//!     &serde_json::json!({ "ticket_id": 35436 }),
//!     &BTreeMap::from([("subdomain".to_string(), "acme".to_string())]),
//!     &[],
//! )?;
//!
//! assert_eq!(plan.request.method, "GET");
//! assert_eq!(plan.request.url, "https://acme.zendesk.com/api/v2/tickets/35436");
//! # Ok(())
//! # }
//! ```
//!
//! # Engine-free, and that is the point rather than a coincidence
//!
//! `codewandler-connector-pack`'s `resolve` returns `flux_core::Result<Arc<dyn Tool>>`, which
//! couples every consumer to one `codewandler-flux-*` line even when no catalogue content changed
//! (`docs/designs/catalog-artifact.md` §3). This crate is the half of that work which has no engine
//! in it: it links no `codewandler-flux-*` crate, and
//! `crates/connector-cli/tests/main/engine_free_core.rs::the_plan_deriving_core_links_no_engine_crate`
//! asserts that over the graph cargo resolves — kinds filtered to the edges a *consumer* links —
//! rather than over this paragraph.
//!
//! It also opens no socket, holds no HTTP client and resolves no host. Dispatch is the consumer's:
//! `connector-pack` wraps a plan in a flux `Tool`, and Exchange wraps one in its own workflows.
//!
//! # One boundary a consumer must hold
//!
//! **Projecting a plan into a Tool is not composing a request.** A consumer wraps and dispatches
//! the plan it was handed; a consumer that *edits* one has become a second request-composition
//! path, which is the defect this whole design exists to end.
//!
//! # What is derived here, and what is the caller's
//!
//! Derived here: the method, the URL with its structured query, the headers, the body, the
//! permission subjects, and the set of strings a redactor must hold. Its two data inputs — the
//! tenant's resolved endpoint map and the assembled credentials — are **produced here too** as of
//! C-557, from bound ports the caller implements: [`resolve_endpoints`] over a [`ConfigPort`], and
//! [`assemble_credentials`] over a [`connector_secrets::SecretStore`]. The ports say *what this
//! tenant's value is*; the producers apply the enforcement — declared defaults, operator approval,
//! origin normalisation, mechanism selection, the acquisition axis, the redaction set — on top. A
//! crate that reached past the ports for a value would be a crate that had opinions about where a
//! tenant's secrets live, which is why the ports exist.
//!
//! The producers are the whole point of the split for a host: they were `pub(crate)` in the
//! flux-coupled `connector-pack`, so no engine-free consumer could produce a plan. `connector-pack`'s
//! own `Configuration`/`Credentials` producers now delegate here, and the whole-catalogue
//! differential gate holds the two to one plan.
//!
//! # The refusals are the safety property
//!
//! Every [`Error`] variant refuses; none repairs. A partly-evaluated request is not a degraded
//! request — it is a different call, and the vendor at the other end answers it.

#![deny(missing_docs)]

pub mod auth;
mod channel;
mod config;
mod credentials;
pub mod document;
mod endpoints;
mod error;
mod plan;
mod request;
mod resolve;
mod slot;
mod template;

pub use channel::{channel_plan, PreparedChannelPlan};
pub use config::{ConfigField, ConfigPort, ConfigValue};
pub use credentials::{assemble_credentials, Assembly, Redaction};
pub use endpoints::{resolve_endpoint, resolve_endpoints};
pub use error::Error;
pub use plan::{RequestPlan, SensitiveText};
pub use request::{Request, DEFAULT_USER_AGENT};
pub use resolve::{build_request, resolve};
pub use slot::{
    validate_authority, validate_header, validate_path, validate_query,
    validate_templated_authority, Slot,
};
pub use template::{scan_template, text, truthy};
