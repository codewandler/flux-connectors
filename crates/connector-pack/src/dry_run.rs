//! **The transport seam, and the transport that cannot send** (C-145).
//!
//! # Why a seam at all, when C-115 already took the transport as an argument
//!
//! It did, and this is an implementation of that decision rather than a change to it.
//! [`crate::Egress`] is a constructor argument precisely so a host can supply the `http.request` it
//! has already configured — and its own documentation named the second consumer from the start: *"a
//! dry-run that renders the request instead of sending it, or a recorded fixture, without either
//! forking the request path"*. [`Transport`] is that seam written down, with the live arm being
//! exactly what `Operation::execute` already did.
//!
//! # Why the dry run is *not* an `Egress` holding a stand-in tool
//!
//! It is the obvious implementation and it is wrong, for one reason that no amount of care at the
//! call site fixes. `Egress` is reached at the **end** of the request path, after
//! [`crate::Operation::build_authenticated_request`] has resolved a tenant's credential out of the
//! host's secret store and placed it on the request. A stand-in tool bolted on there would report
//! the resolved plaintext — and C-159's redacting `Debug` would make it *look* handled, because a
//! redacted rendering and an absent value are indistinguishable on a screen and completely
//! different in a struct.
//!
//! So the dry run sits **upstream of resolution**: [`DryRunTransport::dry_run`] calls
//! [`crate::Operation::build_request`], which applies no credential, and then places each declared
//! credential's **reference** where its value would have gone. The secret store is never consulted,
//! there is no `ToolContext` on the path, and the guarantee is therefore a property of the code
//! path rather than of a scrub applied afterwards. A dry run answers over an empty store, which is
//! the same statement read from the other side.
//!
//! # The reference, and why it is spelled out of the unreserved alphabet
//!
//! `~credential.<name>~` — the credential's own flat-namespace name, as the connector declares it,
//! wrapped in a marker no vendor value carries. Every character is in RFC 3986's *unreserved* set,
//! which is not decoration: [`connector_resolve::auth::placed_form`]'s percent-encoder is the identity over it,
//! so a reference reads the same in a header and on a URL. That is what lets the dry run place
//! credentials through the **real** [`crate::auth::place`] — the same header names, the same
//! prefixes, the same `?`/`&` separator logic — instead of a second copy of that code that would
//! agree with it until the day it did not.
//!
//! # Which mechanism is rehearsed
//!
//! An operation's credentials are an OR over mechanisms; the live path resolves them in declared
//! order and the **first whose credentials all resolve** wins ([`crate::Credentials::resolve`]). A
//! dry run consults no store, so it cannot know which that will be, and it rehearses the **first**
//! declared mechanism — the one a connector's own documentation tells an operator to provision, and
//! the one `resolve` already quotes when every alternative is unmet.
//!
//! # The refusals are the live path's refusals
//!
//! Everything [`crate::Operation::build_request`] refuses is refused here unchanged, and so are the
//! two credential checks that need no store: a mechanism naming nothing
//! ([`Error::EmptyMechanism`]), a credential the connector does not declare
//! ([`Error::UndeclaredCredential`]) and an inbound signing secret
//! ([`Error::InboundCredential`]). The story's closing note is the reason: *a request the pack
//! cannot construct must fail loudly rather than being reported as an empty or partial dry-run*. A
//! partly-built request is a **different** call, and reporting it as the real one would be worse
//! than reporting nothing.
//!
//! What is deliberately **not** rehearsed is whether a value is actually stored. `MissingCredential`
//! is an answer about a tenant's provisioning, not about the request, and a dry run that required a
//! credential to be present before it would describe a call could not do the one job it exists for.

use async_trait::async_trait;
use catalog::Placement;
use flux_runtime::{ToolContext, ToolResult};
use serde_json::Value;

use crate::auth::{self, Assembled};
use crate::request::Request;
use crate::{Egress, Error, Operation};

/// **Where a built request goes.**
///
/// Two implementations ship and the difference between them is structural rather than
/// configurational: [`Egress`] holds the host's `http.request` and delegates to it, and
/// [`DryRunTransport`] holds nothing and therefore cannot. See this module's documentation for why
/// a third, "the live one with sending switched off", is the shape this seam exists to make
/// unnecessary.
///
/// The whole operation is handed over rather than a finished [`Request`], and that is what keeps
/// the two arms from forking the request path in the one place it matters: each transport decides
/// for itself *how far* to build. The live arm authenticates; the dry arm never calls the code that
/// would. Passing a built request instead would mean the caller deciding whether to resolve a
/// credential, which is a flag by another name.
#[async_trait]
pub trait Transport: std::fmt::Debug + Send + Sync {
    /// Carry what `operation` would send for `params`, and answer as flux's own tool result.
    ///
    /// # Errors
    ///
    /// Whatever the operation refuses while building or authenticating its request, and whatever
    /// the transport itself reports. Nothing is repaired and nothing is sent partially.
    async fn carry(
        &self,
        ctx: &ToolContext,
        operation: &Operation,
        params: &Value,
    ) -> flux_core::Result<ToolResult>;
}

/// **The live arm**: exactly what C-115 landed, moved behind the seam and otherwise unchanged.
///
/// The credential is resolved and registered with `ctx.redactor` first, the request is built and
/// authenticated, and the **same** `ctx` travels into the host's own `http.request` — so the
/// redactor, the evidence log and the cancellation token the request is made under are the host's.
/// This crate still opens no socket; it hands bytes to a tool it was given.
#[async_trait]
impl Transport for Egress {
    async fn carry(
        &self,
        ctx: &ToolContext,
        operation: &Operation,
        params: &Value,
    ) -> flux_core::Result<ToolResult> {
        let request = operation.build_authenticated_request(ctx, params).await?;
        self.send(ctx, request).await
    }
}

/// **The transport that cannot send.**
///
/// A unit struct, and that is the entire safety argument rather than a stylistic choice: a
/// zero-sized type has room for no HTTP client, no socket, no channel handle back to one, and not
/// even the `bool` that "a live client with sending switched off" would need. The property is
/// therefore checkable by construction — `size_of::<DryRunTransport>() == 0` — and
/// `tests/dry_run.rs` checks it, with `Egress`'s own non-zero size as the control.
///
/// [`DryRunTransport::new`] takes no arguments for the same reason. It is not a convenience: it is
/// the line that stops compiling the day this type grows a field to fill.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DryRunTransport;

impl DryRunTransport {
    /// The dry-run transport. Takes nothing, because there is nothing it could be given.
    pub fn new() -> Self {
        Self
    }

    /// **What `operation` would send for `params`, without sending it.**
    ///
    /// The request is built by the same evaluator the live path uses ([`crate::request`]), so a dry
    /// run is the module's own request rather than a description of it. Each credential the first
    /// declared mechanism names is then placed by the same [`crate::auth::place`] the live path
    /// uses — carrying its **reference** instead of its value. See this module's documentation.
    ///
    /// # Errors
    ///
    /// Everything [`Operation::build_request`] refuses, plus the store-free credential refusals:
    /// [`Error::EmptyMechanism`], [`Error::UndeclaredCredential`], [`Error::InboundCredential`] and
    /// [`Error::CredentialCollision`]. None of them is reported as a partial dry run.
    pub fn dry_run(&self, operation: &Operation, params: &Value) -> Result<DryRun, Error> {
        let entry = operation.entry();
        let mut request = operation.build_request(params)?;
        let mut credentials = Vec::new();

        for name in first_mechanism(entry)? {
            let credential = operation.provider().credential(name).ok_or_else(|| {
                Error::UndeclaredCredential {
                    operation: entry.id.to_owned(),
                    credential: (*name).to_owned(),
                    provider: entry.provider.to_owned(),
                }
            })?;

            // Refused before anything is placed, exactly as `resolve_mechanism` refuses it before
            // the store is touched: a signing secret verifies bytes that arrived, and a rehearsal
            // that quietly left it out would describe a call the pack will not make.
            if matches!(credential.place, Placement::Inbound) {
                return Err(Error::InboundCredential {
                    operation: entry.id.to_owned(),
                    credential: credential.name.to_owned(),
                });
            }

            let reference = reference(credential.name);
            auth::place(
                entry.id,
                &Assembled::new(credential.name, reference.clone(), credential.place),
                &mut request,
            )?;
            credentials.push(CredentialReference {
                credential: credential.name,
                place: credential.place,
                reference,
            });
        }

        Ok(DryRun {
            operation: entry.id,
            tool: operation.spec_name().to_owned(),
            request,
            credentials,
        })
    }
}

#[async_trait]
impl Transport for DryRunTransport {
    /// The `ctx` is taken and not used, and that is the point rather than an oversight: a transport
    /// whose signature differed from the live one would fork the request path, which is what this
    /// seam exists to avoid. Nothing on this arm has a use for a redactor, because nothing on it
    /// resolves a value that would need one.
    async fn carry(
        &self,
        _ctx: &ToolContext,
        operation: &Operation,
        params: &Value,
    ) -> flux_core::Result<ToolResult> {
        Ok(ToolResult::ok(self.dry_run(operation, params)?.render()))
    }
}

/// The mechanism a dry run rehearses: the first declared one, or none for an operation that
/// authenticates with nothing.
///
/// # Errors
///
/// [`Error::EmptyMechanism`] for a mechanism naming no credentials — it would authenticate nothing
/// while looking satisfied, which is the one failure shape that resembles success. The loader
/// refuses a degenerate mechanism and [`crate::Credentials::resolve_mechanism`] refuses it again;
/// this is the third lock, on the surface an operator reads *before* provisioning anything.
fn first_mechanism(entry: &'static catalog::Operation) -> Result<&'static [&'static str], Error> {
    let Some(mechanism) = entry.credentials.first() else {
        return Ok(&[]);
    };
    if mechanism.is_empty() {
        return Err(Error::EmptyMechanism {
            operation: entry.id.to_owned(),
        });
    }
    Ok(mechanism)
}

/// The text a dry run puts where a credential's value would go.
///
/// See this module's documentation for why every character is in RFC 3986's unreserved set. The
/// name is the connector's own — `slack.bot_token`, not a leaf and not an address — because that is
/// the spelling an operation's `credentials` declares and the one a connector's documentation uses.
fn reference(credential: &str) -> String {
    format!("~credential.{credential}~")
}

/// **A credential a rehearsed call would carry, named rather than resolved.**
///
/// Constructed only by [`DryRunTransport::dry_run`], and the fields are private for that reason: a
/// value of this type cannot be assembled from a secret, so "a dry run holds no credential value"
/// stays a property of the type rather than a habit of its call sites.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CredentialReference {
    credential: &'static str,
    place: Placement,
    reference: String,
}

impl CredentialReference {
    /// The credential's flat-namespace name, as the connector declares it — `slack.bot_token`.
    pub fn credential(&self) -> &'static str {
        self.credential
    }

    /// Where it would go on the request: a header and its prefix, or a query parameter.
    pub fn place(&self) -> Placement {
        self.place
    }

    /// The text standing in for the value, as it appears on the rehearsed request.
    pub fn reference(&self) -> &str {
        &self.reference
    }
}

/// **What an operation would send** — the answer a dry run gives.
///
/// The request here is the module's own request with credential *references* where values would
/// have been. It is not a redacted request: no value was ever resolved, so there is nothing behind
/// the reference to leak through a field access, a serialization or a log line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DryRun {
    operation: &'static str,
    tool: String,
    request: Request,
    credentials: Vec<CredentialReference>,
}

impl DryRun {
    /// The catalogue operation that was rehearsed — `zendesk-ticket-show`.
    pub fn operation(&self) -> &'static str {
        self.operation
    }

    /// The dotted tool name a host calls it by — `zendesk.ticket.show`.
    pub fn tool(&self) -> &str {
        &self.tool
    }

    /// The request itself: method, URL, headers and body.
    pub fn request(&self) -> &Request {
        &self.request
    }

    /// Every credential the rehearsed call would carry, in the order they were placed.
    pub fn credentials(&self) -> &[CredentialReference] {
        &self.credentials
    }

    /// The dry run as JSON.
    ///
    /// Hand-assembled rather than derived, for the same reason [`Request::to_params`] is: this
    /// crate takes no `serde` derive dependency, and the shape a host renders is worth stating
    /// once, in the open, next to the thing that guarantees what is in it.
    pub fn to_json(&self) -> Value {
        let credentials: Vec<Value> = self
            .credentials
            .iter()
            .map(|reference| {
                let (place, target, prefix) = match reference.place {
                    Placement::Header { name, prefix } => ("header", name, prefix),
                    Placement::Query { name } => ("query", name, ""),
                    // Refused by `dry_run` before a reference is ever built, so this arm is
                    // unreachable — stated rather than `unreachable!`, because a panic in a
                    // rendering is a worse answer than a truthful word.
                    Placement::Inbound => ("inbound", reference.credential, ""),
                };
                serde_json::json!({
                    "credential": reference.credential,
                    "reference": reference.reference,
                    "place": place,
                    "target": target,
                    "prefix": prefix,
                })
            })
            .collect();

        serde_json::json!({
            "operation": self.operation,
            "tool": self.tool,
            "request": self.request.to_params(),
            "credentials": credentials,
        })
    }

    /// The dry run as the text a transport answers with — [`DryRun::to_json`], pretty-printed.
    pub fn render(&self) -> String {
        serde_json::to_string_pretty(&self.to_json()).unwrap_or_else(|_| self.to_json().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The reference is stable under every placement the catalogue can declare.**
    ///
    /// A query placement percent-encodes, and the reason the dry run can go through the real
    /// [`auth::place`] rather than a copy of it is that the encoder is the identity over this
    /// alphabet. Asserted against [`connector_resolve::auth::placed_form`] itself — the single answer
    /// to "does this placement transform the value" — so a change to the encoder fails here rather
    /// than quietly producing two spellings of one reference.
    #[test]
    fn a_reference_is_unchanged_by_the_placement_encoder() {
        let reference = reference("acme.token");
        assert_eq!(reference, "~credential.acme.token~");
        assert_eq!(
            connector_resolve::auth::placed_form(Placement::Query { name: "api_key" }, &reference)
                .as_deref(),
            Some(reference.as_str()),
            "a query placement escapes the reference, so it reads differently on a URL"
        );
    }

    /// The size claim, stated where the type is defined as well as where the story asks for it. A
    /// field added here is a compile-time-sized failure rather than a review-time one.
    #[test]
    fn the_dry_run_transport_is_zero_sized() {
        assert_eq!(std::mem::size_of::<DryRunTransport>(), 0);
    }
}
