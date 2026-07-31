//! Running one operation.
//!
//! The whole path is eleven lines of wiring, and that is the point: everything that makes a request
//! correct and safe already lives in `connector-pack`. This module's job is to hand it a tenant and
//! get out of the way.

use std::sync::Arc;

use catalog::{OperationKey, ProviderKey};
use connector_pack::{dotted_name, pack, Configuration};
use flux_runtime::ToolRegistry;
use serde_json::Value;

use crate::state::App;

/// What one execution produced.
#[derive(Debug, serde::Serialize)]
pub struct Outcome {
    /// The dotted tool name that ran — `zendesk.ticket.show`.
    pub tool: String,
    /// The vendor's response, **as the host's redactor renders it**.
    ///
    /// Never the raw content. `http.request` returns one flat string
    /// (`HTTP {status}\n{headers}\n{body}`) and returns it whole, and a vendor that echoes a token
    /// back — several do, in an error body — would otherwise put it on this surface. Passing it
    /// through the same redactor the credential was registered with is the difference between
    /// "the pack kept it off the wire" and "it stayed off every surface".
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
    let provider = catalog::provider(ProviderKey::id(entry.provider)).ok_or_else(|| {
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

    // A fresh registry per request. It is cheap — projection parses the operation's Flux — and it is
    // what keeps one tenant's resolved configuration from outliving the request it was read for.
    let mut registry = ToolRegistry::new();
    pack(&[provider.id], app.egress(), credentials, configuration)(&mut registry)?;

    let tool_name = dotted_name(entry.id)?;
    let tool = registry.get(&tool_name).ok_or_else(|| {
        anyhow::anyhow!("`{tool_name}` did not register, though its provider installed")
    })?;

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
