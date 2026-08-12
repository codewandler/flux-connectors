//! **A connector's request, rehearsed from its canonical document** (C-538).
//!
//! The document-backed equivalent of [`Rehearsal`](crate::Rehearsal), with the same observable
//! semantics, exported beside it until Exchange's settings and connection-verification paths
//! migrate (C-539). The two answer the same questions — the contract, whether it is exposed, the
//! configuration surface, and the request over a bound [`Configuration`] — and
//! `tests/catalogue_differential.rs` requires them to answer identically for every operation in the
//! catalogue.
//!
//! # What is different, and why it is better rather than merely other
//!
//! [`Rehearsal::of`](crate::Rehearsal::of) takes the operation's **emitted Flux** as text, which is
//! what made it usable from a provider story before a full build: no index, no whole-catalogue
//! artifact. This one takes the operation **id**, because the canonical documents are the artifact
//! a scoped `flux-connectors build --provider <id>` writes, and the pack the reader embeds is built
//! from them. So the same story-time question — *does this connector compose a request at all* — is
//! still askable, and the answer no longer depends on re-parsing a text this repository is
//! retiring.
//!
//! It is **the same code path**, not a parallel one: [`DocumentRehearsal::request`] resolves values
//! through the same [`Configuration`] port a host binds and then calls the same
//! [`connector_resolve::build_request`] [`Operation::build_request`](crate::Operation::build_request)
//! calls, with the same variables, the same slots and the same refusals. A rehearsal that passed
//! while the real path failed would be worse than no rehearsal at all.
//!
//! What it does not cover is what needs the catalogue rather than the document: credential placement
//! (the connector's `[[auth]]` table), the network gate's declared hosts, and the dotted tool name's
//! collision behaviour across a whole pack.

use std::collections::{BTreeMap, BTreeSet};

use connector_resolve::{Request, Slot};
use flux_spec::ToolSpec;
use serde_json::Value;

use crate::config::{Configuration, Field};
use crate::{base_url_of, document_of, spec, Error};

/// One operation, rehearsed from its canonical document.
///
/// Built once and asked many times, for the same reason [`crate::Operation`] is: the document is
/// looked up, the contract projected and the configuration surface read at construction, so a
/// connector that cannot be rehearsed is a refusal *here* rather than at whichever call happened to
/// come first.
#[derive(Debug, Clone)]
pub struct DocumentRehearsal {
    /// The catalogue entry, for the provider/service key a configuration lookup needs and for the
    /// contract projection.
    entry: &'static catalog::Operation,
    /// The operation's canonical document.
    document: &'static connector_resolve::document::Operation,
    /// The service base URL template, before this tenant's connection settings fill it.
    base_url: &'static str,
    /// The contract a host would register this operation by.
    spec: ToolSpec,
}

impl DocumentRehearsal {
    /// Rehearse the operation `id`, from the document the build wrote for it.
    ///
    /// # Errors
    ///
    /// [`Error::Unbuildable`] when the embedded catalogue carries no document for `id` or its
    /// provider declares no such service, and everything [`crate::project`] refuses — an id with no
    /// dotted tool name, or a spec flux itself would not register.
    pub fn of(id: &str) -> Result<Self, Error> {
        let entry = catalog::operation(catalog::OperationKey::id(id)).ok_or_else(|| {
            Error::Unbuildable {
                operation: id.to_owned(),
                message: "this catalogue carries no operation by that id".to_owned(),
            }
        })?;
        Ok(Self {
            document: document_of(entry.id)?,
            base_url: base_url_of(entry)?,
            spec: spec::project(entry)?,
            entry,
        })
    }

    /// The catalogue entry behind this rehearsal.
    pub fn entry(&self) -> &'static catalog::Operation {
        self.entry
    }

    /// The contract a host would register this operation by.
    pub fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    /// **Whether a host would register that contract at all** (C-413), as the document states it.
    ///
    /// Unexposed is not uncallable: [`Self::request`] composes this operation's request either way,
    /// and only [`crate::pack`] consults this.
    pub fn is_exposed(&self) -> bool {
        self.document.expose
    }

    /// The configuration variables this operation's request needs, in stable order.
    pub fn endpoint_variables(&self) -> &[String] {
        self.document.endpoint_variables()
    }

    /// Where each of them lands on the request.
    pub fn endpoint_slots(&self) -> &BTreeMap<String, Slot> {
        self.document.endpoint_slots()
    }

    /// Caller-visible parameters the request template places in a URL path segment.
    pub fn caller_path_parameters(&self) -> &BTreeSet<String> {
        self.document.caller_path_parameters()
    }

    /// **The caller-facing name of each declared parameter**, in declaration order.
    ///
    /// The document publishes the IR name (`time.start`) and the wire name; the name a *caller*
    /// addresses is the Flux symbol the emitted declaration declares (`time_start`), which the
    /// document does not carry. `connector-resolve` reproduces that allocation, and
    /// `tests/catalogue_differential.rs` holds the reproduction to the declaration for every
    /// operation in the catalogue.
    pub fn caller_parameters(&self) -> Vec<&str> {
        self.document.caller_parameters()
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
    /// needs, plus everything [`crate::Operation::build_request`] refuses.
    pub fn request(&self, configuration: &Configuration, params: &Value) -> Result<Request, Error> {
        let variables = self.endpoint_variables();
        let settings = configuration.snapshot(
            self.entry.provider,
            self.entry.service,
            variables.iter().map(|name| Field::from_placeholder(name)),
        );
        let endpoints = variables
            .iter()
            .map(|variable| {
                let value = settings.require(self.entry.id, Field::from_placeholder(variable))?;
                Ok((variable.clone(), value))
            })
            .collect::<Result<BTreeMap<String, String>, Error>>()?;

        Ok(connector_resolve::build_request(
            self.document,
            self.base_url,
            params,
            &endpoints,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{test_configuration, TEST_TENANT};

    #[test]
    fn a_rehearsal_composes_the_request_its_document_describes() {
        let rehearsal = DocumentRehearsal::of("zendesk-ticket-update").expect("it rehearses");
        assert_eq!(rehearsal.spec().name, "zendesk.ticket.update");
        assert!(rehearsal.is_exposed());
        assert_eq!(rehearsal.endpoint_variables(), ["subdomain"]);
        assert_eq!(rehearsal.endpoint_slots()["subdomain"], Slot::Host);
        assert!(rehearsal.caller_path_parameters().contains("ticket_id"));

        let request = rehearsal
            .request(
                &test_configuration(),
                &serde_json::json!({"ticket_id": 35436, "ticket": {"status": "open"}}),
            )
            .expect("it composes");
        assert_eq!(request.method, "PUT");
        assert_eq!(request.url, "https://acme.zendesk.com/api/v2/tickets/35436");
    }

    /// The production shape for an unconfigured tenant: a refusal naming the field, not a request
    /// to a host with a brace in it.
    #[test]
    fn an_unconfigured_tenant_is_a_refusal_that_names_the_field() {
        let rehearsal = DocumentRehearsal::of("zendesk-ticket-show").expect("it rehearses");
        let empty =
            Configuration::new(std::sync::Arc::new(crate::MemoryConfig::new()), TEST_TENANT)
                .expect("a valid tenant id");
        let refusal = rehearsal
            .request(&empty, &serde_json::json!({"ticket_id": 1}))
            .expect_err("it refuses");
        assert!(
            matches!(refusal, Error::MissingConfig { ref field, .. } if field == "endpoint.subdomain"),
            "{refusal}"
        );
    }

    #[test]
    fn an_operation_this_catalogue_does_not_carry_is_reported() {
        assert!(matches!(
            DocumentRehearsal::of("no-such-operation"),
            Err(Error::Unbuildable { .. })
        ));
    }
}
