//! The HTTP surface.
//!
//! # The tenant
//!
//! Every handler below takes its tenant from [`tenant_of`], which is a single constant until
//! sign-in lands. It is threaded as a parameter from the first commit rather than added later,
//! because "the tenant comes from the session" is a property that has to hold at every call site,
//! and retrofitting it is how one of them gets missed.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use catalog::{Acquisition, OperationKey, Placement, ProviderKey};
use connector_pack::{CredentialRef, Secret, DEFAULT_SERVICE};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::exec;
use crate::state::App;

/// The single tenant this host serves until sign-in lands (slice 3).
///
/// Named rather than inlined so that the moment a session exists, the compiler points at every
/// place that has to change.
const SOLE_TENANT: &str = "local";

/// Whose data a request is about.
///
/// # This is the confused-deputy seam
///
/// A host that resolves the tenant from anything a caller controls — a path segment, a body field,
/// a header — is a service that adds authority to whoever asks. `docs/designs/connectors-proxy.md`
/// rejected exactly that shape. When sign-in lands this function reads the session and nothing else,
/// and it is a function rather than an inline constant so there is one place for that to be true.
fn tenant_of() -> &'static str {
    SOLE_TENANT
}

/// An error, as JSON, with a status.
///
/// Refusals from the pack are the useful half of this host and are passed through verbatim: they
/// name the missing fact — the credential address, the unbound configuration field and its service —
/// rather than reducing to "500". None of them carries a credential value.
pub struct Failure(StatusCode, String);

impl axum::response::IntoResponse for Failure {
    fn into_response(self) -> axum::response::Response {
        (self.0, Json(serde_json::json!({ "error": self.1 }))).into_response()
    }
}

impl From<anyhow::Error> for Failure {
    fn from(error: anyhow::Error) -> Self {
        // `{error:#}` renders the whole chain, which is where the pack's own diagnostic lives.
        Failure(StatusCode::BAD_REQUEST, format!("{error:#}"))
    }
}

// ---------------------------------------------------------------------------------------------
// The catalogue
// ---------------------------------------------------------------------------------------------

/// One connector, as the connect page needs it.
#[derive(Serialize)]
pub struct ConnectorView {
    id: &'static str,
    vendor: &'static str,
    description: &'static str,
    authority: Option<&'static str>,
    base_url: &'static str,
    operation_count: usize,
    /// Every operation id, so the page can fetch their detail without a second index.
    operation_ids: Vec<&'static str>,
    /// Whether every credential this connector declares is stored for this tenant.
    connected: bool,
    credentials: Vec<CredentialView>,
    /// Configuration fields with a value, as `("default/endpoint.subdomain", "acme")`.
    settings: Vec<(String, String)>,
}

/// One credential a connector declares — **its address and its shape, never its value**.
#[derive(Serialize)]
pub struct CredentialView {
    /// The flat name an operation references, e.g. `zendesk.api_token`.
    name: &'static str,
    /// The last segment of its address.
    leaf: &'static str,
    /// Where the pack will put it: `header:Authorization`, `query:api_key`, or `inbound`.
    placement: String,
    /// Whether it also needs a non-secret user half through the configuration port.
    needs_username: bool,
    /// The rendered address an operator stores it at.
    address: Option<String>,
    /// Whether a value is stored there. Never what.
    stored: bool,
}

/// Every connector in the catalogue.
pub async fn connectors(State(app): State<App>) -> Result<Json<Vec<ConnectorView>>, Failure> {
    let tenant = tenant_of();
    let mut views = Vec::new();
    for provider in catalog::providers() {
        views.push(view_of(&app, tenant, provider).await?);
    }
    Ok(Json(views))
}

/// One connector.
pub async fn connector(
    State(app): State<App>,
    Path(provider): Path<String>,
) -> Result<Json<ConnectorView>, Failure> {
    let entry = catalog::provider(ProviderKey::id(&provider)).ok_or_else(|| {
        Failure(
            StatusCode::NOT_FOUND,
            format!("no connector `{provider}` in this catalogue"),
        )
    })?;
    Ok(Json(view_of(&app, tenant_of(), entry).await?))
}

async fn view_of(
    app: &App,
    tenant: &str,
    provider: &'static catalog::Provider,
) -> Result<ConnectorView, Failure> {
    let mut credentials = Vec::new();
    let mut all_stored = true;
    for credential in provider.auth {
        let address = provider.authority.and_then(|authority| {
            CredentialRef::new(tenant, authority, DEFAULT_SERVICE, credential.leaf).ok()
        });
        let stored = match &address {
            Some(reference) => app.has_secret(reference).await.map_err(|error| {
                Failure(StatusCode::BAD_GATEWAY, format!("secret store: {error}"))
            })?,
            None => false,
        };
        // An inbound signing secret never leaves, so it is not part of "can this connector call
        // out". Counting it would show a connector as unconnectable for want of a value no outgoing
        // request would ever carry.
        if !stored && !matches!(credential.place, Placement::Inbound) {
            all_stored = false;
        }
        credentials.push(CredentialView {
            name: credential.name,
            leaf: credential.leaf,
            placement: match credential.place {
                Placement::Header { name, .. } => format!("header:{name}"),
                Placement::Query { name } => format!("query:{name}"),
                Placement::Inbound => "inbound".to_owned(),
            },
            needs_username: matches!(credential.acquire, Acquisition::BasicJoin { .. }),
            address: address.as_ref().map(|reference| {
                connector_pack::Layout::render(&connector_pack::TenantLayout, reference)
            }),
            stored,
        });
    }

    Ok(ConnectorView {
        id: provider.id,
        vendor: provider.vendor,
        description: provider.description,
        authority: provider.authority,
        base_url: provider.base_url,
        operation_count: provider.operations.len(),
        operation_ids: provider.operations.iter().map(|op| op.id).collect(),
        connected: all_stored && !provider.auth.is_empty(),
        credentials,
        settings: app.settings().bound_for(tenant, provider.id),
    })
}

/// One operation, with the Flux it was compiled from.
#[derive(Serialize)]
pub struct OperationView {
    id: &'static str,
    provider: &'static str,
    service: &'static str,
    description: &'static str,
    risk: String,
    idempotency: String,
    hosts: &'static [&'static str],
    credentials: &'static [&'static [&'static str]],
    /// The dotted name it registers under, which is what a caller actually invokes.
    tool: String,
    /// The operation's own emitted Flux — the human-readable contract behind the request.
    flux: &'static str,
}

/// One operation.
pub async fn operation(Path(operation): Path<String>) -> Result<Json<OperationView>, Failure> {
    let entry = catalog::operation(OperationKey::id(&operation)).ok_or_else(|| {
        Failure(
            StatusCode::NOT_FOUND,
            format!("no operation `{operation}` in this catalogue"),
        )
    })?;
    let tool = connector_pack::dotted_name(entry.id)
        .map_err(|error| Failure(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    Ok(Json(OperationView {
        id: entry.id,
        provider: entry.provider,
        service: entry.service,
        description: entry.description,
        risk: format!("{:?}", entry.risk).to_lowercase(),
        idempotency: format!("{:?}", entry.idempotency).to_lowercase(),
        hosts: entry.hosts,
        credentials: entry.credentials,
        tool,
        flux: entry.flux,
    }))
}

// ---------------------------------------------------------------------------------------------
// What an operator supplies
// ---------------------------------------------------------------------------------------------

/// A credential value on its way in. One direction only — nothing sends this shape back.
#[derive(Deserialize)]
pub struct CredentialInput {
    value: String,
}

/// Store one credential for this tenant.
///
/// **The response never carries the value, on any path including failure.** That is asserted by
/// `tests/credentials_never_echo.rs` rather than left to review, because the natural shape of an
/// error message — "could not store `<value>`" — is exactly the mistake.
pub async fn put_credential(
    State(app): State<App>,
    Path((provider, credential)): Path<(String, String)>,
    Json(input): Json<CredentialInput>,
) -> Result<StatusCode, Failure> {
    let tenant = tenant_of();
    let entry = catalog::provider(ProviderKey::id(&provider)).ok_or_else(|| {
        Failure(
            StatusCode::NOT_FOUND,
            format!("no connector `{provider}` in this catalogue"),
        )
    })?;
    let declared = entry.credential(&credential).ok_or_else(|| {
        Failure(
            StatusCode::NOT_FOUND,
            format!("`{provider}` declares no credential `{credential}`"),
        )
    })?;
    let authority = entry.authority.ok_or_else(|| {
        Failure(
            StatusCode::CONFLICT,
            format!("`{provider}` declares no authority, so its credentials have no address"),
        )
    })?;
    let reference = CredentialRef::new(tenant, authority, DEFAULT_SERVICE, declared.leaf)
        .map_err(|reason| Failure(StatusCode::BAD_REQUEST, reason))?;

    app.put_secret(&reference, Secret::new(input.value))
        .await
        .map_err(|error| Failure(StatusCode::BAD_GATEWAY, format!("secret store: {error}")))?;

    Ok(StatusCode::NO_CONTENT)
}

/// Forget one credential.
pub async fn delete_credential(
    State(app): State<App>,
    Path((provider, credential)): Path<(String, String)>,
) -> Result<StatusCode, Failure> {
    let tenant = tenant_of();
    let entry = catalog::provider(ProviderKey::id(&provider)).ok_or_else(|| {
        Failure(
            StatusCode::NOT_FOUND,
            format!("no connector `{provider}` in this catalogue"),
        )
    })?;
    let declared = entry.credential(&credential).ok_or_else(|| {
        Failure(
            StatusCode::NOT_FOUND,
            format!("`{provider}` declares no credential `{credential}`"),
        )
    })?;
    let Some(authority) = entry.authority else {
        return Ok(StatusCode::NO_CONTENT);
    };
    let reference = CredentialRef::new(tenant, authority, DEFAULT_SERVICE, declared.leaf)
        .map_err(|reason| Failure(StatusCode::BAD_REQUEST, reason))?;

    app.delete_secret(&reference)
        .await
        .map_err(|error| Failure(StatusCode::BAD_GATEWAY, format!("secret store: {error}")))?;

    Ok(StatusCode::NO_CONTENT)
}

/// A connection setting on its way in.
#[derive(Deserialize)]
pub struct ConfigInput {
    value: String,
}

/// Bind one configuration field — an endpoint variable, or a Basic user half.
pub async fn put_config(
    State(app): State<App>,
    Path((provider, service, kind, field)): Path<(String, String, String, String)>,
    Json(input): Json<ConfigInput>,
) -> Result<StatusCode, Failure> {
    // Leaked as an owned `String` because `Field` borrows its name for `'a` and the store keeps it.
    // One small leak per distinct field name, bounded by the catalogue's own vocabulary.
    let name: &'static str = Box::leak(field.into_boxed_str());
    let target = match kind.as_str() {
        "endpoint" => connector_pack::Field::Endpoint(name),
        "username" => connector_pack::Field::Username(name),
        other => {
            return Err(Failure(
                StatusCode::BAD_REQUEST,
                format!("unknown configuration kind `{other}`; expected `endpoint` or `username`"),
            ))
        }
    };

    app.settings()
        .set(tenant_of(), &provider, &service, target, input.value);
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------------------------
// The point of it all
// ---------------------------------------------------------------------------------------------

/// Run an operation and return what the vendor said.
pub async fn execute(
    State(app): State<App>,
    Path(operation): Path<String>,
    Json(params): Json<Value>,
) -> Result<Json<exec::Outcome>, Failure> {
    let outcome = exec::execute(&app, tenant_of(), &operation, params).await?;
    Ok(Json(outcome))
}
