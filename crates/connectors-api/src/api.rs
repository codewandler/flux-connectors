//! The HTTP surface.
//!
//! # The tenant
//!
//! Every handler below takes its tenant from a [`Principal`], and a `Principal` can only be built
//! from a live session cookie. Slice 1 threaded the tenant through as a parameter from the first
//! commit — with a `tenant_of()` returning the constant `"local"` — precisely so that this change
//! would be a substitution at every call site rather than a retrofit that missed one. C-204 makes
//! the substitution, and the constant is gone.
//!
//! The property is now structural: a handler that wants a tenant must name `Principal` in its
//! signature, and there is no other constructor for one. A path segment, a body field or a header
//! naming a tenant is simply ignored — asserted in `tests/tenancy.rs`, which names tenant B four
//! ways while holding tenant A's session.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use catalog::{Acquisition, CredentialRequirement, OperationKey, Placement, ProviderKey};
use connector_pack::{CredentialRef, Secret, DEFAULT_SERVICE};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::auth::Principal;
use crate::exec;
use crate::state::App;

/// An error, as JSON, with a status.
///
/// Refusals from the pack are the useful half of this host and are passed through verbatim: they
/// name the missing fact — the credential address, the unbound configuration field and its service —
/// rather than reducing to "500". None of them carries a credential value.
pub struct Failure(StatusCode, String);

impl Failure {
    /// A refusal with a status and a message.
    ///
    /// Public so that [`crate::auth`] can refuse in the same shape the rest of this surface does —
    /// one JSON envelope, so a caller has one thing to parse and a leak has one place to be caught.
    pub fn new(status: StatusCode, message: String) -> Self {
        Self(status, message)
    }
}

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

/// **What an operator still has to do with a connector** (C-212, extended by C-235).
///
/// The question the page exists to answer is *"is there anything for me to do here?"*, and there
/// are three answers, not two. `connected: bool` carried two of them and served the third — a
/// vendor that requires no credential — as the same `false` it served *supply something*. Two
/// opposite situations, one byte, in the surface a person uses to decide which of 53 connectors
/// they can use.
///
/// C-235 makes it four, by carrying the reason rather than inferring it: *nothing to supply because
/// the vendor needs nothing* and *nothing to supply because this repository will not hold what the
/// vendor needs* are also opposite answers, and C-212 could only serve them as one.
///
/// # The vocabulary is C-206's, deliberately
///
/// [`NoCredentialRequired`](Self::NoCredentialRequired) serializes as `no-credential-required` and
/// [`NoCredential`](Self::NoCredential) as `no-credential` — the exact tokens
/// `connector_cli::status::NO_CREDENTIAL_REQUIRED` and `connector_cli::status::NO_CREDENTIAL`
/// publish in the catalogue for the same two states, and the two
/// `catalog::CredentialRequirement::as_str` returns. Restating them here in different words is how
/// two surfaces describing one fact come to disagree, so this surface restates nothing.
///
/// The two tokens differ by one word and mean opposite things about whether the connector works,
/// which is a real hazard in a list a person scans. It is answered where it can be answered — the
/// operator page's copy, not a second spelling — because a UI-local rename would have been the
/// drift this whole vocabulary exists to prevent.
///
/// # What the host reads
///
/// C-206's token is a **positive declaration**: `Operation::auth` is `Some([])`, the author saying
/// the vendor needs none, as against `None` inheriting the connector default. Until C-235 the
/// embedded catalogue did not carry that distinction — `catalog::Operation::credentials` was `[]`
/// for both a positively-public operation and a credential deliberately withheld — so this host had
/// to infer the state from an absence, and freshdesk landed on `no-credential-required` for the
/// wrong reason.
///
/// It now reads `catalog::Operation::credential_requirement`, which is what the connector declares.
/// Freshdesk is `no-credential`: its API key occupies the Basic *username* position, this
/// repository deliberately does not model it (`AGENTS.md`, Intentional gaps), and the honest answer
/// to *"is there anything for me to do here?"* is **no, and it will not work either** — which is
/// neither of the states it previously had to borrow.
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Wiring {
    /// Every operation declares that its vendor requires no credential: there is nothing for an
    /// operator to supply, and every operation works.
    ///
    /// Scoped to the *outbound* direction, like everything else here. A connector may still declare
    /// an inbound signing secret and be `no-credential-required`: the secret verifies bytes that
    /// arrived, no operation authenticates with one, and the credentials list shows it either way.
    NoCredentialRequired,
    /// **Every operation's credential is withheld** — this repository deliberately does not model
    /// it, so there is nothing for an operator to supply *and* nothing they can do (C-235).
    ///
    /// The state freshdesk was borrowing [`NoCredentialRequired`](Self::NoCredentialRequired) for.
    /// The difference matters to the one person the page is for: under the old answer an operator
    /// read "nothing to supply" and reasonably concluded the connector was ready, and every call
    /// they made 401'd.
    ///
    /// Distinct from [`NotWired`](Self::NotWired) in the same way and the other direction: `not-wired`
    /// is work an operator can do, and this is not.
    NoCredential,
    /// Every operation is callable with what this tenant has stored.
    Wired,
    /// Some operations are callable and some are not — `callable_operations` of `operation_count`.
    PartlyWired,
    /// Nothing is callable yet. The operator has to supply something.
    NotWired,
}

/// One connector, as the connect page needs it.
#[derive(Serialize)]
pub struct ConnectorView {
    id: &'static str,
    vendor: &'static str,
    description: &'static str,
    authority: Option<&'static str>,
    base_url: &'static str,
    operation_count: usize,
    /// Every operation, with what it requires and whether this tenant can call it.
    ///
    /// Replaces the bare `operation_ids` list: the page still gets every id without a second index,
    /// and it no longer has to fetch 254 operation details to find out which of them it could run.
    operations: Vec<OperationWiring>,
    /// Which of the four states this connector is in for this tenant. See [`Wiring`].
    wiring: Wiring,
    /// How many of `operation_count` operations this tenant can call.
    callable_operations: usize,
    credentials: Vec<CredentialView>,
    /// Configuration fields with a value, as `("default/endpoint.subdomain", "acme")`.
    settings: Vec<(String, String)>,
    /// **The configuration slots that permit a closed set of values, and the set** (C-225).
    ///
    /// What turns the configuration form's value box into a select. Empty for nearly every
    /// connector; present for the ones whose value is a *choice* an operator cannot discover — New
    /// Relic's two region hosts, Intercom's three. Published straight from the catalogue, so the
    /// page and [`put_config`]'s refusal cannot disagree about what is permitted.
    config_choices: Vec<ConfigChoicesView>,
}

/// One configuration slot whose value comes from a closed set — C-225.
///
/// A view rather than the catalogue's own type because `catalog` has **no dependencies** and
/// therefore no `Serialize`; the mapping is one-to-one and the field names are the catalogue's.
#[derive(Serialize)]
pub struct ConfigChoicesView {
    /// The service this field configures — the first path segment of `PUT /v1/config/…`.
    service: &'static str,
    /// The declared field name, for the row's label copy.
    field: &'static str,
    /// The form label.
    label: &'static str,
    /// `endpoint` or `username` — the `kind` segment of the same route.
    kind: &'static str,
    /// The binding target — the `field` segment of the same route.
    name: &'static str,
    /// The permitted values, in the vendor's own order.
    choices: Vec<ChoiceView>,
}

/// One permitted value and the text the page shows for it.
#[derive(Serialize)]
pub struct ChoiceView {
    value: &'static str,
    label: &'static str,
}

/// One operation, and whether this tenant can call it — **the honest unit** (C-212).
///
/// A connector is not the unit an operator asks about, because a connector's credentials do not all
/// belong to the same surface. Anthropic declares `api_key`, which nearly every operation carries,
/// and `admin_key`, which belongs to the management API; requiring both made a connector with the
/// first one stored read as entirely unwired.
///
/// # Why this carries the operation's own facts too — C-237
///
/// C-212 added [`requires`](Self::requires) and [`callable`](Self::callable) so the page would not
/// have to fetch every operation to find out which of them it could run, and its Progress note says
/// exactly that. The page kept fetching them anyway, for `tool`, `description` and `risk` — up to
/// ~30 requests per connector click, to read three fields off a catalogue entry this function
/// already holds. So the five fields the list actually renders travel with the list.
///
/// **What deliberately does not travel here** is `flux` and `input_schema`. Those are the two the
/// operator reads *one at a time*, when they expand an operation, and they are by far the largest —
/// `flux` alone is a whole rendered declaration. Putting them here would trade an N+1 for a
/// response nobody wants all of. `GET /v1/operations/{id}` remains the expansion, and it is now the
/// only reason to call it. `tests/catalogue_response.rs` states the size this leaves the list at.
#[derive(Serialize)]
pub struct OperationWiring {
    /// The Flux symbol, as [`ConnectorView::operations`]'s predecessor carried it.
    id: &'static str,
    /// The dotted name a caller actually invokes — `zendesk.ticket.show`.
    ///
    /// Derived here rather than by the page: it is [`connector_pack::dotted_name`]'s answer, and a
    /// page deriving its own would be a second spelling of the name a tool registers under.
    tool: String,
    /// **The service this operation belongs to** — the addressing level C-49 established.
    ///
    /// `OperationView` has always carried it and the page discarded it, so ~30 operations rendered
    /// as one flat list when contentful's `delivery` and `management` are two different APIs
    /// against two different spaces.
    service: &'static str,
    /// What the operation does, in the catalogue's own words. [`Published`] on the page: a
    /// catalogue that does not carry one is not an operation without one.
    ///
    /// [`Published`]: https://github.com/codewandler/flux-connectors/blob/main/docs/stories/C-408-components-cannot-say-unpublished.md
    description: &'static str,
    /// `low`, `medium` or `high`, lowercased exactly as [`OperationView::risk`] renders it.
    risk: String,
    /// `idempotent` or `nonidempotent`, lowercased exactly as [`OperationView::idempotency`] does.
    ///
    /// The one fact that says whether a retry is safe, and the page threw it away.
    idempotency: String,
    /// The hosts this operation reaches. Never empty for a shipped operation —
    /// `connector_pack::Operation::project` refuses one with no declared host.
    hosts: &'static [&'static str],
    /// What the operation needs, in the catalogue's own shape: the outer list is an **OR** over
    /// ways to authenticate, each inner list an **AND** of credentials that must travel together.
    /// Empty means the operation needs no credential.
    ///
    /// Names, never values — these are [`CredentialView::name`]s, and the page joins on them.
    requires: &'static [&'static [&'static str]],
    /// **Why `requires` is what it is** — and, when it is empty, which of the two opposite reasons
    /// that is (C-235). Serializes as `declared`, `no-credential-required` or `no-credential`.
    ///
    /// Kept as the catalogue's own type and serialized through
    /// [`CredentialRequirement::as_str`](catalog::CredentialRequirement::as_str) rather than
    /// stored as a string, so the token on the wire is the catalogue's and cannot be re-spelled
    /// here.
    #[serde(serialize_with = "credential_requirement_token")]
    requirement: CredentialRequirement,
    /// Whether this tenant can call it: one whole mechanism stored, or a vendor that needs none.
    ///
    /// **A withheld credential is not callable**, which is the correction C-235 makes here. An
    /// empty `requires` used to be read as "needs nothing, so anyone can call it", and for
    /// freshdesk's nine operations that was false — the request goes out unauthenticated and 401s.
    callable: bool,
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

/// Every connector in the catalogue, as **this session's tenant** has it configured.
pub async fn connectors(
    State(app): State<App>,
    principal: Principal,
) -> Result<Json<Vec<ConnectorView>>, Failure> {
    let tenant = principal.tenant();
    let mut views = Vec::new();
    for provider in catalog::providers() {
        views.push(view_of(&app, tenant, provider).await?);
    }
    Ok(Json(views))
}

/// One connector.
pub async fn connector(
    State(app): State<App>,
    principal: Principal,
    Path(provider): Path<String>,
) -> Result<Json<ConnectorView>, Failure> {
    let entry = catalog::provider(ProviderKey::id(&provider)).ok_or_else(|| {
        Failure(
            StatusCode::NOT_FOUND,
            format!("no connector `{provider}` in this catalogue"),
        )
    })?;
    Ok(Json(view_of(&app, principal.tenant(), entry).await?))
}

/// **Whether an operation can be called with the credentials named in `stored`.**
///
/// `mechanisms` is [`catalog::Operation::credentials`] verbatim: an OR over ways to authenticate,
/// each an AND of credentials that must be on the same request. `requirement` is the same
/// operation's [`catalog::Operation::credential_requirement`], and it is what decides the empty
/// case — which C-235 is the story for. An empty mechanism list is *callable by anyone* when the
/// connector declared that its vendor needs nothing, and callable by **no one** when the credential
/// is withheld: the request goes out unauthenticated and the vendor refuses it.
///
/// Pure and taking only names, so the states can be proved against **fixtures**. That matters more
/// than usual here: the positively-public case has still not shipped, so a test over the catalogue
/// could only assert the states that already existed.
fn is_callable(
    mechanisms: &[&[&str]],
    requirement: CredentialRequirement,
    stored: &[&str],
) -> bool {
    match requirement {
        CredentialRequirement::NoneRequired => true,
        CredentialRequirement::Withheld => false,
        CredentialRequirement::Declared => mechanisms
            .iter()
            .any(|mechanism| mechanism.iter().all(|name| stored.contains(name))),
    }
}

/// **Which of the four states a connector is in**, from its operations' own answers.
///
/// The first two arms read what the connector **declares**, and that is C-235: the test used to be
/// `requires.is_empty()`, which is true of a positively-public operation and of a withheld one
/// alike, so freshdesk was served as though there were nothing to supply *and* everything worked.
/// Only the second half of that was ever true of it.
///
/// A connector mixing the two — some operations public, some withheld — falls through to the
/// counting arms, which is right: the answer is then per operation, and `partly-wired` with the
/// per-operation `callable` flags is what says which.
///
/// Note what is *not* here: a special case for [`Placement::Inbound`]. The loop this replaced
/// carried one, with the right reasoning — *"an inbound signing secret never leaves, so it is not
/// part of 'can this connector call out'. Counting it would show a connector as unconnectable for
/// want of a value no outgoing request would ever carry."* That reasoning was never specific to
/// inbound; `admin_key` is also a value no ordinary outgoing request carries. Asking each operation
/// what *it* declares applies the principle everywhere at once, and the inbound case falls out of
/// it: no operation may authenticate with a signing secret (`AGENTS.md`, authentication contract),
/// so one never appears in a mechanism list and never counts against anything.
fn wiring_of(operations: &[OperationWiring]) -> Wiring {
    let every = |requirement| {
        operations
            .iter()
            .all(|operation| operation.requirement == requirement)
    };
    if every(CredentialRequirement::NoneRequired) {
        return Wiring::NoCredentialRequired;
    }
    if every(CredentialRequirement::Withheld) {
        return Wiring::NoCredential;
    }
    match operations.iter().filter(|op| op.callable).count() {
        callable if callable == operations.len() => Wiring::Wired,
        0 => Wiring::NotWired,
        _ => Wiring::PartlyWired,
    }
}

/// Serialize a [`CredentialRequirement`] as the catalogue's own token.
///
/// One line rather than a `From` impl or a mirrored enum: the token is the catalogue's to spell,
/// and every way of restating it here is a second spelling waiting to drift from the first.
fn credential_requirement_token<S: serde::Serializer>(
    requirement: &CredentialRequirement,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(requirement.as_str())
}

async fn view_of(
    app: &App,
    tenant: &str,
    provider: &'static catalog::Provider,
) -> Result<ConnectorView, Failure> {
    let mut credentials = Vec::new();
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

    // The names that have a value, which is the only thing callability depends on. Never a value.
    let stored: Vec<&str> = credentials
        .iter()
        .filter(|credential| credential.stored)
        .map(|credential| credential.name)
        .collect();

    // **The whole list, whole** (C-237). A `for` rather than a `map` because `dotted_name` can
    // refuse, and a name this host cannot compose is a catalogue defect worth reporting as one
    // rather than eliding into an operation the page cannot invoke.
    let mut operations = Vec::with_capacity(provider.operations.len());
    for operation in provider.operations {
        let tool = connector_pack::dotted_name(operation.id)
            .map_err(|error| Failure(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
        operations.push(OperationWiring {
            id: operation.id,
            tool,
            service: operation.service,
            description: operation.description,
            risk: format!("{:?}", operation.risk).to_lowercase(),
            idempotency: format!("{:?}", operation.idempotency).to_lowercase(),
            hosts: operation.hosts,
            requires: operation.credentials,
            requirement: operation.credential_requirement,
            callable: is_callable(
                operation.credentials,
                operation.credential_requirement,
                &stored,
            ),
        });
    }

    Ok(ConnectorView {
        id: provider.id,
        vendor: provider.vendor,
        description: provider.description,
        authority: provider.authority,
        base_url: provider.base_url,
        operation_count: provider.operations.len(),
        wiring: wiring_of(&operations),
        callable_operations: operations.iter().filter(|op| op.callable).count(),
        operations,
        credentials,
        settings: app.settings().bound_for(tenant, provider.id),
        config_choices: provider
            .config_choices
            .iter()
            .map(|entry| ConfigChoicesView {
                service: entry.service,
                field: entry.field,
                label: entry.label,
                kind: entry.kind,
                name: entry.name,
                choices: entry
                    .choices
                    .iter()
                    .map(|choice| ChoiceView {
                        value: choice.value,
                        label: choice.label,
                    })
                    .collect(),
            })
            .collect(),
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
    /// **The parameters this operation takes, as a JSON Schema** — C-237.
    ///
    /// [`connector_pack::project`]'s answer, which is flux's own `OpSpec::lower` and therefore the
    /// exact schema a model is handed. No codegen and no second derivation: the page draws one
    /// control per property and every declared parameter is required by construction, because a
    /// composite op declaration has no optional-parameter concept.
    ///
    /// It is on *this* view rather than on [`OperationWiring`] for the reason that type documents:
    /// this is the response an operator asks for one operation at a time.
    input_schema: Value,
    /// The operation's own emitted Flux — the human-readable contract behind the request.
    flux: &'static str,
}

/// One operation.
///
/// Takes a `Principal` it does not read. Everything here is published catalogue data — the same
/// facts `web/public/catalog.json` serves to the open internet — so nothing tenant-scoped could
/// leak from it. It is gated anyway, so that "every route under `/v1` takes a `Principal`" is a
/// rule with no exception to remember: an ungated route sitting among gated ones is the one a later
/// change extends with a tenant-scoped field.
#[allow(clippy::needless_pass_by_value)]
pub async fn operation(
    _principal: Principal,
    Path(operation): Path<String>,
) -> Result<Json<OperationView>, Failure> {
    let entry = catalog::operation(OperationKey::id(&operation)).ok_or_else(|| {
        Failure(
            StatusCode::NOT_FOUND,
            format!("no operation `{operation}` in this catalogue"),
        )
    })?;
    let tool = connector_pack::dotted_name(entry.id)
        .map_err(|error| Failure(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    // The projection, for its schema alone. It is the same function `resolve` runs on the way to a
    // real call, so a schema the page draws a form from is the contract the call will be checked
    // against rather than a description of it.
    let spec = connector_pack::project(entry)
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
        input_schema: spec.input_schema,
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
    principal: Principal,
    Path((provider, credential)): Path<(String, String)>,
    Json(input): Json<CredentialInput>,
) -> Result<StatusCode, Failure> {
    let tenant = principal.tenant();
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
    principal: Principal,
    Path((provider, credential)): Path<(String, String)>,
) -> Result<StatusCode, Failure> {
    let tenant = principal.tenant();
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
///
/// # A value outside a declared closed set is refused **here** — C-225
///
/// This is *the point a value is supplied*, and it is deliberately the only place membership is
/// checked. New Relic serves one API from two hosts and nothing pre-auth says which is yours, so a
/// wrong region is a `401` on every call that reads exactly like a bad key; the whole value of
/// declaring the set is that the mistake is caught at the input rather than diagnosed from a status
/// code that names the wrong cause. The refusal lists the permitted values for the same reason —
/// "invalid" would reproduce the guessing this exists to remove.
///
/// **A stored value that later leaves the set is left alone.** Nothing re-validates on read, so a
/// vendor adding a region does not brick a connection configured before it existed: the next edit
/// of that field is where the operator is asked to pick again. Refusing at read time would turn a
/// catalogue update into an outage on connections that were never wrong.
pub async fn put_config(
    State(app): State<App>,
    principal: Principal,
    Path((provider, service, kind, field)): Path<(String, String, String, String)>,
    Json(input): Json<ConfigInput>,
) -> Result<StatusCode, Failure> {
    // Leaked as an owned `String` because `Field` borrows its name for `'a` and the store keeps it.
    // One small leak per distinct field name, bounded by the catalogue's own vocabulary.
    let name: &'static str = Box::leak(field.into_boxed_str());
    let target = match kind.as_str() {
        "endpoint" => connector_pack::Field::Endpoint(name),
        "username" => connector_pack::Field::Username(name),
        "channel_query" => {
            let (channel, parameter) = name.split_once('.').ok_or_else(|| {
                Failure(
                    StatusCode::BAD_REQUEST,
                    "a channel query field is `<binding>.<parameter>`".to_owned(),
                )
            })?;
            connector_pack::Field::ChannelQuery { channel, parameter }
        }
        other => {
            return Err(Failure(
                StatusCode::BAD_REQUEST,
                format!(
                    "unknown configuration kind `{other}`; expected `endpoint`, `username` or \
                     `channel_query`"
                ),
            ))
        }
    };

    // An unknown provider is not refused here — it never was, and a setting bound under one is
    // inert rather than dangerous — but a *known* provider's closed set is enforced.
    if let Some(entry) = catalog::provider(catalog::ProviderKey::id(&provider))
        .and_then(|declared| declared.choices_for(&service, &kind, name))
    {
        if !entry.choices.iter().any(|c| c.value == input.value) {
            let permitted: Vec<String> = entry
                .choices
                .iter()
                .map(|choice| format!("`{}` ({})", choice.value, choice.label))
                .collect();
            return Err(Failure(
                StatusCode::BAD_REQUEST,
                format!(
                    "`{}` permits only {}, and `{}` is none of them",
                    entry.field,
                    permitted.join(", "),
                    input.value
                ),
            ));
        }
    }

    app.settings()
        .set(principal.tenant(), &provider, &service, target, input.value);
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------------------------
// The point of it all
// ---------------------------------------------------------------------------------------------

/// Run an operation and return what the vendor said.
///
/// The tenant handed to `exec::execute` — and from there to `Credentials::new` and
/// `Configuration::new` — is the session's. **This is the line that makes the whole crate not a
/// confused deputy:** the credential this request sends belongs to the person who made it.
pub async fn execute(
    State(app): State<App>,
    principal: Principal,
    Path(operation): Path<String>,
    Json(params): Json<Value>,
) -> Result<Json<exec::Outcome>, Failure> {
    let outcome = exec::execute(&app, principal.tenant(), &operation, params).await?;
    Ok(Json(outcome))
}

/// **Rehearse an operation: the exact request, without sending it** (C-237, over C-145's seam).
///
/// The route that answers *"why will this not work"* precisely. Everything it can refuse names the
/// missing fact rather than a status code — the unbound configuration field and the service it
/// belongs to, the credential address with nothing at it — and it reaches no socket and no secret
/// store to do it. See [`exec::dry_run`].
pub async fn dry_run(
    State(app): State<App>,
    principal: Principal,
    Path(operation): Path<String>,
    Json(params): Json<Value>,
) -> Result<Json<Value>, Failure> {
    Ok(Json(exec::dry_run(
        &app,
        principal.tenant(),
        &operation,
        &params,
    )?))
}

// ---------------------------------------------------------------------------------------------
// The three states, against fixtures
// ---------------------------------------------------------------------------------------------

/// **Why these are fixtures and not the catalogue** (C-212, extended by C-235).
///
/// One of the four states is *latent*: nothing in the shipped catalogue positively declares
/// `auth = []` yet, so a test written over `catalog::providers()` could only exercise the states
/// that already worked, and would go green on a classifier that still collapsed the public case
/// into the withheld one. A declaration is four characters to write down, so it is written down.
///
/// `tests/wiring.rs` carries the other half — the same states over the real HTTP surface, on the
/// shipped connectors that reach them.
#[cfg(test)]
mod tests {
    use super::{is_callable, wiring_of, CredentialRequirement, OperationWiring, Wiring};

    /// An operation that authenticates, against a set of stored credential names.
    fn declared(
        id: &'static str,
        requires: &'static [&'static [&'static str]],
        stored: &[&str],
    ) -> OperationWiring {
        requirement(id, requires, CredentialRequirement::Declared, stored)
    }

    /// An operation whose vendor requires no credential — C-206's positive declaration.
    fn public(id: &'static str) -> OperationWiring {
        requirement(id, &[], CredentialRequirement::NoneRequired, &[])
    }

    /// An operation whose credential this repository will not hold — freshdesk's shape.
    fn withheld(id: &'static str) -> OperationWiring {
        requirement(id, &[], CredentialRequirement::Withheld, &[])
    }

    /// One fixture operation, with its callability derived exactly as [`super::view_of`] derives
    /// it — so a change to [`is_callable`] cannot pass here and fail on the wire.
    fn requirement(
        id: &'static str,
        requires: &'static [&'static [&'static str]],
        requirement: CredentialRequirement,
        stored: &[&str],
    ) -> OperationWiring {
        OperationWiring {
            id,
            // The five C-237 added travel with the list and none of them takes part in a wiring
            // decision, so they are filled with the shape rather than with a value under test. A
            // fixture that varied them would be asserting about the serializer.
            tool: id.replace('-', "."),
            service: connector_pack::DEFAULT_SERVICE,
            description: "a fixture operation",
            risk: "low".to_owned(),
            idempotency: "idempotent".to_owned(),
            hosts: &["api.vendor.test"],
            requires,
            requirement,
            callable: is_callable(requires, requirement, stored),
        }
    }

    /// **The third state, which no shipped connector reaches yet.**
    ///
    /// A vendor requiring no credential is `no-credential-required` with nothing stored — not
    /// `not-wired`, which is what an operator is shown when they must go and find a token.
    #[test]
    fn a_connector_whose_vendor_needs_no_credential_has_nothing_to_supply() {
        let connector = [public("ping"), public("version")];
        assert_eq!(wiring_of(&connector), Wiring::NoCredentialRequired);
        assert!(connector.iter().all(|operation| operation.callable));
    }

    /// **And it is not the same state as a credential simply left unset.**
    ///
    /// The whole story in one assertion: two opposite situations, and before C-212 both were
    /// `connected: false`.
    #[test]
    fn nothing_to_supply_is_not_the_same_state_as_supply_something() {
        let open = [public("ping")];
        let unset = [declared("list", &[&["vendor.api_key"]], &[])];
        assert_ne!(wiring_of(&open), wiring_of(&unset));
        assert_eq!(wiring_of(&unset), Wiring::NotWired);
    }

    /// **C-235's assertion: nor is it the same state as a credential this repository withholds.**
    ///
    /// Both declare no credential and both leave an operator with nothing to supply, which is why
    /// C-212 had to serve them alike — and they are opposite answers to whether the connector
    /// works. A public operation is callable by anyone; a withheld one is callable by no one,
    /// because the request goes out unauthenticated and the vendor refuses it.
    #[test]
    fn a_public_operation_is_not_the_same_state_as_a_withheld_credential() {
        let open = [public("ping")];
        let refused = [withheld("ticket-list")];

        assert_eq!(wiring_of(&open), Wiring::NoCredentialRequired);
        assert_eq!(wiring_of(&refused), Wiring::NoCredential);
        assert_ne!(
            wiring_of(&open),
            wiring_of(&refused),
            "a vendor that needs nothing and a credential this repository will not hold are \
             served identically again"
        );

        assert!(
            open[0].callable,
            "the unauthenticated call is the right one"
        );
        assert!(
            !refused[0].callable,
            "an unauthenticated request to an endpoint that wants a credential is a 401, and \
             telling an operator it is callable is the lie C-235 exists to remove"
        );
    }

    /// **The two tokens are the catalogue's own**, character for character — C-206's vocabulary
    /// rather than a second spelling of it. `tests/wiring_vocabulary.rs` holds the page to them.
    #[test]
    fn the_two_empty_states_travel_as_the_tokens_the_catalogue_publishes() {
        let token = |wiring| serde_json::to_value(wiring).expect("Wiring serializes");
        assert_eq!(
            token(Wiring::NoCredentialRequired),
            "no-credential-required"
        );
        assert_eq!(token(Wiring::NoCredential), "no-credential");
        assert_eq!(
            CredentialRequirement::NoneRequired.as_str(),
            "no-credential-required"
        );
        assert_eq!(CredentialRequirement::Withheld.as_str(), "no-credential");
    }

    /// **An operation that positively declares no credential under a connector that has some.**
    ///
    /// A partly-public connector is callable in part with nothing stored, and the connector-level
    /// answer is the counting one rather than either of the two whole-connector states.
    #[test]
    fn a_public_operation_is_callable_under_a_connector_that_declares_credentials() {
        let mixed = [
            public("ping"),
            declared("list", &[&["vendor.api_key"]], &[]),
        ];
        assert_eq!(wiring_of(&mixed), Wiring::PartlyWired);
        assert!(mixed[0].callable, "a public operation needs nothing stored");
        assert!(!mixed[1].callable);
    }

    /// **A connector mixing a public operation with a withheld one is neither whole-connector
    /// state**, and says so per operation.
    #[test]
    fn a_connector_mixing_public_and_withheld_operations_is_partly_wired() {
        let mixed = [public("ping"), withheld("ticket-list")];
        assert_eq!(wiring_of(&mixed), Wiring::PartlyWired);
        assert_eq!(mixed.iter().filter(|op| op.callable).count(), 1);
    }

    /// **Anthropic's shape: one credential stored, two surfaces.**
    ///
    /// The second half of the defect. `all_stored` required every declared credential, so this
    /// connector read as entirely unwired for want of `admin_key` — a value no ordinary outgoing
    /// request carries, which is the reason the loop already excluded inbound secrets.
    #[test]
    fn storing_the_credential_an_operation_uses_makes_that_operation_callable() {
        let stored = ["vendor.api_key"];
        let connector = [
            declared("models-list", &[&["vendor.api_key"]], &stored),
            declared("model-get", &[&["vendor.api_key"]], &stored),
            declared("organization-get", &[&["vendor.admin_key"]], &stored),
        ];
        assert_eq!(wiring_of(&connector), Wiring::PartlyWired);
        assert_eq!(connector.iter().filter(|op| op.callable).count(), 2);
    }

    /// **Every operation callable is `wired`.**
    #[test]
    fn a_connector_whose_every_operation_is_callable_is_wired() {
        let stored = ["vendor.api_key", "vendor.admin_key"];
        let connector = [
            declared("list", &[&["vendor.api_key"]], &stored),
            declared("admin", &[&["vendor.admin_key"]], &stored),
        ];
        assert_eq!(wiring_of(&connector), Wiring::Wired);
    }

    /// **A mechanism is an AND; the mechanism list is an OR.**
    ///
    /// babelforce is why: `&[&["babelforce.access_id", "babelforce.access_token"]]` is one way to
    /// authenticate needing two headers together, not two ways. Half of it stored is no way at all.
    #[test]
    fn one_mechanism_needs_all_of_its_credentials_and_any_mechanism_will_do() {
        let declared = CredentialRequirement::Declared;
        let pair: &[&[&str]] = &[&["vendor.access_id", "vendor.access_token"]];
        assert!(!is_callable(pair, declared, &["vendor.access_id"]));
        assert!(is_callable(
            pair,
            declared,
            &["vendor.access_id", "vendor.access_token"]
        ));

        let alternatives: &[&[&str]] = &[&["vendor.oauth_token"], &["vendor.api_key"]];
        assert!(is_callable(alternatives, declared, &["vendor.api_key"]));
        assert!(!is_callable(alternatives, declared, &["vendor.unrelated"]));
    }

    /// **An inbound signing secret is excluded by construction, not by a special case.**
    ///
    /// No operation may authenticate with a signing secret, so one never appears in a mechanism
    /// list — and a connector that declares one alongside operations needing nothing outbound is
    /// still `no-credential-required`, unstored secret and all. That is the same answer the old
    /// `all_stored` loop reached through an explicit `Placement::Inbound` arm.
    #[test]
    fn an_unstored_inbound_secret_holds_no_operation_back() {
        let connector = [public("event-reply")];
        assert_eq!(wiring_of(&connector), Wiring::NoCredentialRequired);
    }
}
