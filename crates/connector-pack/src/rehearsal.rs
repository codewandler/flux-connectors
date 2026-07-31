//! **A connector's request, rehearsed from its emitted Flux alone** (C-233).
//!
//! # The question a provider story could not ask
//!
//! Every other entry point here takes a `&'static catalog::Operation`, and that is three walls at
//! once for someone who has just written `providers/<id>.toml`:
//!
//! - [`catalog::Operation`] is `#[non_exhaustive]`, so no synthetic one can be constructed outside
//!   the `catalog` crate;
//! - the only real ones come from `crates/catalog/src/generated.rs`, which is a **whole-catalogue**
//!   artifact — coordinator-owned, written by a full build, and therefore not carrying a new
//!   provider until integration ([`AGENTS.md`](../../../AGENTS.md));
//! - [`Operation::project`](crate::Operation::project) additionally looks the *provider* up in that
//!   same index, so even a synthetic entry would be refused with
//!   [`Error::UnknownProvider`](crate::Error::UnknownProvider).
//!
//! So "does this connector compose a request at all" — the one question worth asking first, and the
//! one C-110 answered *no* to after shipping eight operations through a fully green gate — was
//! structurally unaskable until integration.
//!
//! # What this does instead, and what it deliberately does not reopen
//!
//! A rehearsal takes the operation's **own emitted Flux**: the text
//! `crates/catalog/ops/<provider>/<operation>.flux` carries, which a **scoped**
//! `flux-connectors build --provider <id>` writes. No index, no whole-catalogue artifact, nothing a
//! story implementor is fenced away from.
//!
//! It does **not** add a builder or a test-only constructor for [`catalog::Operation`], and that is
//! the point rather than an omission. `#[non_exhaustive]` on that struct is protecting one specific
//! thing: that C-37's global address and C-10's resolved endpoint spec can land as new fields
//! without breaking a consumer, because no consumer can write a struct literal or destructure one
//! exhaustively. A constructor taking every field would break on the next field exactly as a struct
//! literal does; a builder would not, but it would still put the burden of keeping a synthetic entry
//! *plausible* — hosts, credentials, service — on whoever writes the test, and a wrong synthetic
//! entry is a test that measures a connector nobody ships. Taking the Flux and nothing else means
//! there is no synthetic entry to get wrong, and the attribute keeps its full guarantee.
//!
//! # It is the same code path, not a parallel one
//!
//! [`Rehearsal::request`] resolves values through the same [`Configuration`] port a host binds and
//! then calls the same [`crate::request`] evaluator [`Operation::build_request`] calls, with the
//! same variables, the same slots and the same refusals. A rehearsal that passed while the real path
//! failed would be worse than no rehearsal at all.
//!
//! What it does not cover is what needs the catalogue rather than the module: credential placement
//! (the connector's `[[auth]]` table), the network gate's declared hosts, and the dotted tool name's
//! collision behaviour across a whole pack. Those arrive at integration, where the index does.

use std::collections::BTreeMap;

use flux_lang::program::CompositeOpDecl;
use flux_spec::ToolSpec;
use serde_json::Value;

use crate::config::{Configuration, Field};
use crate::request::{self, Request, Slot};
use crate::{spec, Error};

/// One operation, rehearsed from its emitted Flux.
///
/// Built once and asked many times, for the same reason [`crate::Operation`] is: the declaration is
/// parsed, the spec projected and the configuration variables derived at construction, so a
/// connector that cannot be projected is a refusal *here* rather than at whichever call happened to
/// come first.
#[derive(Debug, Clone)]
pub struct Rehearsal {
    /// The operation id, for a refusal that names what it refused.
    operation: String,
    /// The connector this operation belongs to — the `provider` half of a configuration key.
    provider: String,
    /// The service it belongs to — `default` for a connector with a single API surface. Load-bearing
    /// rather than decoration: a service owns its `base_url`, so a `{variable}` in one belongs to
    /// that service (C-197).
    service: String,
    /// The parsed declaration the request is evaluated from.
    declaration: CompositeOpDecl,
    /// The contract a host would register this operation by.
    spec: ToolSpec,
    /// The configuration variables this operation's own emitted Flux carries.
    variables: Vec<String>,
    /// Where each of them lands on the request (C-214).
    slots: BTreeMap<String, Slot>,
}

impl Rehearsal {
    /// Rehearse `operation` of `provider`'s `service`, from the Flux the emitter wrote for it.
    ///
    /// `flux` is one `op` declaration — exactly what `crates/catalog/ops/<provider>/<id>.flux`
    /// contains, and exactly the substring `connectors/<provider>.flux` carries for it.
    ///
    /// # Errors
    ///
    /// Everything [`crate::spec::project`] refuses — an id with no dotted tool name, Flux that is not
    /// a single `op` declaration, a spec flux would not register — plus
    /// [`Error::Unbuildable`] for a body binding a brace-carrying literal this pack cannot classify
    /// as configuration (C-232), which is the refusal C-110 needed and could not reach.
    pub fn of(operation: &str, provider: &str, service: &str, flux: &str) -> Result<Self, Error> {
        let declaration = spec::declaration_of(operation, flux)?;
        let spec = spec::project_declaration(operation, &declaration)?;
        request::refuse_unconfigurable(operation, &declaration)?;

        Ok(Self {
            operation: operation.to_owned(),
            provider: provider.to_owned(),
            service: service.to_owned(),
            variables: request::endpoint_variables(&declaration),
            slots: request::endpoint_slots(&declaration),
            declaration,
            spec,
        })
    }

    /// The contract a host would register this operation by.
    pub fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    /// The configuration variables this operation's URL carries, in stable order — the same answer
    /// [`crate::Operation::endpoint_variables`] gives for a shipped one.
    pub fn endpoint_variables(&self) -> &[String] {
        &self.variables
    }

    /// **The request this operation makes**, over the configuration `configuration` holds.
    ///
    /// The port is read exactly as [`crate::Operation::project`] reads it — one snapshot, keyed by
    /// `(tenant, provider, service, endpoint, name)` — so a value stored under the wrong service is
    /// as invisible here as it would be in production.
    ///
    /// # Errors
    ///
    /// [`Error::MissingConfig`] when the tenant supplies no value for a variable this operation
    /// needs — the production shape for a connector whose `[[config]]` declares nothing that binds
    /// one — plus everything [`crate::Operation::build_request`] refuses.
    pub fn request(&self, configuration: &Configuration, params: &Value) -> Result<Request, Error> {
        let settings = configuration.snapshot(
            &self.provider,
            &self.service,
            self.variables.iter().map(|name| Field::Endpoint(name)),
        );
        let endpoints = self
            .variables
            .iter()
            .map(|variable| {
                let value = settings.require(&self.operation, Field::Endpoint(variable))?;
                Ok((variable.clone(), value))
            })
            .collect::<Result<BTreeMap<String, String>, Error>>()?;

        request::build(
            &self.operation,
            &self.declaration,
            params,
            &endpoints,
            &self.slots,
        )
    }
}
