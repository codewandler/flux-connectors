//! Running one operation.
//!
//! The whole path is eleven lines of wiring, and that is the point: everything that makes a request
//! correct and safe already lives in `connector-pack`. This module's job is to hand it a tenant and
//! get out of the way.

use std::sync::Arc;

use catalog::{OperationKey, ProviderKey};
use connector_pack::{dotted_name, resolve, Configuration};
use serde_json::Value;

use crate::state::App;

/// What one execution produced.
#[derive(Debug, serde::Serialize)]
pub struct Outcome {
    /// The dotted tool name that ran — `zendesk.ticket.show`.
    pub tool: String,
    /// The vendor's response, **as the host's redactor renders it**.
    ///
    /// Never the raw content. A vendor that echoes a token back — several do, in an error body —
    /// would otherwise put it on this surface, so it goes through the same redactor the credential
    /// was registered with. That is the difference between "the pack kept it off the wire" and "it
    /// stayed off every surface".
    ///
    /// **Since C-403 this carries the record, not the block.** flux-web 0.43 made `http.request`'s
    /// canonical `content` the JSON-encoded `{status, headers, body}` — `body` parsed when the
    /// response is JSON — where it used to be `HTTP {status}\n{headers}\n{body}`. The pack returns
    /// it whole, so this field's *text* changed while its type and its name did not: a client of
    /// this route that scanned the block for a status line reads a JSON document now. The flat
    /// block is still produced, as flux's model-facing `view`; this surface deliberately does not
    /// carry it, because a route that published two renderings of one response would be asking
    /// every caller to choose.
    pub content: String,
    /// Whether the tool reported failure. A `404` is **not** one of these: it is a result the vendor
    /// gave, and the pack returns it unshaped.
    pub is_error: bool,
}

/// Run `operation` for `tenant`.
///
/// # Errors
///
/// Every refusal from the pack arrives here unchanged and is worth reading rather than flattening:
/// `MissingCredential` names the address an operator has to go and fill, `MissingConfig` names the
/// connection setting and the service it belongs to, and `UnredactableCredential` means the value
/// was resolved and deliberately **not sent** because the host's redactor would not hold it.
pub async fn execute(
    app: &App,
    tenant: &str,
    operation_id: &str,
    params: Value,
) -> anyhow::Result<Outcome> {
    let entry = catalog::operation(OperationKey::id(operation_id))
        .ok_or_else(|| anyhow::anyhow!("no operation `{operation_id}` in this catalogue"))?;
    // Checked here rather than left to the projection, so a catalogue naming a provider it does not
    // carry is diagnosed against the operation the caller actually asked for.
    catalog::provider(ProviderKey::id(entry.provider)).ok_or_else(|| {
        anyhow::anyhow!(
            "`{operation_id}` names provider `{}`, which this catalogue does not carry",
            entry.provider
        )
    })?;

    // Both ports, for the one tenant. Built from a single value at a single call site, which is what
    // makes `Error::TenantMismatch` unreachable here rather than merely unlikely.
    let credentials = app.credentials(tenant)?;
    let configuration = Configuration::new(
        Arc::clone(app.settings()) as Arc<dyn connector_pack::ConfigStore>,
        tenant,
    )?;

    // **Resolved by name, not looked up in the model's registry** (C-413).
    //
    // This route serves a caller that has *named* an operation, which is a different question from
    // "what may a model be offered". `pack` answers the second and withholds operations declaring
    // `expose = false`; a `ToolRegistry` is both the advertisement surface and the resolution
    // surface, so resolving through one packed here would make an unexposed operation unreachable —
    // catalogued, documented, manifest-listed and uncallable, which is the feature inverted.
    // `resolve` is the caller-facing seam and withholds nothing, under the identical admission
    // checks a packed tool passes.
    //
    // Projected per request for the reason the registry was built per request before it: it is what
    // keeps one tenant's resolved configuration from outliving the request it was read for. It is
    // also strictly less work — this projects the one operation asked for, where `pack` projected
    // and parsed every operation the provider ships.
    let tool = resolve(entry, app.egress(), credentials, configuration)?;
    let tool_name = dotted_name(entry.id)?;

    // The same `ctx` travels into `http.request`, so the redactor the credential was registered with
    // a moment ago is the one the response is rendered through below.
    let ctx = app.context();
    let result = tool.execute(&ctx, params).await?;

    Ok(Outcome {
        tool: tool_name,
        content: ctx.redactor.redact(&result.content),
        is_error: result.is_error,
    })
}
